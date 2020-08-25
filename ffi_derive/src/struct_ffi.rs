//!
//! Creates an FFI module for a struct, exposing its fields as C getter functions.
//!

use crate::{parsing, parsing::WrappingType};
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{Field, Fields, Ident};

/// Builds an FFI module for the struct `type_name`.
///
pub(super) fn build(
    module_name: &Ident,
    type_name: &Ident,
    fields: &Fields,
    alias_map: &HashMap<Ident, Ident>,
) -> TokenStream {
    let fields = match fields {
        Fields::Named(named) => named,
        // Do we care about unnamed or unit fields on resource types? I think not, at least
        // until we have a case for it.
        Fields::Unnamed(_) => panic!("Unnamed fields are not supported"),
        Fields::Unit => panic!("Unit fields are not supported"),
    };

    let (init_arguments, argument_mapping, attribute_fns) =
        fields
            .named
            .iter()
            .fold((quote!(), quote!(), quote!()), |mut acc, field| {
                let field_ffi = build_field_ffi(type_name, field, alias_map);
                acc.0.extend(field_ffi.0);
                acc.1.extend(field_ffi.1);
                acc.2.extend(field_ffi.2);
                acc
            });

    let free_fn_name = Ident::new(
        &[&type_name.to_string().to_snake_case(), "_free"].concat(),
        type_name.span(),
    );

    let init_fn_name = Ident::new(
        &[&type_name.to_string().to_snake_case(), "_init"].concat(),
        type_name.span(),
    );

    // Create a new module for the FFI for this type.
    quote!(
        #[allow(box_pointers)]
        #[allow(missing_docs)]
        pub mod #module_name {
            use ffi_common::{*, string::FFIArrayString, datetime::*};
            use std::os::raw::c_char;
            use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
            use paste::paste;
            use uuid::Uuid;
            use super::*;

            #[no_mangle]
            pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                ffi_common::error::clear_last_err_msg();
                let _ = Box::from_raw(data as *mut #type_name);
            }

            declare_opaque_type_array_struct! { #type_name }

            #[no_mangle]
            pub extern "C" fn #init_fn_name(
                #init_arguments
            ) -> *const #type_name {
                let data = #type_name {
                    #argument_mapping
                };
                Box::into_raw(Box::new(data))
            }

            // Defined here for convenience so other macros can reference it. We could probably
            // move this to ffi_common, though, and let them all reference it from there.
            macro_rules! ffi_string {
                ($string:expr) => {{
                    ffi_common::error::clear_last_err_msg();
                    let c_string = try_or_set_error!(CString::new($string));
                    let c: *const c_char = c_string.into_raw();
                    c
                }}
            }

            #attribute_fns
        }
    )
}

/// Build an FFI getter function (figuring out the intermediate stuff like the field's real `Ident`,
/// whether it's an `Option`, etc).
///
fn build_field_ffi(
    type_name: &Ident,
    field: &Field,
    alias_map: &HashMap<Ident, Ident>,
) -> (TokenStream, TokenStream, TokenStream) {
    let field_name = field.ident.as_ref().unwrap();
    let (wrapping_type, unaliased_field_type) = match parsing::get_segment_for_field(&field.ty) {
        Some(segment) => {
            let (ident, wrapping_type) = parsing::separate_wrapping_type_from_inner_type(segment);
            (wrapping_type, resolve_type_alias(&ident, alias_map))
        }
        None => panic!("No path segment (field without a type?)"),
    };
    match unaliased_field_type.to_string().as_str() {
        "String" => generate_string_ffi(type_name, field_name, wrapping_type, false),
        "Uuid" => generate_string_ffi(type_name, field_name, wrapping_type, true),
        // Long discussion, but Rust's bool should be safe for C FFI:
        // https://github.com/rust-lang/rust/pull/46176
        // TODO: Sounds like Mozilla has had bool issues on Android with JNA, though, so maybe we
        // want to use a u8 that's always 0 or 1:
        // https://github.com/mozilla/application-services/blob/main/docs/howtos/when-to-use-what-in-the-ffi.md#primitives
        "bool" | "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" => {
            generate_raw_ffi(type_name, field_name, &unaliased_field_type, wrapping_type)
        }
        "NaiveDateTime" => generate_datetime_ffi(type_name, field_name, wrapping_type),
        _ => {
            // Other fields should be marked as `ffi(raw)` if they are repr(C) types that can be
            // returned directly. Otherwise, all other fields will be returned as boxed types.
            if parsing::is_raw_ffi_field(field) {
                generate_raw_ffi(type_name, field_name, &unaliased_field_type, wrapping_type)
            } else {
                generate_boxed_ffi(type_name, field_name, &unaliased_field_type, wrapping_type)
            }
        }
    }
}

