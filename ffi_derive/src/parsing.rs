//!
//! Parses the data that we're interested in out of `syn::DeriveInput` syntax tree.
//!

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use syn::{
    Attribute, Field, GenericArgument, Ident, Item, Lit,
    Meta::{List, NameValue, Path},
    NestedMeta,
    NestedMeta::Meta,
    PathArguments, PathSegment, Type,
};

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
            if let List(l) = m {
                if l.path.segments.first().map(|s| s.ident.to_string()) == Some("repr".to_string())
                {
                    if let Meta(m) = l.nested.first().unwrap_or_else(|| panic!(format!("Expected attribute list to include metadata: {:?} to have an identifier.", &l))) {
                        return m.path().segments.first().map(|s| s.ident.to_string()) == Some("C".to_string());
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
    paths.iter().flat_map(|path| {
        let mut file = File::open(path).expect("Unable to open file");
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

/// Figures out the names and types of all of the arguments in the custom FFI initializer and
/// getters for `type_name` at `path`.
///
/// Returns a tuple of:
/// * The initializer's argument names and their types.
/// * The getter functions' names and return types.
///
/// Pretty gross, but should get nuked in DEV-13175 in favor parsing the FFI module into a type.
///
#[allow(clippy::complexity)]
pub(super) fn custom_ffi_types(
    path: &str,
    type_name: &str,
    expected_init: &Ident,
) -> (Vec<(Ident, Type)>, Vec<(Ident, Type)>) {
    let mut file = File::open(path).expect("Unable to open file");
    let mut src = String::new();
    let _ = file.read_to_string(&mut src).expect("Unable to read file");

    let fns: Vec<syn::ItemFn> = syn::parse_file(&src)
        .expect("Unable to parse file")
        .items
        .into_iter()
        .filter_map(|item| {
            if let Item::Fn(f) = item {
                Some(f)
            } else {
                None
            }
        })
        .collect();

    let initializer = fns
        .iter()
        .find(|f| &f.sig.ident == expected_init)
        .unwrap_or_else(|| {
            panic!(
                "No function found with identifier {:?} in file {:?}",
                expected_init, file
            )
        })
        .clone();

    // Make sure the initializer's signature is right.
    if let syn::ReturnType::Type(_, return_type) = &initializer.sig.output {
        assert_eq!(
            return_type.as_ref(),
            &syn::parse_str::<Type>(&format!("*const {}", type_name)).unwrap()
        );
    } else {
        panic!("Couldn't find expected type signature on custom initializer.")
    }

    let init_data: Vec<(Ident, Type)> = initializer
        .sig
        .inputs
        .iter()
        .map(|arg| {
            if let syn::FnArg::Typed(arg) = arg {
                if let syn::Pat::Ident(ident) = arg.pat.as_ref() {
                    return (ident.ident.clone(), *arg.ty.clone());
                }
            }
            panic!("Unsupported initializer argument: {:?}", arg);
        })
        .collect();

    let function_data: Vec<(Ident, Type)> = fns
        .iter()
        .filter_map(|f| {
            if &f.sig.ident == expected_init {
                return None;
            }
            // TODO: Assert that the one and only argument is a `ptr: *const type_name`.
            if let syn::ReturnType::Type(_, return_type) = &f.sig.output {
                return Some((f.sig.ident.clone(), *return_type.clone()));
            }
            panic!("Can't read return type of function: {:?}", f);
        })
        .collect();

    (init_data, function_data)
}

fn parse_ffi_meta(attr: &Attribute) -> Result<Vec<NestedMeta>, ()> {
    if !attr.path.is_ident("ffi") {
        return Ok(Vec::new());
    }

    match attr.parse_meta() {
        Ok(List(meta)) => Ok(meta.nested.into_iter().collect()),
        Ok(other) => {
            panic!("Unexpected meta attribute found: {:?}", other);
        }
        Err(err) => {
            panic!("Error parsing meta attribute: {:?}", err);
        }
    }
}

pub(super) struct StructAttributes {
    pub(super) alias_paths: Vec<String>,
    pub(super) custom_path: Option<String>,
}

pub(super) fn parse_struct_attributes(attrs: &[Attribute]) -> StructAttributes {
    let mut alias_paths = vec![];
    let mut custom_path: Option<String> = None;
    for meta_item in attrs.iter().flat_map(parse_ffi_meta).flatten() {
        match &meta_item {
            Meta(NameValue(m)) if m.path.is_ident("custom") => {
                if let Lit::Str(lit) = &m.lit {
                    custom_path = Some(lit.value());
                }
            }
            Meta(List(l)) if l.path.is_ident("alias_paths") => {
                alias_paths.extend(l.nested.iter().flat_map(parse_alias_paths));
            }
            other => {
                panic!("Unsupported ffi attribute type: {:?}", other);
            }
        }
    }
    StructAttributes {
        alias_paths,
        custom_path,
    }
}

/// Dig the paths out of an attribute argument and collect them into a `Vec<String>`.
///
fn parse_alias_paths(arg: &NestedMeta) -> Vec<String> {
    match arg {
        Meta(_) => {
            panic!("Unexpected meta attribute {:?}", arg);
        }
        NestedMeta::Lit(lit) => {
            if let Lit::Str(lit_str) = lit {
                lit_str
                    .value()
                    .split(',')
                    .map(|s| s.trim_end().trim_start().to_string())
                    .collect()
            } else {
                panic!("Non-string literal attribute: {:?}", lit)
            }
        }
    }
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
            if let List(l) = m {
                if l.path.segments.first().map(|s| s.ident.to_string()) == Some("ffi".to_string()) {
                    if let Some(Meta(Path(path))) = l.nested.first() {
                        if path.segments.first().map(|p| p.ident.to_string())
                            == Some("raw".to_string())
                        {
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
    if let Type::Path(path) = field_type {
        path.path.segments.first().cloned()
    } else {
        None
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::format_ident;
    use std::{env, fs};

    #[test]
    fn test_is_repr_c() {
        let item = match syn::parse_str::<Item>(
            r#"
            #[derive(Clone, Copy, Debug, PartialEq)]
            #[doc = "a doc attr"]
            #[repr(C)]
            struct TestStruct { }
        "#,
        ) {
            Ok(Item::Struct(i)) => i,
            _ => panic!("Unexpected item type"),
        };
        assert!(is_repr_c(&*item.attrs));
    }

    #[test]
    fn test_is_not_repr_c() {
        let item = match syn::parse_str::<Item>(
            r#"
            #[derive(Clone, Copy, Debug, PartialEq)]
            #[doc = "a doc attr"]
            struct TestStruct { }
        "#,
        ) {
            Ok(Item::Struct(i)) => i,
            _ => panic!("Unexpected item type"),
        };
        assert!(!is_repr_c(&*item.attrs));
    }

    #[test]
    fn test_parse_alias_paths() {
        // Seed an alias file.
        let mut dir = env::temp_dir();
        dir.push("aliases.rs");
        let _ = fs::write(
            &dir,
            r#"
        pub type AnotherNameForU8 = u8;
        pub type AnotherNameForF32 = f32;
        pub type AliasedAlias = AnotherNameForF32;
        "#,
        )
        .ok();

        let mut dir2 = env::temp_dir();
        dir2.push("aliases2.rs");
        let _ = fs::write(
            &dir2,
            r#"
            pub type AliasedI128 = i128;
            "#,
        )
        .ok();

        // Parse the alias paths attribute from a struct
        let item_string = format!(
            r#"
            #[ffi(alias_paths("{}, {}"))]
            struct TestStruct {{ }}
            "#,
            dir.to_str().unwrap(),
            dir2.to_str().unwrap(),
        );
        let item = match syn::parse_str::<Item>(&item_string) {
            Ok(Item::Struct(i)) => i,
            _ => panic!("Unexpected item type"),
        };
        let paths = parse_struct_attributes(&item.attrs).alias_paths;

        let expected: HashMap<Ident, Ident> = [
            (format_ident!("AnotherNameForU8"), format_ident!("u8")),
            (format_ident!("AnotherNameForF32"), format_ident!("f32")),
            (
                format_ident!("AliasedAlias"),
                format_ident!("AnotherNameForF32"),
            ),
            (format_ident!("AliasedI128"), format_ident!("i128")),
        ]
        .iter()
        .cloned()
        .collect();
        assert_eq!(expected, type_alias_map(&paths));
    }

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
        .to_owned();
        assert!(is_raw_ffi_field(&field));
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
        .to_owned();
        assert!(!is_raw_ffi_field(&field));
    }

    #[test]
    fn test_no_wrapping_type() {
        let segment = syn::parse_str::<PathSegment>("SomeType").unwrap();
        assert_eq!(
            separate_wrapping_type_from_inner_type(segment),
            (format_ident!("SomeType"), WrappingType::None)
        );
    }

    #[test]
    fn test_wrapping_vec() {
        let segment = syn::parse_str::<PathSegment>("Vec<SomeType>").unwrap();
        assert_eq!(
            separate_wrapping_type_from_inner_type(segment),
            (format_ident!("SomeType"), WrappingType::Vec)
        );
    }

    #[test]
    fn test_wrapping_option() {
        let segment = syn::parse_str::<PathSegment>("Option<SomeType>").unwrap();
        assert_eq!(
            separate_wrapping_type_from_inner_type(segment),
            (format_ident!("SomeType"), WrappingType::Option)
        );
    }

    #[test]
    fn test_wrapping_option_vec() {
        let segment = syn::parse_str::<PathSegment>("Option<Vec<SomeType>>").unwrap();
        assert_eq!(
            separate_wrapping_type_from_inner_type(segment),
            (format_ident!("SomeType"), WrappingType::OptionVec)
        );
    }
}
