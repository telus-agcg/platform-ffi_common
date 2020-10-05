use crate::{parsing, parsing::WrappingType};
use ffi_common::{codegen_helpers, codegen_helpers::FieldFFI};
use heck::{CamelCase, SnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Field, Ident};

pub(super) fn generate(
    type_name: &Ident,
    field: &Field,
    alias_map: &HashMap<Ident, Ident>,
) -> FieldFFI {
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
) -> FieldFFI {
    let argument;
    let assignment_expression;
    let getter_name;
    let consumer_type;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = quote! { *const c_char };
            argument = quote!(#field_name: #ffi_type,);
            let parse_some = if is_uuid_field {
                uuid_from_c(field_name)
            } else {
                string_from_c(field_name)
            };
            assignment_expression = quote! {
                #field_name: if #field_name.is_null() {
                    None
                } else {
                    Some(#parse_some)
                },
            };
            getter_name = getter_ident(type_name, field_name, true);
            consumer_type = "String?".to_string();
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
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
            return vec_field_ffi(
                type_name,
                field_name,
                &format_ident!("FFIArrayString"),
                "[String]".to_string(),
            );
        }
        WrappingType::OptionVec => {
            return option_vec_field_ffi(
                type_name,
                field_name,
                &format_ident!("FFIArrayString"),
                "[String]?".to_string(),
            )
        }
        WrappingType::None => {
            let ffi_type = quote! { *const c_char };
            argument = quote!( #field_name: #ffi_type, );
            let parse_field = if is_uuid_field {
                uuid_from_c(field_name)
            } else {
                string_from_c(field_name)
            };
            assignment_expression = quote!(#field_name: #parse_field,);
            getter_name = getter_ident(type_name, field_name, false);
            consumer_type = "String".to_string();
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        let data = &*ptr;
                        ffi_string!(data.#field_name.to_string())
                    }
                }
            }
        }
    };
    FieldFFI {
        field: field_name.clone(),
        argument,
        assignment_expression,
        getter_name,
        consumer_type,
        getter,
    }
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
) -> FieldFFI {
    let argument;
    let assignment_expression;
    let getter_name;
    let consumer_type;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = quote! { paste!([<Option #field_type:camel>]) };
            argument = quote!(#field_name: #ffi_type,);
            assignment_expression = quote!(#field_name: #field_name.into(),);
            getter_name = getter_ident(type_name, field_name, true);
            consumer_type = codegen_helpers::consumer_type_for(&field_type.to_string(), true);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
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
                &format_ident!("FFIArray{}", field_type.to_string().to_camel_case()),
                format!(
                    "[{}]",
                    codegen_helpers::consumer_type_for(&field_type.to_string(), false)
                ),
            );
        }
        WrappingType::OptionVec => {
            return option_vec_field_ffi(
                type_name,
                field_name,
                &format_ident!("FFIArray{}", field_type.to_string().to_camel_case()),
                format!(
                    "[{}]?",
                    codegen_helpers::consumer_type_for(&field_type.to_string(), false)
                ),
            );
        }
        WrappingType::None => {
            argument = quote!(#field_name: #field_type,);
            assignment_expression = quote!(#field_name: #field_name,);
            getter_name = getter_ident(type_name, field_name, false);
            consumer_type = codegen_helpers::consumer_type_for(&field_type.to_string(), false);
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
                        ptr: *const #type_name
                    ) -> #field_type {
                        let data = &*ptr;
                        data.#field_name.clone()
                    }
                }
            }
        }
    };
    FieldFFI {
        field: field_name.clone(),
        argument,
        assignment_expression,
        getter_name,
        consumer_type,
        getter,
    }
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
) -> FieldFFI {
    let argument;
    let assignment_expression;
    let getter_name;
    let consumer_type;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = format_ident!("OptionTimeStamp");
            argument = quote!(#field_name: #ffi_type,);
            assignment_expression = quote!(#field_name: #field_name.into(),);
            getter_name = getter_ident(type_name, field_name, true);
            consumer_type = "Date?".to_string();
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
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
                &format_ident!("FFIArrayTimeStamp"),
                "[Date]".to_string(),
            );
        }
        WrappingType::OptionVec => {
            return option_vec_field_ffi(
                type_name,
                field_name,
                &format_ident!("FFIArrayTimeStamp"),
                "[Date]?".to_string(),
            );
        }
        WrappingType::None => {
            argument = quote!(#field_name: TimeStamp,);
            assignment_expression = quote!(#field_name: #field_name.into(),);
            getter_name = getter_ident(type_name, field_name, false);
            consumer_type = "Date".to_string();
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
                        ptr: *const #type_name
                    ) -> TimeStamp {
                        let data = &*ptr;
                        (&data.#field_name).into()
                    }
                }
            }
        }
    };
    FieldFFI {
        field: field_name.clone(),
        argument,
        assignment_expression,
        getter_name,
        consumer_type,
        getter,
    }
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
) -> FieldFFI {
    let argument;
    let assignment_expression;
    let getter_name;
    let consumer_type: String;
    let getter = match wrapping_type {
        WrappingType::Option => {
            let ffi_type = quote!(*const #field_type);
            argument = quote!(#field_name: #ffi_type,);
            assignment_expression = quote! {
                #field_name: unsafe {
                    if #field_name.is_null() {
                        None
                    } else {
                        Some((*#field_name).clone())
                    }
                },
            };
            getter_name = getter_ident(type_name, field_name, true);
            consumer_type = format!("{}?", &field_type.to_string());
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
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
            let inner_type = field_type.to_string().to_camel_case();
            return vec_field_ffi(
                type_name,
                field_name,
                &format_ident!("FFIArray{}", inner_type),
                format!("[{}]", inner_type),
            );
        }
        WrappingType::OptionVec => {
            let inner_type = field_type.to_string().to_camel_case();
            return option_vec_field_ffi(
                type_name,
                field_name,
                &format_ident!("FFIArray{}", inner_type),
                format!("[{}]", inner_type),
            );
        }
        WrappingType::None => {
            let ffi_type = quote!(*const #field_type);
            argument = quote!(#field_name: #ffi_type,);
            assignment_expression = quote!(#field_name: unsafe { (*#field_name).clone() },);
            getter_name = getter_ident(type_name, field_name, false);
            consumer_type = field_type.to_string();
            quote! {
                paste! {
                    #[no_mangle]
                    #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                    pub unsafe extern "C" fn #getter_name(
                        ptr: *const #type_name
                    ) -> #ffi_type {
                        let data = &*ptr;
                        Box::into_raw(Box::new(data.#field_name.clone()))
                    }
                }
            }
        }
    };
    FieldFFI {
        field: field_name.clone(),
        argument,
        assignment_expression,
        getter_name,
        consumer_type,
        getter,
    }
}

/// Generates the interface for any `Vec<T>` field. Because they're always represented by some
/// `FFIArrayT` type for FFI, which implements `From<&[T]>`, we end up using `into()` regardless of
/// what `T` is.
///
fn vec_field_ffi(
    type_name: &Ident,
    field_name: &Ident,
    ffi_type: &Ident,
    consumer_type: String,
) -> FieldFFI {
    let argument = quote!(#field_name: #ffi_type,);
    let assignment_expression = quote!(#field_name: (#field_name).into(),);
    let getter_name = getter_ident(type_name, field_name, false);
    let getter = quote! {
        paste! {
            #[no_mangle]
            #[doc = "Get `" #field_name "` for a `" #type_name"`."]
            pub unsafe extern "C" fn #getter_name(
                ptr: *const #type_name
            ) -> #ffi_type {
                let data = &*ptr;
                (&*data.#field_name).into()
            }
        }
    };
    FieldFFI {
        field: field_name.clone(),
        argument,
        assignment_expression,
        getter_name,
        consumer_type,
        getter,
    }
}

/// Generates the interface for any `Option<Vec<T>>` field. Because they're always represented by
/// some `FFIArrayT` type for FFI, which implements `From<Option<&[T]>>`, we end up using `into()`
/// regardless of what `T` is.
///
fn option_vec_field_ffi(
    type_name: &Ident,
    field_name: &Ident,
    ffi_type: &Ident,
    consumer_type: String,
) -> FieldFFI {
    let argument = quote!(#field_name: #ffi_type,);
    let assignment_expression = quote!(#field_name: (#field_name).into(),);
    let getter_name = getter_ident(type_name, field_name, true);
    let getter = quote! {
        paste! {
            #[no_mangle]
            #[doc = "Get `" #field_name "` for this `" #type_name"`."]
            pub unsafe extern "C" fn #getter_name(
                ptr: *const #type_name
            ) -> #ffi_type {
                let data = &*ptr;
                data.#field_name.as_deref().into()
            }
        }
    };
    FieldFFI {
        field: field_name.clone(),
        argument,
        assignment_expression,
        getter_name,
        consumer_type,
        getter,
    }
}

fn getter_ident(type_name: &Ident, field_name: &Ident, optional: bool) -> Ident {
    if optional {
        format_ident!(
            "get_optional_{}_{}",
            type_name.to_string().to_snake_case(),
            field_name.to_string().to_snake_case()
        )
    } else {
        format_ident!(
            "get_{}_{}",
            type_name.to_string().to_snake_case(),
            field_name.to_string().to_snake_case()
        )
    }
}