/// If `field_type` is an alias in `alias_map`, returns the underlying type (resolving aliases
/// recursively, so if someone is weird and defines typealiases over other typealiases, we'll still
/// find the underlying type, as long as they were all specified in `alias_paths`).
///
fn resolve_type_alias(field_type: &Ident, alias_map: &HashMap<Ident, Ident>) -> Ident {
    match alias_map.get(field_type) {
        Some(alias) => resolve_type_alias(alias, alias_map),
        None => field_type.clone(),
    }
}

/// Code for getting a `Uuid` from an FFI string.
fn uuid_from_c(field_name: &Ident) -> TokenStream {
    quote! {
        unsafe {
            Uuid::parse_str(CStr::from_ptr(#field_name as *mut c_char).to_str().unwrap()).unwrap()
        }
    }
}

/// Code for getting a `String` from an FFI string.
fn string_from_c(field_name: &Ident) -> TokenStream {
    quote! {
        unsafe {
            CStr::from_ptr(#field_name as *mut c_char).to_str().unwrap().to_string()
        }
    }
}

/// For a string-like field (a `String`, `Uuid`, etc.), including any supported `WrappingType`, this
/// generates the following:
/// 1. An initializer argument of the field's name and its FFI-safe type.
/// 1. An assignment expression where the initializer argument is converted to the native type of
/// the field and assigned.
/// 1. An extern "C" getter function for reading the value of this field from across the FFI, which
/// takes a pointer to an instance of the type to which this field belongs, and converts the value
/// of the field to an FFI-safe type.
///
fn generate_string_ffi(
    type_name: &Ident,
    field_name: &Ident,
    wrapping_type: WrappingType,
    is_uuid_field: bool,
) -> (TokenStream, TokenStream, TokenStream) {
    let assignment;
    let arg;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = quote! { *const c_char };
            arg = quote!(#field_name: #ffi_type,);
            let parse_some = if is_uuid_field {
                uuid_from_c(field_name)
            } else {
                string_from_c(field_name)
            };
            assignment = quote! {
                #field_name: if #field_name.is_null() {
                    None
                } else {
                    Some(#parse_some)
                },
            };
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_optional_ #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        data.#field_name.as_ref().map_or(ptr::null(), |s| {
                            ffi_string!(s.to_string())
                        })
                    }
                }
            }
        }
        WrappingType::Vec | WrappingType::OptionVec => {
            let prefix = if wrapping_type == WrappingType::Vec {
                "get_"
            } else {
                "get_optional_"
            };
            let ffi_type = quote! { FFIArrayString };
            arg = quote!( #field_name: #ffi_type, );
            assignment = quote!(#field_name: #field_name.into(),);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<#prefix #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        let v = &data.#field_name;
                        v.into()
                    }
                }
            }
        }
        WrappingType::None => {
            let ffi_type = quote! { *const c_char };
            arg = quote!( #field_name: #ffi_type, );
            let parse_field = if is_uuid_field {
                uuid_from_c(field_name)
            } else {
                string_from_c(field_name)
            };
            assignment = quote!(#field_name: #parse_field,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_ #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        let data = &*ptr;
                        ffi_string!(data.#field_name.to_string())
                    }
                }
            }
        }
    };
    (arg, assignment, getter)
}

