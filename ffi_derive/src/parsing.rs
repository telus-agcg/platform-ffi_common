//!
//! Parses the data that we're interested in out of `syn::DeriveInput` syntax tree.
//!

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use syn::{
    Attribute, Field, GenericArgument, Ident, Item, Lit, Meta, NestedMeta, PathArguments,
    PathSegment, Type,
};

// TODO
// Parsing is really naive for now; we're digging into the ast to find pretty specific info. It's
// probably worth writing a more complete parser (similar to serde's derive; basically parse the ast
// into a custom struct that represents all of the information we care about) so that we can support
// a larger subset of the language features.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum WrappingType {
    /// An `Option<T>`.
    Option,
    /// A `Vec<T>`.
    Vec,
    /// An `Option<Vec<T>>`. We support this because it's required by some services. In general,
    /// optional collections should be avoided because empty and nil almost always mean the same
    /// thing.
    OptionVec,
    /// A `T`.
    None,
}

/// Returns true if an element of `attrs` marks this item as `repr(C)`. Otherwise, false.
///
pub(super) fn is_repr_c(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.parse_meta().map_or(false, |m| {
            if let Meta::List(l) = m {
                if l.path.segments.first().map(|s| s.ident.to_string()) == Some("repr".to_string())
                {
                    if let NestedMeta::Meta(m) = l.nested.first().unwrap() {
                        return m.path().segments.first().unwrap().ident == "C";
                    }
                }
                false
            } else {
                false
            }
        })
    })
}

/// Build a map of all the typealiases we know about and their underlying types. This is necessary
/// because the ast doesn't have any information other than the typealias, so when we're iterating
/// through a struct's fields, all we see is `GrowerId`, `CommodityId`, etc., when we really need to
/// know that it's a `Uuid` or a `u16` so that we can generate the right FFI for it.
///
pub(super) fn type_alias_map(paths: &[String]) -> HashMap<Ident, Ident> {
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    paths.iter().flat_map(|path| {
        let absolute_path = format!("{}/{}", crate_root, path);
        let mut file = File::open(absolute_path).expect("Unable to open file");
        let mut src = String::new();
        let _ = file.read_to_string(&mut src).expect("Unable to read file");

        syn::parse_file(&src)
            .expect("Unable to parse file")
            .items
            .iter()
            .fold(HashMap::new(), |mut acc, item| {
                if let Item::Type(item_type) = item {
                    if acc.contains_key(&item_type.ident) {
                        panic!("The alias {:?} is defined multiple times. Consider only listing one entry in the `ffi(alias_paths())` attribute, or renaming the duplicate alias.", item_type)
                    }
                    if let Type::Path(p) = &*item_type.ty {
                        if let Some(segment) = p.path.segments.first() {
                            let _ = acc.insert(item_type.ident.clone(), segment.ident.clone());
                        }
                    }
                }
                acc
            })
    })
    .collect()
}

/// Reads through the metadata for each attribute and collects all of the ones marked `ffi`.
///
fn parse_ffi_attributes(attrs: &[Attribute]) -> Vec<syn::MetaList> {
    attrs
        .iter()
        .filter_map(|attr| {
            attr.parse_meta().map_or(None, |m| {
                if let Meta::List(l) = m {
                    if l.path.segments.first().map(|s| s.ident.to_string())
                        != Some("ffi".to_string())
                    {
                        return None;
                    }
                    Some(l)
                } else {
                    None
                }
            })
        })
        .collect()
}

pub(super) fn alias_paths(attrs: &[Attribute]) -> Vec<String> {
    parse_ffi_attributes(attrs)
        .iter()
        .map(|l| l.nested.iter().flat_map(|n| parse_alias_paths(n)).collect())
        .collect()
}

/// Dig the paths out of the attribute argument and collect them into a `Vec<String>`.
///
/// Note that this only takes one arg because we're not supporting multiple `alias_paths` or any
/// additional arguments.
///
fn parse_alias_paths(arg: &NestedMeta) -> Vec<String> {
    if let NestedMeta::Meta(Meta::List(list)) = arg {
        assert_eq!(
            list.path.segments.first().unwrap().ident,
            "alias_paths",
            "Unsupported argument for derive_ffi attribute."
        );
        let paths: Vec<String> = list
            .nested
            .iter()
            .filter_map(|meta| {
                if let NestedMeta::Lit(Lit::Str(path)) = meta {
                    Some(path.value())
                } else {
                    None
                }
            })
            .collect();
        return paths;
    }
    vec![]
}

