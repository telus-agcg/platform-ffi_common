//!
//! Contains data structures for describing and implementations for parsing a struct's FFI
//! attributes.
//!

use syn::{spanned::Spanned, Attribute, Lit, Meta, NestedMeta, Path};

/// Struct-level FFI helper attributes.
///
pub struct StructAttributes {
    /// Alias modules that are referenced by the types of this struct's fields.
    ///
    pub alias_modules: Vec<String>,
    /// Custom attributes specified when parsing a struct that has a custom (i.e. manually defined)
    /// FFI. This is `None` whenever we're dealing with a normal struct that can be derived through
    /// `struct_ffi::standard`.
    ///
    pub custom_attributes: Option<CustomAttributes>,
    /// Paths that need to be imported into the consumer module.
    ///
    pub consumer_imports: Vec<Path>,
    /// Paths that need to be imported into the FFI module.
    ///
    pub ffi_mod_imports: Vec<Path>,
    /// If true, do not generate a memberwise initializer for this type. Some types only allow
    /// construction via specific APIs that implemenat additional checks; in those cases, a
    /// generated memberwise init bypasses those restrictions.
    ///
    pub forbid_memberwise_init: bool,
}

/// Helper attributes that describe special behavior for structs with a custom FFI.
///
#[derive(Debug, Clone, Default)]
pub struct CustomAttributes {
    /// A collection of paths to functions that can fail (i.e., return a `Result` internally).
    ///
    pub failable_fns: Vec<Path>,
    /// True if the initializer in this custom FFI can fail, otherwise false.
    ///
    pub failable_init: bool,
    /// The path (relative to the crate's root) to the file containing the custom FFI implementation
    /// for the struct.
    ///
    pub path: String,
}

impl From<&[Attribute]> for StructAttributes {
    fn from(attrs: &[Attribute]) -> Self {
        let mut alias_modules = vec![];
        let mut custom_attributes: Option<CustomAttributes> = None;
        let mut consumer_imports = vec![];
        let mut ffi_mod_imports = vec![];
        let mut forbid_memberwise_init = false;
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
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("consumer_imports") => {
                    consumer_imports.extend(
                        l.nested
                            .iter()
                            .filter_map(super::parse_path_from_nested_meta),
                    );
                }
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("ffi_mod_imports") => {
                    ffi_mod_imports.extend(
                        l.nested
                            .iter()
                            .filter_map(super::parse_path_from_nested_meta),
                    );
                }
                NestedMeta::Meta(Meta::Path(m)) if m.is_ident("failable_init") => {
                    let mut c = custom_attributes.unwrap_or_default();
                    c.failable_init = true;
                    custom_attributes = Some(c);
                }
                NestedMeta::Meta(Meta::List(l)) if l.path.is_ident("failable_fns") => {
                    let mut c = custom_attributes.unwrap_or_default();
                    c.failable_fns.extend(
                        l.nested
                            .iter()
                            .filter_map(super::parse_path_from_nested_meta),
                    );
                    custom_attributes = Some(c);
                }
                NestedMeta::Meta(Meta::Path(m)) if m.is_ident("forbid_memberwise_init") => {
                    forbid_memberwise_init = true;
                }
                other => {
                    proc_macro_error::abort!(other.span(), "Unsupported ffi attribute -- only `custom`, `alias_modules`, `consumer_imports`, `ffi_mod_imports`, `failable_init`, `failable_fns`, and `forbid_memberwise_init` are allowed in this position.");
                }
            }
        }
        Self {
            alias_modules,
            custom_attributes,
            consumer_imports,
            ffi_mod_imports,
            forbid_memberwise_init,
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
