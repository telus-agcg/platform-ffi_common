//!
//! Creates an FFI module for a struct, exposing its fields as C getter functions.
//!

use ffi_consumer::consumer_struct::ConsumerStruct;
use ffi_internals::{
    parsing,
    struct_ffi::{StructFFI, StructInputs},
};
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::format_ident;
use std::convert::TryFrom;
use syn::Ident;

pub(super) fn custom(
    type_name: &Ident,
    module_name: &Ident,
    custom_path: &str,
    out_dir: &str,
) -> TokenStream {
    let init_fn_name = format_ident!("{}_init", &type_name.to_string().to_snake_case());
    let free_fn_name = format_ident!("{}_free", &type_name.to_string().to_snake_case());
    let clone_fn_name = format_ident!("clone_{}", &type_name.to_string().to_snake_case());
    let custom_ffi = parsing::custom_ffi_types(custom_path, &type_name.to_string(), &init_fn_name);

    let consumer = ConsumerStruct::custom(
        type_name.to_string(),
        init_fn_name.to_string(),
        custom_ffi.0.as_ref(),
        custom_ffi.1.as_ref(),
        free_fn_name.to_string(),
        clone_fn_name.to_string(),
    );

    ffi_internals::write_consumer_files(type_name, consumer.into(), out_dir);

    quote::quote!(
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

pub(super) fn standard(
    module_name: Ident,
    type_name: &Ident,
    data: syn::DataStruct,
    alias_modules: Vec<String>,
    out_dir: &str,
) -> TokenStream {
    let struct_ffi = StructFFI::try_from(&StructInputs {
        module_name,
        type_name: type_name.clone(),
        data,
        alias_modules,
    })
    .expect("Unsupported struct data");

    ffi_internals::write_consumer_files(
        type_name,
        ConsumerStruct::from(&struct_ffi).into(),
        out_dir,
    );

    struct_ffi.into()
}
