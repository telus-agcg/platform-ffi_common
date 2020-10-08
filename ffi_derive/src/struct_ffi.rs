//!
//! Creates an FFI module for a struct, exposing its fields as C getter functions.
//!

use crate::{field_ffi, parsing};
use ffi_common::codegen_helpers::FieldFFI;
use ffi_consumer::consumer_struct;
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Fields, Ident};

/// Builds an FFI module for the struct `type_name`.
///
pub(super) fn build(
    module_name: &Ident,
    type_name: &Ident,
    fields: &Fields,
    alias_map: &HashMap<Ident, Ident>,
) -> TokenStream {
    let (fields_ffi, init_fn_name, free_fn_name) = generate_ffi(type_name, fields, alias_map);

    let consumer = consumer_struct::generate(
        &type_name.to_string(),
        &fields_ffi,
        &init_fn_name.to_string(),
        &free_fn_name.to_string(),
    );

    write_consumer_files(type_name, consumer);

    module_token_stream(
        module_name,
        type_name,
        fields_ffi,
        &init_fn_name,
        &free_fn_name,
    )
}

pub(super) fn build_custom(
    module_name: &Ident,
    custom_module_path: &str,
    type_name: &Ident,
) -> TokenStream {
    let init_fn_name = format_ident!("{}_init", &type_name.to_string().to_snake_case());
    let free_fn_name = format_ident!("{}_free", &type_name.to_string().to_snake_case());
    let custom_ffi =
        parsing::custom_ffi_types(custom_module_path, &type_name.to_string(), &init_fn_name);

    let consumer = consumer_struct::generate_custom(
        &type_name.to_string(),
        &init_fn_name.to_string(),
        custom_ffi.0.as_ref(),
        custom_ffi.1.as_ref(),
        &free_fn_name.to_string(),
    );

    write_consumer_files(type_name, consumer);

    quote!(
        #[allow(box_pointers, missing_docs)]
        pub mod #module_name {
            use ffi_common::{*, datetime::*, ffi_string, string::FFIArrayString};
            use std::os::raw::c_char;
            use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
            use paste::paste;
            use super::*;

            #[no_mangle]
            pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                let _ = Box::from_raw(data as *mut #type_name);
            }

            declare_opaque_type_ffi! { #type_name }
        }
    )
}

fn write_consumer_files(type_name: &Ident, consumer: String) {
    let out_dir = match option_env!("FFI_CONSUMER_ROOT_DIR") {
        Some(dir) => dir,
        None => env!("OUT_DIR"),
    };
    let consumer_dir = ffi_common::codegen_helpers::create_consumer_dir(out_dir)
        .unwrap_or_else(|e| panic!("Failed to create dir at {} with error {}.", out_dir, e));
    let output_file = format!("{}/{}.swift", consumer_dir, type_name.to_string());
    std::fs::write(&output_file, consumer)
        .unwrap_or_else(|e| panic!("Failed to write {} with error {}", output_file, e));
}

/// Generate all the data for the FFI so that we can operate on it to produce both the FFI consumer
/// wrapper and the tokenized module.
///
fn generate_ffi(
    type_name: &Ident,
    fields: &Fields,
    alias_map: &HashMap<Ident, Ident>,
) -> (Vec<FieldFFI>, Ident, Ident) {
    let fields_ffi = match fields {
        Fields::Named(named) => named,
        Fields::Unnamed(_) => panic!("Unnamed fields are not supported"),
        Fields::Unit => panic!("Unit fields are not supported"),
    }
    .named
    .iter()
    .map(|field| field_ffi::generate(type_name, field, alias_map))
    .collect();
    (
        fields_ffi,
        format_ident!("{}_init", &type_name.to_string().to_snake_case()),
        format_ident!("{}_free", &type_name.to_string().to_snake_case()),
    )
}

/// Tokenize all of the generated code in an FFI module so we can return it.
///
fn module_token_stream(
    module_name: &Ident,
    type_name: &Ident,
    fields_ffi: Vec<FieldFFI>,
    init_fn_name: &Ident,
    free_fn_name: &Ident,
) -> TokenStream {
    let (init_arguments, assignment_expressions, getter_fns) =
        fields_ffi
            .into_iter()
            .fold((quote!(), quote!(), quote!()), |mut acc, field_ffi| {
                acc.0.extend(field_ffi.argument);
                acc.1.extend(field_ffi.assignment_expression);
                acc.2.extend(field_ffi.getter);
                acc
            });

    // Create a new module for the FFI for this type.
    quote!(
        #[allow(box_pointers, missing_docs)]
        pub mod #module_name {
            use ffi_common::{*, datetime::*, ffi_string, string::FFIArrayString};
            use std::os::raw::c_char;
            use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
            use paste::paste;
            use super::*;

            #[no_mangle]
            pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                let _ = Box::from_raw(data as *mut #type_name);
            }

            declare_opaque_type_ffi! { #type_name }

            #[no_mangle]
            pub extern "C" fn #init_fn_name(
                #init_arguments
            ) -> *const #type_name {
                let data = #type_name {
                    #assignment_expressions
                };
                Box::into_raw(Box::new(data))
            }

            #getter_fns
        }
    )
}
