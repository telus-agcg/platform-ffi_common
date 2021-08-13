use syn::{Attribute, Lit, Meta, NestedMeta, Path, spanned::Spanned};

pub struct StructAttributes {
    pub alias_modules: Vec<String>,
    pub custom_attributes: Option<CustomAttributes>,
    pub required_imports: Vec<Path>,
}

#[derive(Debug, Clone, Default)]
pub struct CustomAttributes {
    pub failable_fns: Vec<Path>,
    pub failable_init: bool,
    pub path: String,
}

impl From<&[Attribute]> for StructAttributes {
    fn from(attrs: &[Attribute]) -> Self {
        let mut alias_modules = vec![];
        let mut custom_attributes: Option<CustomAttributes> = None;
        let mut required_imports = vec![];
        for meta_item in attrs.iter().flat_map(super::parse_ffi_meta) {
            match &meta_item {
                NestedMeta::Meta(Meta::NameValue(m)) if m.path.is_ident("custom") => {
                    if let Lit::Str(lit) = &m.lit {
                        let mut c = custom_attributes.unwrap_or_default();
                        c.path = lit.value();
                        custom_attributes = Some(c);
                    }
                }
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("alias_modules") => {
                    alias_modules.extend(l.nested.iter().flat_map(get_modules_from_meta));
                }
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("required_imports") => {
                    required_imports
                        .extend(l.nested.iter().filter_map(super::parse_path_from_nested_meta));
                }
                NestedMeta::Meta(Meta::Path(m)) if m.is_ident("failable_init") => {
                    let mut c = custom_attributes.unwrap_or_default();
                    c.failable_init = true;
                    custom_attributes = Some(c);
                }
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("failable_fns") => {
                    let mut c = custom_attributes.unwrap_or_default();
                    c.failable_fns.extend(l.nested.iter().filter_map(super::parse_path_from_nested_meta));
                    custom_attributes = Some(c);
                }
                other => {
                    proc_macro_error::abort!(other.span(), "Unsupported ffi attribute -- only `custom`, `alias_modules`, `required_imports`, `failable_init`, and `failable_fns` are allowed in this position.");
                }
            }
        }
        Self {
            alias_modules,
            custom_attributes,
            required_imports,
        }
    }
}

/// Parses the path from a `NestedMeta`, then converts its segments into a `Vec<String>`.
///
fn get_modules_from_meta(meta: &NestedMeta) -> Vec<String> {
    super::parse_path_from_nested_meta(meta)
        .map(|p| p.segments.iter().map(|s| s.ident.to_string()).collect())
        .unwrap_or_default()
}
