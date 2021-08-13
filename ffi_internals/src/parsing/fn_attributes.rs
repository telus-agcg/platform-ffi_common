use syn::{Ident, Meta, NestedMeta, Path, spanned::Spanned};

pub struct FnAttributes {
    pub extend_type: Ident,
    pub raw_types: Vec<Ident>,
}

impl From<syn::AttributeArgs> for FnAttributes {
    fn from(args: syn::AttributeArgs) -> Self {
        let mut extend_type: Option<Ident> = None;
        let mut raw_types = vec![];
        for arg in &args {
            match arg {
                NestedMeta::Meta(m) => {
                    let paths: Vec<Path> = match m {
                        Meta::List(l) => {
                            l
                            .nested
                            .iter()
                            .filter_map(super::parse_path_from_nested_meta)
                            .collect()
                        },
                        Meta::Path(_) | Meta::NameValue(_) => proc_macro_error::abort!(m.span(), "Unsupported meta type."),
                    };
                    if m.path().is_ident("extend_type") {
                        if extend_type.is_some() {
                            proc_macro_error::abort!(m.span(), "Duplicate `extend_type` attribute defined for a single call. This attribute must be set once at most.")
                        }
                        extend_type = match paths.first() {
                            Some(path) => path.get_ident().cloned(),
                            None => proc_macro_error::abort!(m.span(), "Paths is empty?"),
                        };
                    // }
                    //  else if m.path().is_ident("consumer_imports") {
                    //     if !consumer_imports.is_empty() {
                    //         panic!("Duplicate `consumer_imports` attributes defined for a single macro call")
                    //     }
                    //     consumer_imports = paths
                    } else if m.path().is_ident("raw_types") {
                        if !raw_types.is_empty() {
                            proc_macro_error::abort!(m.span(), "Duplicate `raw_types` attribute defined for a single call. This attribute must be set once at most.")
                        }
                        raw_types = paths
                            .iter()
                            .filter_map(syn::Path::get_ident)
                            .cloned()
                            .collect();
                    } else {
                        proc_macro_error::abort!(m.span(), "Unsupported ffi attribute -- ")
                    }
                }
                other @ NestedMeta::Lit(_) => proc_macro_error::abort!(other.span(), "Unsupported ffi attribute -- "),
            }
        }
        let extend_type = match extend_type {
            Some(extend_type) => extend_type,
            None => proc_macro_error::abort!(extend_type.span(), "`extend_type` attribute must be set."),
        };
        Self {
            extend_type,
            raw_types,
        }
    }
}