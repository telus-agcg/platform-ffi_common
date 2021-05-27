use syn::{Path, NestedMeta, Meta};

pub struct ImplAttributes {
    pub ffi_imports: Vec<Path>,
    pub consumer_imports: Vec<Path>,
}

impl From<syn::AttributeArgs> for ImplAttributes {
    fn from(args: syn::AttributeArgs) -> Self {
        let mut ffi_imports = vec![];
        let mut consumer_imports = vec![];
        for arg in args.iter() {
            match arg {
                NestedMeta::Meta(m) if m.path().is_ident("ffi_imports") => {
                    if let Meta::List(l) = m {
                        ffi_imports = l.nested.iter().flat_map(super::parse_path_from_nested_meta).collect();
                    }
                },
                NestedMeta::Meta(m) if m.path().is_ident("consumer_imports") => {
                    if let Meta::List(l) = m {
                        consumer_imports = l.nested.iter().flat_map(super::parse_path_from_nested_meta).collect();
                    }
                },
                other => panic!("Unsupported ffi attribute type: {:?}", other),
            }
        };
        ImplAttributes {
            ffi_imports,
            consumer_imports,
        }
    }
}