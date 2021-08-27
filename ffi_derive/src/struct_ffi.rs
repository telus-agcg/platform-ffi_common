//!
//! Creates an FFI module for a struct, exposing its fields as C getter functions.
//!

use ffi_internals::{
    consumer::consumer_struct::{ConsumerStruct, CustomConsumerStructInputs},
    heck::SnakeCase,
    parsing,
    parsing::CustomAttributes,
    quote::{format_ident, quote},
    struct_internals::struct_ffi::{StructFFI, StructInputs},
    syn::{DataStruct, Ident, Path},
};
use proc_macro2::TokenStream;

pub(super) fn custom(
    type_name: &Ident,
    module_name: &Ident,
    crate_root: &str,
    custom_attributes: &CustomAttributes,
    required_imports: &[Path],
    out_dir: &str,
) -> TokenStream {
    let init_fn_name = format_ident!("{}_init", &type_name.to_string().to_snake_case());
    let free_fn_name = format_ident!("{}_free", &type_name.to_string().to_snake_case());
    let clone_fn_name = format_ident!("clone_{}", &type_name.to_string().to_snake_case());
    let custom_path = &format!("{}/{}", crate_root, custom_attributes.path);
    let custom_ffi =
        parsing::parse_custom_ffi_type(custom_path, &type_name.to_string(), &init_fn_name);

    let inputs = CustomConsumerStructInputs {
        type_name: type_name.to_string(),
        required_imports,
        custom_attributes,
        init_fn_name: init_fn_name.to_string(),
        init_args: custom_ffi.0.as_ref(),
        getters: custom_ffi.1.as_ref(),
        free_fn_name: free_fn_name.to_string(),
        clone_fn_name: clone_fn_name.to_string(),
    };
    let consumer = ConsumerStruct::from(inputs);

    let file_name = format!("{}.swift", type_name.to_string());
    ffi_internals::write_consumer_file(&file_name, String::from(&consumer), out_dir)
        .unwrap_or_else(|err| {
            proc_macro_error::abort!(type_name.span(), "Error writing consumer file: {}", err)
        });

    quote!(
        #[allow(box_pointers, missing_docs)]
        pub mod #module_name {
            use ffi_common::core::{ffi_string, declare_opaque_type_ffi, datetime::*, paste, string::FFIArrayString};
            use std::os::raw::c_char;
            use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
            use super::*;

            #[no_mangle]
            pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                let _ = Box::from_raw(data as *mut #type_name);
            }

            declare_opaque_type_ffi! { #type_name }
        }
    )
}

pub(super) fn standard(
    module_name: &Ident,
    type_name: &Ident,
    data: &DataStruct,
    alias_modules: &[String],
    required_imports: &[Path],
    out_dir: &str,
) -> TokenStream {
    let struct_ffi = StructFFI::from(&StructInputs {
        module_name,
        type_name: type_name.clone(),
        data,
        alias_modules,
        required_imports,
    });
    let file_name = format!("{}.swift", type_name.to_string());
    ffi_internals::write_consumer_file(
        &file_name,
        String::from(&ConsumerStruct::from(&struct_ffi)),
        out_dir,
    )
    .unwrap_or_else(|err| {
        proc_macro_error::abort!(type_name.span(), "Error writing consumer file: {}", err)
    });

    struct_ffi.into()
}
