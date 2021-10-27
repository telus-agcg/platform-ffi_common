//!
//! Parses the data that we're interested in out of `syn::DeriveInput` syntax tree.
//!

use proc_macro_error::{abort, OptionExt, ResultExt};
use std::fs::File;
use std::io::Read;
use syn::{
    spanned::Spanned, Attribute, GenericArgument, Ident, Item, Meta, NestedMeta, Path,
    PathArguments, PathSegment, Type,
};

mod field_attributes;
mod fn_attributes;
mod impl_attributes;
mod struct_attributes;
mod type_attributes;

pub use field_attributes::FieldAttributes;
pub use fn_attributes::FnAttributes;
pub use impl_attributes::ImplAttributes;
pub use struct_attributes::{CustomAttributes, StructAttributes};
pub use type_attributes::TypeAttributes;

/// If the path of the `Attribute` parameter is `"ffi"`, this will return a Vec of the attribute's
/// `NestedMeta` data. If other types of data are found in an `"ffi"` attribute, this will panic.
///
fn parse_ffi_meta(attr: &Attribute) -> Vec<NestedMeta> {
    if !attr.path.is_ident("ffi") {
        return Vec::new();
    }

    match attr.parse_meta() {
        Ok(Meta::List(meta)) => meta.nested.into_iter().collect(),
        Ok(other) => {
            abort!(other.span(), "Unexpected meta attribute.")
        }
        Err(err) => {
            abort!(attr.span(), "Error parsing meta attribute: {}.", err)
        }
    }
}

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
#[must_use]
pub fn is_repr_c(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.parse_meta().map_or(false, |m| {
            if let Meta::List(l) = m {
                if l.path.segments.first().map(|s| s.ident.to_string()) == Some("repr".to_string())
                {
                    if let NestedMeta::Meta(m) = l
                        .nested
                        .first()
                        .expect_or_abort("Expected `repr` attribute to have a nested identifier.")
                    {
                        return m.path().segments.first().map(|s| s.ident.to_string())
                            == Some("C".to_string());
                    }
                }
                false
            } else {
                false
            }
        })
    })
}

/// Filters and clones a slice of attributes into a `Vec<Attribute>` containing doc comment
/// attributes only.
///
#[must_use]
pub fn clone_doc_comments(attrs: &[Attribute]) -> Vec<Attribute> {
    let doc_path: Path = syn::parse_str("doc").unwrap_or_abort();
    attrs
        .iter()
        .filter_map(|attr| {
            if attr.path != doc_path {
                return None;
            }
            Some(attr.clone())
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
#[must_use]
#[allow(clippy::complexity)]
pub fn parse_custom_ffi_type(
    path: &str,
    type_name: &str,
    expected_init: &Ident,
) -> (Vec<(Ident, Type)>, Vec<(Ident, Type)>) {
    let mut file = File::open(path).unwrap_or_else(|err| {
        proc_macro_error::abort_call_site!("Unable to open file {:?} with error {}", path, err)
    });
    let mut src = String::new();
    let _ = file.read_to_string(&mut src).unwrap_or_else(|err| {
        proc_macro_error::abort_call_site!(
            "Unable to read file at path {:?} with error {}",
            path,
            err
        )
    });

    let fns: Vec<syn::ItemFn> = syn::parse_file(&src)
        .expect_or_abort("Could not parse file.")
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
        .expect_or_abort(&format!(
            "No function found with identifier {:?} in file {:?}",
            expected_init, file
        ))
        .clone();

    // Make sure the initializer's signature is right.
    if let syn::ReturnType::Type(_, return_type) = &initializer.sig.output {
        let expected_return_type = &syn::parse_str::<Type>(&format!("*const {}", type_name))
            .expect_or_abort("Error parsing expected signature");
        if return_type.as_ref() != expected_return_type {
            abort!(
                return_type.span(),
                "Expected return type {:?}",
                expected_return_type
            )
        }
    } else {
        proc_macro_error::abort_call_site!(
            "Couldn't find expected type signature on custom initializer in file {:?}.",
            file
        )
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
            abort!(arg.span(), "Unsupported initializer argument: {:?}", arg)
        })
        .collect();

    let function_data: Vec<(Ident, Type)> = fns
        .iter()
        .filter_map(|f| {
            if &f.sig.ident == expected_init {
                return None;
            }
            let expected_arg = syn::parse_str::<syn::FnArg>(&format!("ptr: *const {}", type_name)).unwrap_or_abort();
            if f.sig.inputs.len() != 1 || f.sig.inputs.first().expect_or_abort("") != &expected_arg {
                abort!(f.sig.span(), "Non-initializer functions in the custom FFI module must take exactly one `ptr: *const TypeName` argument. Found:\n\n {:?}", f.sig.inputs);
            }
            if let syn::ReturnType::Type(_, return_type) = &f.sig.output {
                return Some((f.sig.ident.clone(), *return_type.clone()));
            }
            abort!(f.span(), "Can't read return type of function: {:?}", f);
        })
        .collect();

    (init_data, function_data)
}

/// Dig the `Meta::Path` out of a `NestedMeta` if present, and return the `Path`.
///
pub(super) fn parse_path_from_nested_meta(arg: &NestedMeta) -> Option<Path> {
    if let NestedMeta::Meta(Meta::Path(path)) = arg {
        return Some(path.clone());
    }
    None
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
        PathArguments::None => abort!(
            field_type_path.span(),
            "No generic args in an option type...?"
        ),
        PathArguments::AngleBracketed(generic) => {
            if let Some(GenericArgument::Type(t)) = generic.args.first() {
                if let Some(inner_segment) = get_segment_for_field(t) {
                    if wrapping_type == WrappingType::Option && inner_segment.ident == "Vec" {
                        let unwrapped =
                            separate_wrapping_type_from_inner_type(inner_segment.clone());
                        assert!(
                            unwrapped.1 == WrappingType::Vec,
                            "Expected Vec<T>, found {:?}",
                            inner_segment
                        );
                        (unwrapped.0, WrappingType::OptionVec)
                    } else {
                        (inner_segment.ident, wrapping_type)
                    }
                } else {
                    abort!(t.span(), "Unsupported path type in generic position")
                }
            } else {
                abort!(generic.span(), "No generic args...?")
            }
        }
        PathArguments::Parenthesized(_) => abort!(
            field_type_path.span(),
            "Parenthesized path args are not supported."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::format_ident;

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
