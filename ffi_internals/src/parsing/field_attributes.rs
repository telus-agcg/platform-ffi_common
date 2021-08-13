use proc_macro_error::emit_error;
use syn::{spanned::Spanned, Attribute, Ident, Lit, Meta, NestedMeta, Path};

/// Field-level FFI helper attributes.
///
#[derive(Debug, Clone)]
pub struct FieldAttributes {
    /// If `Some`, a path to the type that this field should be exposed as. This type must meet
    /// some prerequisites:
    /// 1. It must be FFI-safe (either because it's a primitive value or derives its own FFI with
    /// `ffi_derive`).
    /// 1. It must have a `From<T> for U` impl, where `T` is the native type of the field and `U` is
    /// the type referenced by the `expose_as` `Path`.
    ///
    /// This is necessary for exposing remote types where we want to derive an FFI, but don't
    /// control the declaration of the type.
    ///
    pub expose_as: Option<Path>,

    /// Whether the field's data should be exposed as a raw value (i.e., not `Box`ed). This should
    /// only be applied to fields whose type is `repr(C)` and safe to expose over FFI.
    ///
    pub raw: bool,
}

impl FieldAttributes {
    /// If there's an `expose_as` attribute, get the ident of the last segment in the path (i.e.,
    /// the ident of the type being referenced).
    ///
    #[must_use]
    pub fn expose_as_ident(&self) -> Option<&Ident> {
        self.expose_as
            .as_ref()
            .and_then(|p| p.segments.last().map(|s| &s.ident))
    }
}

impl From<&[Attribute]> for FieldAttributes {
    fn from(attrs: &[Attribute]) -> Self {
        let mut expose_as: Option<Path> = None;
        let mut raw = false;
        for meta_item in attrs.iter().flat_map(super::parse_ffi_meta) {
            match &meta_item {
                NestedMeta::Meta(Meta::NameValue(m)) if m.path.is_ident("expose_as") => {
                    if let Lit::Str(lit) = &m.lit {
                        expose_as = Some(syn::parse_str(&lit.value()).expect("Not a valid path"));
                    }
                }
                NestedMeta::Meta(Meta::Path(p)) if p.is_ident("raw") => {
                    raw = true;
                }
                _other => {
                    emit_error!(meta_item.span(), "Unsupported ffi attribute -- only `raw` and `expose_as` are valid in this position");
                }
            }
        }
        Self { expose_as, raw }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::Item;

    #[test]
    fn test_is_raw_ffi_field() {
        let item = match syn::parse_str::<Item>(
            r#"
            #[derive(Clone, Copy, Debug, PartialEq)]
            #[doc = "a doc attr"]
            #[repr(C)]
            struct TestStruct {
                #[ffi(raw)]
                test_field: CustomReprCType
            }
        "#,
        ) {
            Ok(Item::Struct(i)) => i,
            _ => panic!("Unexpected item type"),
        };
        let field = match item.fields {
            syn::Fields::Named(n) => n,
            _ => panic!("Unexpected field type"),
        }
        .named
        .first()
        .expect("Failed to parse field")
        .clone();
        assert!(FieldAttributes::from(&*field.attrs).raw);
    }

    #[test]
    fn test_is_not_raw_ffi_field() {
        let item = match syn::parse_str::<Item>(
            r#"
                #[derive(Clone, Copy, Debug, PartialEq)]
                #[doc = "a doc attr"]
                #[repr(C)]
                struct TestStruct {
                    test_field: CustomNonReprCType
                }
            "#,
        ) {
            Ok(Item::Struct(i)) => i,
            _ => panic!("Unexpected item type"),
        };
        let field = match item.fields {
            syn::Fields::Named(n) => n,
            _ => panic!("Unexpected field type"),
        }
        .named
        .first()
        .expect("Failed to parse field")
        .clone();
        assert!(!FieldAttributes::from(&*field.attrs).raw);
    }
}
