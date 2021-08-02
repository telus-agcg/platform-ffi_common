//!
//! Creates an FFI module for an (FFI-safe) `enum` data structure.
//!

use ffi_consumer::consumer_enum::ConsumerEnum;
use proc_macro2::TokenStream;
use ffi_internals::{
    quote::{format_ident, quote},
    syn::Ident,
    heck::SnakeCase,
};

/// Builds an FFI module for the enum `type_name`.
///
pub(super) fn build(module_name: &Ident, type_name: &Ident, out_dir: &str) -> TokenStream {
    let fn_name = format_ident!("free_{}", &type_name.to_string().to_snake_case());

    let consumer = ConsumerEnum {
        type_name: type_name.to_string(),
    };

    let file_name = format!("{}.swift", type_name.to_string());
    ffi_internals::write_consumer_file(&file_name, consumer.into(), out_dir);

    quote! {
        #[allow(missing_docs)]
        pub mod #module_name {
            use ffi_common::ffi_core::{error, paste, declare_value_type_ffi};
            use super::*;

            #[no_mangle]
            pub unsafe extern "C" fn #fn_name(data: #type_name) {
                let _ = data;
            }

            declare_value_type_ffi! { #type_name }
        }
    }
}