/// For a field whose type is FFI-safe without conversion/wrapping (a `u16`, a `repr(C)` enum,
/// etc.), including any supported `WrappingType`, this generates the following:
/// 1. An initializer argument of the field's name and type (or FFI-safe type, depending on
/// `WrappingType`).
/// 1. An assignment expression where the initializer argument is the field's type (or its FFI-safe
/// type, depending on `WrappingType`).
/// 1. An extern "C" getter function for reading the value of this field from across the FFI, which
/// takes a pointer to an instance of the type to which this field belongs, and returns the value of
/// the field (or its FFI-safe type, depending on `WrappingType`).
///
fn generate_raw_ffi(
    type_name: &Ident,
    field_name: &Ident,
    field_type: &Ident,
    wrapping_type: WrappingType,
) -> (TokenStream, TokenStream, TokenStream) {
    let assignment;
    let arg;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = quote! { paste!([<Option #field_type:camel>]) };
            assignment = quote!(#field_name: #field_name.into(),);
            arg = quote!(#field_name: #ffi_type,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_optional_ #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        let opt = &data.#field_name;
                        opt.into()
                    }
                }
            }
        }
        WrappingType::Vec | WrappingType::OptionVec => {
            let prefix = if wrapping_type == WrappingType::Vec {
                "get_"
            } else {
                "get_optional_"
            };
            let ffi_type = quote! { paste!([<FFIArray #field_type:camel>]) };
            assignment = quote!(#field_name: #field_name.into(),);
            arg = quote!(#field_name: #ffi_type,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get the `" #field_name "` for a `" #type_name"`."]
                    pub unsafe extern "C" fn [<#prefix #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        let v = &data.#field_name;
                        v.into()
                    }
                }
            }
        }
        WrappingType::None => {
            assignment = quote!(#field_name: #field_name,);
            arg = quote!(#field_name: #field_type,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_ #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #field_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        data.#field_name.clone()
                    }
                }
            }
        }
    };
    (arg, assignment, getter)
}

/// For a `NaiveDateTime`, including any supported `WrappingType`, this generates the following:
/// 1. An initializer argument of the field's name and its FFI-safe type.
/// 1. An assignment expression where the initializer argument is converted to the native type of
/// the field and assigned.
/// 1. An extern "C" getter function for reading the value of this field from across the FFI, which
/// takes a pointer to an instance of the type to which this field belongs, and converts the value
/// of the field to an FFI-safe type.
///
fn generate_datetime_ffi(
    type_name: &Ident,
    field_name: &Ident,
    wrapping_type: WrappingType,
) -> (TokenStream, TokenStream, TokenStream) {
    let assignment;
    let arg;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = quote! { paste!([<Option TimeStamp>]) };
            assignment = quote!(#field_name: #field_name.into(),);
            arg = quote!(#field_name: #ffi_type,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_optional_ #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        let opt = &data.#field_name;
                        opt.into()
                    }
                }
            }
        }
        WrappingType::Vec | WrappingType::OptionVec => {
            let prefix = if wrapping_type == WrappingType::Vec {
                "get_"
            } else {
                "get_optional_"
            };
            let ffi_type = quote! { paste!([<FFIArray TimeStamp>]) };
            assignment = quote!(#field_name: #field_name.into(),);
            arg = quote!(#field_name: #ffi_type,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get the `" #field_name "` for a `" #type_name"`."]
                    pub unsafe extern "C" fn [<#prefix #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        let v = &data.#field_name;
                        v.into()
                    }
                }
            }
        }
        WrappingType::None => {
            assignment = quote!(#field_name: #field_name.into(),);
            arg = quote!(#field_name: TimeStamp,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_ #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> TimeStamp {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        (&data.#field_name).into()
                    }
                }
            }
        }
    };
    (arg, assignment, getter)
}

/// For any field whose type cannot be represented to the FFI in a meaningful way, we `Box` the Rust
/// representation of that type, which generates the following:
/// 1. An initializer argument of the field's name and its FFI-safe type.
/// 1. An assignment expression where the initializer argument is "unboxed", consuming the raw
/// reference and assigning it to the field.
/// 1. An extern "C" getter function for reading the value of this field from across the FFI, which
/// takes a pointer to an instance of the type to which this field belongs, and converts the value
/// of the field to an FFI-safe type.
///
fn generate_boxed_ffi(
    type_name: &Ident,
    field_name: &Ident,
    field_type: &Ident,
    wrapping_type: WrappingType,
) -> (TokenStream, TokenStream, TokenStream) {
    let assignment;
    let arg;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = quote! { *const #field_type };
            assignment = quote! {
                #field_name: unsafe {
                    if #field_name.is_null() {
                        None
                    } else {
                        Some(*Box::from_raw(#field_name as *mut #field_type))
                    }
                },
            };
            arg = quote!( #field_name: #ffi_type, );
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_optional_ #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        data.#field_name.as_ref().map_or(ptr::null(), |f| {
                            Box::into_raw(Box::new(f.clone()))
                        })
                    }
                }
            }
        }
        WrappingType::Vec | WrappingType::OptionVec => {
            let prefix = if wrapping_type == WrappingType::Vec {
                "get_"
            } else {
                "get_optional_"
            };
            let ffi_type = quote! { paste!([<FFIArray #field_type:camel>]) };
            assignment = quote!(#field_name: #field_name.into(),);
            arg = quote!(#field_name: #ffi_type,);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get the `" #field_name "` for a `" #type_name"`."]
                    pub unsafe extern "C" fn [<#prefix #type_name:snake _ #field_name>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        let v = &data.#field_name;
                        v.into()
                    }
                }
            }
        }
        WrappingType::None => {
            let ffi_type = quote! { *const #field_type };
            assignment =
                quote!(#field_name: unsafe { *Box::from_raw(#field_name as *mut #field_type) },);
            arg = quote!( #field_name: #ffi_type, );
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn [<get_ #type_name:snake _ #field_name:snake>](
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        ffi_common::error::clear_last_err_msg();
                        let data = &*ptr;
                        Box::into_raw(Box::new(data.#field_name.clone()))
                    }
                }
            }
        }
    };
    (arg, assignment, getter)
}
