use syn::{Ident, Meta, NestedMeta, Path};

pub struct FnAttributes {
    pub extend_type: Ident,
    pub raw_types: Vec<Ident>,
}

impl From<syn::AttributeArgs> for FnAttributes {
    fn from(args: syn::AttributeArgs) -> Self {
        let mut extend_type: Option<Ident> = None;
        let mut raw_types = vec![];
        for arg in args.iter() {
            match arg {
                NestedMeta::Meta(m) => {
                    let paths: Vec<Path> = match m {
                        Meta::List(l) => {
                            l
                            .nested
                            .iter()
                            .flat_map(super::parse_path_from_nested_meta)
                            .collect()
                        },
                        Meta::Path(_) | Meta::NameValue(_) => panic!("Unsupported meta types"),
                    };
                    if m.path().is_ident("extend_type") {
                        if extend_type.is_some() {
                            panic!("Duplicate `extend_type` attribute defined for a single macro call")
                        }
                        extend_type = paths.first().unwrap().get_ident().cloned();
                    // }
                    //  else if m.path().is_ident("consumer_imports") {
                    //     if !consumer_imports.is_empty() {
                    //         panic!("Duplicate `consumer_imports` attributes defined for a single macro call")
                    //     }
                    //     consumer_imports = paths
                    } else if m.path().is_ident("raw_types") {
                        if !raw_types.is_empty() {
                            panic!("Duplicate `raw_types` attributes defined for a single macro call")
                        }
                        raw_types = paths
                            .iter()
                            .filter_map(|p| p.get_ident())
                            .cloned()
                            .collect();
                    } else {
                        panic!("Unsupported ffi attribute path: {:?}", m.path());
                    }
                }
                other => panic!("Unsupported ffi attribute type: {:?}", other),
            }
        }
        Self {
            extend_type: extend_type.unwrap(),
            raw_types,
        }
    }
}