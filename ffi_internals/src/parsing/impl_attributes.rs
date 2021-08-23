//!
//! Contains data structures for describing and implementations for parsing an impl's FFI
//! attributes.
//!

use proc_macro_error::abort;
use syn::{spanned::Spanned, Ident, Meta, NestedMeta, Path};

/// Impl-level FFI helper attributes.
///
pub struct ImplAttributes {
    /// Any imports that need to be included in the generated FFI module.
    pub ffi_imports: Vec<Path>,
    /// Any imports the consumer will need in order to support the implementation.
    pub consumer_imports: Vec<Path>,
    /// Any types in this function that should be treated as raw types.
    pub raw_types: Vec<Ident>,
}

impl From<syn::AttributeArgs> for ImplAttributes {
    fn from(args: syn::AttributeArgs) -> Self {
        let mut ffi_imports = vec![];
        let mut consumer_imports = vec![];
        let mut raw_types = vec![];
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
                    ffi_imports = paths
                } else if m.path().is_ident("consumer_imports") {
                    if !consumer_imports.is_empty() {
                        abort!(m.span(), "Duplicate `consumer_imports` attribute defined for a single call. This attribute must be set once at most.")
                    }
                    consumer_imports = paths
                } else if m.path().is_ident("raw_types") {
                    if !raw_types.is_empty() {
                        abort!(m.span(), "Duplicate `raw_types` attribute defined for a single call. This attribute must be set once at most.")
                    }
                    raw_types = paths
                        .iter()
                        .filter_map(syn::Path::get_ident)
                        .cloned()
                        .collect();
                } else {
                    abort!(m.span(), "Unsupported ffi attribute -- ")
                }
            } else {
                abort!(arg.span(), "Unsupported ffi attribute.")
            }
        }
        Self {
            ffi_imports,
            consumer_imports,
            raw_types,
        }
    }
}
