//!
//! Contains structures describing a repr(C) enum, and implementations for building the related FFI.
//!

use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// Represents a `repr(C)` enum. This type can be converted into a `proc_macro2::TokenStream` to
/// produce an FFI for the type it represents.
///
pub struct EnumFFI<'a> {
    module_name: &'a Ident,
    /// The name of the type that this represents.
    ///
    pub type_name: &'a Ident,
}

impl<'a> EnumFFI<'a> {
    /// Create a new `EnumFFI` from derive macro inputs.
    ///
    #[must_use]
    pub const fn new(module_name: &'a Ident, type_name: &'a Ident) -> Self {
        Self {
            module_name,
            type_name,
        }
    }

    fn free_fn_name(&self) -> Ident {
        format_ident!("free_{}", &self.type_name.to_string().to_snake_case())
    }
}

impl From<EnumFFI<'_>> for TokenStream {
    fn from(enum_ffi: EnumFFI<'_>) -> Self {
        let module_name = enum_ffi.module_name;
        let type_name = enum_ffi.type_name;
        let free_fn_name = enum_ffi.free_fn_name();
        quote! {
            #[allow(missing_docs)]
            pub mod #module_name {
                use ffi_common::core::{error, paste, declare_value_type_ffi};
                use super::*;

                #[no_mangle]
                pub unsafe extern "C" fn #free_fn_name(data: #type_name) {
                    let _ = data;
                }

                declare_value_type_ffi! { #type_name }
            }
        }
    }
}
