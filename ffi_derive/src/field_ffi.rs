use crate::{parsing, parsing::WrappingType};
use ffi_common::{codegen_helpers::{FieldFFI, FieldType}};
use std::collections::HashMap;
use syn::{Field, Ident};

pub(super) fn generate(
    type_name: Ident,
    field: &Field,
    alias_map: &HashMap<Ident, Ident>,
) -> FieldFFI {
    let field_name = field.ident.as_ref().unwrap_or_else(|| {
        panic!(format!(
            "Expected field: {:?} to have an identifier.",
            &field
        ))
    }).clone();
    let attributes = parsing::parse_fn_attributes(&field.attrs);
    let (wrapping_type, unaliased_field_type) = match parsing::get_segment_for_field(&field.ty) {
        Some(segment) => {
            let (ident, wrapping_type) = parsing::separate_wrapping_type_from_inner_type(segment);
            (wrapping_type, resolve_type_alias(&ident, alias_map))
        }
        None => panic!("No path segment (field without a type?)"),
    };

    // If this has a raw attribute, bypass the normal `FieldType` logic and use `FieldType::raw`.
    let field_type = if attributes.raw {
        FieldType::Raw(unaliased_field_type)
    } else {
        FieldType::from(unaliased_field_type)
    };

    FieldFFI {
        type_name,
        field_name,
        field_type,
        attributes,
        option: wrapping_type == WrappingType::Option || wrapping_type == WrappingType::OptionVec,
        vec: wrapping_type == WrappingType::Vec || wrapping_type == WrappingType::OptionVec,
    }
}

/// If `field_type` is an alias in `alias_map`, returns the underlying type (resolving aliases
/// recursively, so if someone is weird and defines typealiases over other typealiases, we'll still
/// find the underlying type, as long as they were all specified in the `alias_paths` helper
/// attribute).
///
fn resolve_type_alias(field_type: &Ident, alias_map: &HashMap<Ident, Ident>) -> Ident {
    match alias_map.get(field_type) {
        Some(alias) => resolve_type_alias(alias, alias_map),
        None => field_type.clone(),
    }
}
