//!
//! Contains data structures for describing and implementations for parsing a functions's FFI
//! attributes.
//!

use proc_macro_error::{abort, ResultExt};
use std::collections::HashMap;
use syn::{spanned::Spanned, Ident, Meta, NestedMeta, Path, Type, TypePath};

/// Function-level FFI helper attributes.
///
pub struct FnAttributes {
    /// The type to be extended with an implementation for this function in the consumer.
    ///
    pub extend_type: Ident,

    /// Any types in this function that should be treated as raw types.
    ///
    pub raw_types: Vec<Ident>,

    /// A hashmap whose keys are `Ident`s for the generics used in this function and whose values
    /// are `Ident`s for the concrete types to use in place of the generic for FFI.
    ///
    pub generics: HashMap<Type, Type>,
}

impl From<syn::AttributeArgs> for FnAttributes {
    fn from(args: syn::AttributeArgs) -> Self {
        let mut extend_type: Option<Ident> = None;
        let mut raw_types = vec![];
        let mut generics = HashMap::<Type, Type>::new();
        for arg in &args {
            match arg {
                NestedMeta::Meta(m) => {
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
                    if m.path().is_ident("extend_type") {
                        if extend_type.is_some() {
                            abort!(m.span(), "Duplicate `extend_type` attribute defined for a single call. This attribute must be set once at most.")
                        }
                        extend_type = match paths.first() {
                            Some(path) => path.get_ident().cloned(),
                            None => abort!(m.span(), "Paths is empty?"),
                        };
                    } else if m.path().is_ident("raw_types") {
                        if !raw_types.is_empty() {
                            abort!(m.span(), "Duplicate `raw_types` attribute defined for a single call. This attribute must be set once at most.")
                        }
                        raw_types = paths
                            .iter()
                            .filter_map(syn::Path::get_ident)
                            .cloned()
                            .collect();
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
                            "Unsupported ffi attribute {:?} -- expected `generic`, `raw_types`, or `extend_type`.",
                            m.path()
                        )
                    }
                }
                other @ NestedMeta::Lit(_) => {
                    abort!(
                        other.span(),
                        "Unsupported `NestedMeta::Lit` ffi attribute -- {:?}",
                        arg
                    )
                }
            }
        }
        let extend_type = match extend_type {
            Some(extend_type) => extend_type,
            None => {
                abort!(extend_type.span(), "`extend_type` attribute must be set.")
            }
        };

        Self {
            extend_type,
            raw_types,
            generics,
        }
    }
}