/// Looks for the `ffi(raw)` helper attribute on `field`. Returns `true` if found, otherwise
/// `false`.
///
/// We use this because we can't know whether a custom type is safe for FFI as a raw type (like a
/// `repr(C)` enum), or unsafe (like a complex struct that would need to be boxed before exposing).
///
/// We could eventually expand this to allow the `ffi` attribute to describe a custom FFI format,
/// along the lines of `serde`'s custom (de)serializer attributes.
///
pub(super) fn is_raw_ffi_field(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .filter_map(|a| a.parse_meta().ok())
        .any(|m| {
            if let Meta::List(l) = m {
                if l.path.segments.first().map(|s| s.ident.to_string()) == Some("ffi".to_string()) {
                    if let NestedMeta::Meta(Meta::Path(path)) = l.nested.first().unwrap() {
                        if path.segments.first().unwrap().ident == "raw" {
                            return true;
                        }
                    }
                }
            }
            false
        })
}

/// Finds the first `PathSegment` for `field_type`.
/// Returns `None` on unsupported types, or types with no path segments.
///
pub(super) fn get_segment_for_field(field_type: &Type) -> Option<PathSegment> {
    // TODO: Do we need to support any other field types? Might just want to panic on others and
    // add support if/when they come up.
    match field_type {
        // syn::Type::Array(_) => {}
        // syn::Type::BareFn(_) => {}
        // syn::Type::Group(_) => {}
        // syn::Type::ImplTrait(_) => {}
        // syn::Type::Infer(_) => {}
        // syn::Type::Macro(_) => {}
        // syn::Type::Never(_) => {}
        // syn::Type::Paren(_) => {}
        Type::Path(path) => {
            // TODO: Any reason to loop through path segments? Might leave it at this until we
            // encounter a type where it's necessary.
            path.path.segments.first().cloned()
        }
        // syn::Type::Ptr(_) => {}
        // syn::Type::Reference(_) => {}
        // syn::Type::Slice(_) => {}
        // syn::Type::TraitObject(_) => {}
        // syn::Type::Tuple(_) => {}
        // syn::Type::Verbatim(_) => {}
        // syn::Type::__Nonexhaustive => {}
        _ => None,
    }
}

/// Given a `PathSegment`, flatten an outer generic (if any) so that we can work with the inner type
/// directly to build the FFI.
///
/// If `field_type_path` describes an `Option<Vec<T>>` (gross and rare, but necessary to support
/// some structures), this will call itself to unwrap `Vec<T>`, then return the `Ident` for `T` and
/// `WrappingType::OptionVec`.
///
pub(super) fn separate_wrapping_type_from_inner_type(
    field_type_path: PathSegment,
) -> (Ident, WrappingType) {
    let wrapping_type = match field_type_path.ident.to_string().as_ref() {
        "Option" => WrappingType::Option,
        "Vec" => WrappingType::Vec,
        _ => {
            return (field_type_path.ident, WrappingType::None);
        }
    };

    match field_type_path.arguments {
        PathArguments::None => panic!("No generic args in an option type...?"),
        PathArguments::AngleBracketed(generic) => {
            // TODO: Do we need to care about lifetimes, bindings, constraints, or constants?
            // I think not...these struct definitions should be pretty simple.
            if let Some(GenericArgument::Type(t)) = generic.args.first() {
                if let Some(inner_segment) = get_segment_for_field(t) {
                    if wrapping_type == WrappingType::Option && inner_segment.ident == "Vec" {
                        let unwrapped =
                            separate_wrapping_type_from_inner_type(inner_segment.clone());
                        assert!(
                            unwrapped.1 == WrappingType::Vec,
                            format!("Expected Vec<T>, found {:?}", inner_segment)
                        );
                        (unwrapped.0, WrappingType::OptionVec)
                    } else {
                        (inner_segment.ident, wrapping_type)
                    }
                } else {
                    panic!("Unsupported path type in generic position")
                }
            } else {
                panic!("No generic args...?")
            }
        }
        PathArguments::Parenthesized(_) => panic!("Parenthesized path args are not supported."),
    }
}
