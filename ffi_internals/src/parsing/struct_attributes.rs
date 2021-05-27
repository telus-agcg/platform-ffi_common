use syn::{Attribute, NestedMeta, Meta, Lit, Path};

pub struct StructAttributes {
    pub alias_modules: Vec<String>,
    pub custom_path: Option<String>,
    pub required_imports: Vec<Path>,
}

impl From<&[Attribute]> for StructAttributes {
    fn from(attrs: &[Attribute]) -> Self {
        let mut alias_modules = vec![];
        let mut custom_path: Option<String> = None;
        let mut required_imports = vec![];
        for meta_item in attrs.iter().flat_map(super::parse_ffi_meta).flatten() {
            match &meta_item {
                NestedMeta::Meta(Meta::NameValue(m)) if m.path.is_ident("custom") => {
                    if let Lit::Str(lit) = &m.lit {
                        custom_path = Some(lit.value());
                    }
                }
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("alias_modules") => {
                    alias_modules.extend(l.nested.iter().flat_map(get_modules_from_meta));
                }
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("required_imports") => {
                    required_imports.extend(l.nested.iter().flat_map(super::parse_path_from_nested_meta));
                }
                other => {
                    panic!("Unsupported ffi attribute type: {:?}", other);
                }
            }
        }
        StructAttributes {
            alias_modules,
            custom_path,
            required_imports,
        }
    }
}

/// Parses the path from a `NestedMeta`, then converts its segments into a `Vec<String>`.
/// 
fn get_modules_from_meta(meta: &NestedMeta) -> Vec<String> {
    super::parse_path_from_nested_meta(meta)
        .map(|p| {
            p.segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect()
        })
        .unwrap_or_default()
}
