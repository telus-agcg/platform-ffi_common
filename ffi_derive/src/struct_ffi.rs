//!
//! Creates an FFI module for a struct, exposing its fields as C getter functions.
//!

use crate::{parsing, parsing::WrappingType};
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
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

    let free_fn_name = format_ident!("{}_free", &type_name.to_string().to_snake_case());
    let init_fn_name = format_ident!("{}_init", &type_name.to_string().to_snake_case());

    // Create a new module for the FFI for this type.
    quote!(
        #[allow(box_pointers, missing_docs)]
        pub mod #module_name {
            use ffi_common::{*, datetime::*, ffi_string, string::FFIArrayString};
            use std::os::raw::c_char;
            use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
            use paste::paste;
            use uuid::Uuid;
            use super::#type_name;

            #[no_mangle]
            pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
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
    let field_name = field.ident.as_ref().unwrap_or_else(|| {
        panic!(format!(
            "Expected field: {:?} to have an identifier.",
            &field
        ))
    });
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
            Uuid::parse_str(&CStr::from_ptr(#field_name).to_string_lossy()).unwrap()
        }
    }
}

/// Code for getting a `String` from an FFI string.
fn string_from_c(field_name: &Ident) -> TokenStream {
    quote! {
        unsafe {
            CStr::from_ptr(#field_name).
            to_string_lossy().into_owned()
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
                        let data = &*ptr;
                        data.#field_name.as_ref().map_or(ptr::null(), |s| {
                            ffi_string!(s.to_string())
                        })
                    }
                }
            }
        }
        WrappingType::Vec => {
            return vec_field_ffi(type_name, field_name, &quote!(FFIArrayString));
        }
        WrappingType::OptionVec => {
            return option_vec_field_ffi(type_name, field_name, &quote!(FFIArrayString));
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
                        let data = &*ptr;
                        data.#field_name.as_ref().into()
                    }
                }
            }
        }
        WrappingType::Vec => {
            return vec_field_ffi(
                type_name,
                field_name,
                &quote! { paste!([<FFIArray #field_type:camel>]) },
            );
        }
        WrappingType::OptionVec => {
            return option_vec_field_ffi(
                type_name,
                field_name,
                &quote! { paste!([<FFIArray #field_type:camel>]) },
            );
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
                        let data = &*ptr;
                        data.#field_name.as_ref().into()
                    }
                }
            }
        }
        WrappingType::Vec => {
            return vec_field_ffi(
                type_name,
                field_name,
                &quote! { paste!([<FFIArray TimeStamp>]) },
            );
        }
        WrappingType::OptionVec => {
            return option_vec_field_ffi(
                type_name,
                field_name,
                &quote! { paste!([<FFIArray TimeStamp>]) },
            );
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
                        let data = &*ptr;
                        data.#field_name.as_ref().map_or(ptr::null(), |f| {
                            Box::into_raw(Box::new(f.clone()))
                        })
                    }
                }
            }
        }
        WrappingType::Vec => {
            return vec_field_ffi(
                type_name,
                field_name,
                &quote! { paste!([<FFIArray #field_type:camel>]) },
            );
        }
        WrappingType::OptionVec => {
            return option_vec_field_ffi(
                type_name,
                field_name,
                &quote! { paste!([<FFIArray #field_type:camel>]) },
            );
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
                        let data = &*ptr;
                        Box::into_raw(Box::new(data.#field_name.clone()))
                    }
                }
            }
        }
    };
    (arg, assignment, getter)
}

fn vec_field_ffi(
    type_name: &Ident,
    field_name: &Ident,
    ffi_type: &TokenStream,
) -> (TokenStream, TokenStream, TokenStream) {
    let arg = quote!(#field_name: #ffi_type,);
    let assignment = quote!(#field_name: #field_name.into(),);
    let getter = quote! {
        paste! {
            #[no_mangle]
            #[doc = "Get `" #field_name "` for a `" #type_name"`."]
            pub unsafe extern "C" fn [<get_ #type_name:snake _ #field_name>](
                ptr: *const #type_name
            ) -> #ffi_type {
                let data = &*ptr;
                (&*data.#field_name).into()
            }
        }
    };
    (arg, assignment, getter)
}

fn option_vec_field_ffi(
    type_name: &Ident,
    field_name: &Ident,
    ffi_type: &TokenStream,
) -> (TokenStream, TokenStream, TokenStream) {
    let arg = quote!(#field_name: #ffi_type,);
    let assignment = quote!(#field_name: #field_name.into(),);
    let getter = quote! {
        paste! {
            #[no_mangle]
            #[doc = "Get `" #field_name "` for this `" #type_name"`."]
            pub unsafe extern "C" fn [<get_optional_ #type_name:snake _ #field_name>](
                ptr: *const #type_name
            ) -> #ffi_type {
                let data = &*ptr;
                data.#field_name.as_deref().into()
            }
        }
    };
    (arg, assignment, getter)
}
