use syn::{Ident, Meta, NestedMeta, Path};

pub struct ImplAttributes {
    pub ffi_imports: Vec<Path>,
    pub consumer_imports: Vec<Path>,
    pub raw_types: Vec<Ident>,
}

impl From<syn::AttributeArgs> for ImplAttributes {
    fn from(args: syn::AttributeArgs) -> Self {
        let mut ffi_imports = vec![];
        let mut consumer_imports = vec![];
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
                    if m.path().is_ident("ffi_imports") {
                        if !ffi_imports.is_empty() {
                            panic!("Duplicate `ffi_imports` attributes defined for a single macro call")
                        }
                        ffi_imports = paths
                    } else if m.path().is_ident("consumer_imports") {
                        if !consumer_imports.is_empty() {
                            panic!("Duplicate `consumer_imports` attributes defined for a single macro call")
                        }
                        consumer_imports = paths
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
        ImplAttributes {
            ffi_imports,
            consumer_imports,
            raw_types,
        }
    }
}
