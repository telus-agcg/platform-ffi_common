//!
//! Creates an FFI module for an (FFI-safe) `enum` data structure.
//!

use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// Builds an FFI module for the enum `type_name`.
///
pub(super) fn build(module_name: &Ident, type_name: &Ident) -> TokenStream {
    let fn_name = format_ident!("free_{}", &type_name.to_string().to_snake_case());

    quote! {
        #[allow(missing_docs)]
        pub mod #module_name {
            use super::*;
            use ffi_common::declare_value_type_array_struct;
            use paste::paste;
            use ffi_common::error;

            #[no_mangle]
            pub unsafe extern "C" fn #fn_name(data: #type_name) {
                let _ = data;
            }

            declare_value_type_array_struct! { #type_name }
        }
    }
}
