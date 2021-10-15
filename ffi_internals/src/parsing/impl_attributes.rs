//!
//! Contains data structures for describing and implementations for parsing an impl's FFI
//! attributes.
//!

use proc_macro_error::{abort, OptionExt, ResultExt};
use std::collections::HashMap;
use syn::{spanned::Spanned, Ident, Meta, NestedMeta, Path, Type, TypePath};

/// Impl-level FFI helper attributes.
///
pub struct ImplAttributes {
    /// Any imports that need to be included in the generated FFI module.
    ///
    pub ffi_imports: Vec<Path>,

    /// Any imports the consumer will need in order to support the implementation.
    ///
    pub consumer_imports: Vec<Path>,

    /// Any types in this function that should be treated as raw types.
    ///
    pub raw_types: Vec<Ident>,

    /// A description of this impl, to be used in generating a unique name for the type and impl.
    ///
    /// When operating on a trait impl, we can use the trait name, so this is unnecessary. However,
    /// when we want to operate on something like `impl SomeType { ... }`, we need a way to uniquely
    /// identify that impl (otherwise the file for this would collide with the type definition file
    /// and/or other impls for the same type).
    ///
    pub description: Option<Ident>,

    /// A hashmap whose keys are `Type`s for the generics used throughout this impl and whose
    /// values are `Type`s for the concrete types to use in place of the generic for FFI.
    ///
    /// # Limitations
    ///
    /// This breaks down if an impl contains multiple generic functions that use the same generic
    /// parameter but need to be matched with different concrete types. If we run into that use
    /// case, we'll need to do something else (or perhaps let the function attributes override in
    /// those cases?).
    ///
    pub generics: HashMap<Type, Type>,
}

impl From<syn::AttributeArgs> for ImplAttributes {
    fn from(args: syn::AttributeArgs) -> Self {
        let mut ffi_imports = vec![];
        let mut consumer_imports = vec![];
        let mut raw_types = vec![];
        let mut description: Option<Ident> = None;
        let mut generics = HashMap::<Type, Type>::new();
        for arg in &args {
            if let NestedMeta::Meta(m) = arg {
                let paths: Vec<Path> = match m {
                    Meta::List(l) => l
                        .nested
                        .iter()
                        .filter_map(super::parse_path_from_nested_meta)
                        .collect(),
                    Meta::Path(_) | Meta::NameValue(_) => {
                        abort!(m.span(), "Unsupported meta type.")
                    }
                };
                if m.path().is_ident("ffi_imports") {
                    if !ffi_imports.is_empty() {
                        abort!(m.span(), "Duplicate `ffi_imports` attribute defined for a single call. This attribute must be set once at most.")
                    }
                    ffi_imports = paths;
                } else if m.path().is_ident("consumer_imports") {
                    if !consumer_imports.is_empty() {
                        abort!(m.span(), "Duplicate `consumer_imports` attribute defined for a single call. This attribute must be set once at most.")
                    }
                    consumer_imports = paths;
                } else if m.path().is_ident("raw_types") {
                    if !raw_types.is_empty() {
                        abort!(m.span(), "Duplicate `raw_types` attribute defined for a single call. This attribute must be set once at most.")
                    }
                    raw_types = paths
                        .iter()
                        .filter_map(syn::Path::get_ident)
                        .cloned()
                        .collect();
                } else if m.path().is_ident("description") {
                    if description.is_some() {
                        abort!(m.span(), "Duplicate `description` attribute defined for a single call. This attribute must be set once at most.")
                    }
                    if let Meta::List(l) = m {
                        let nested = l.nested.first()
                            .expect_or_abort("Attribute `description` missing nested value. Use it like `description(\"some description\"");
                        if let NestedMeta::Lit(syn::Lit::Str(lit)) = nested {
                            description = Some(quote::format_ident!("{}", lit.value()));
                        }
                    }
                } else if m.path().is_ident("generic") {
                    if let Meta::List(l) = m {
                        generics = l.nested.iter().fold(generics, |mut acc, n| {
                            if let NestedMeta::Meta(nested_meta) = n {
                                let generic = Type::Path(TypePath {
                                    qself: None,
                                    path: nested_meta.path().clone(),
                                });
                                if let Meta::NameValue(name_value) = nested_meta {
                                    // TODO: We could accept a list of types here to
                                    // implement this for, making it possible to expose an
                                    // FFI for f64, f32, etc all in one derive.
                                    if let syn::Lit::Str(lit) = name_value.lit.clone() {
                                        let ty: Type =
                                            syn::parse_str(&lit.value()).unwrap_or_abort();
                                        if acc.insert(generic.clone(), ty).is_some() {
                                            abort!(
                                                m.span(),
                                                "Multiple definitions for generic {:?} found.",
                                                generic
                                            )
                                        }
                                    }
                                }
                            }
                            acc
                        });
                    }
                } else {
                    abort!(
                        m.span(),
                        "Unsupported ffi attribute {:?} -- expected `ffi_imports`, `consumer_imports`, `raw_types`, `description`, or `generic`, ",
                        m.path())
                }
            } else {
                abort!(arg.span(), "Unsupported ffi attribute -- {:?}.", arg)
            }
        }
        Self {
            ffi_imports,
            consumer_imports,
            raw_types,
            description,
            generics,
        }
    }
}
