//!
//! Creates an FFI module for an (FFI-safe) `enum` data structure.
//!

use ffi_internals::{
    consumer::{
        consumer_enum::{ComplexConsumerEnum, ReprCConsumerEnum},
        ConsumerOutput
    },
    heck::SnakeCase,
    quote::{format_ident, quote},
    struct_internals::enum_ffi::{EnumFFI, VariantFFI},
    syn::{DataEnum, Ident, Path},
};
use proc_macro2::TokenStream;

/// Builds an FFI module for the enum `type_name`.
///
pub(super) fn reprc(module_name: &Ident, type_name: &Ident, out_dir: &str) -> TokenStream {
    let file_name = format!("{}.swift", type_name.to_string());
    ffi_internals::write_consumer_file(
        &file_name,
        (&ReprCConsumerEnum::new(type_name)).output(),
        out_dir,
    )
    .unwrap_or_else(|err| proc_macro_error::abort!("Error writing consumer file: {}", err));

    let free_fn_name = format_ident!("free_{}", &type_name.to_string().to_snake_case());

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

pub(super) fn complex(
    module_name: &Ident,
    type_name: &Ident,
    derive: &DataEnum,
    alias_modules: &[String],
    required_imports: &[Path],
    out_dir: &str,
) -> TokenStream {
    let variants = derive
        .variants
        .iter()
        .map(|variant| {
            let other_variants = derive
                .variants
                .iter()
                .cloned()
                .filter_map(|other_variant| {
                    if &other_variant == variant {
                        None
                    } else {
                        Some((other_variant.ident, other_variant.fields.len()))
                    }
                })
                .collect();
            let fields = ffi_internals::struct_internals::field_ffi::fields_for_variant(
                type_name,
                alias_modules,
                &variant.ident,
                &variant.fields,
                other_variants,
            );
            VariantFFI {
                ident: &variant.ident,
                fields,
            }
        })
        .collect();

    let enum_ffi = EnumFFI {
        module_name,
        type_name,
        variants,
        alias_modules,
        required_imports,
    };
    let file_name = format!("{}.swift", type_name.to_string());
    ffi_internals::write_consumer_file(
        &file_name,
        (&ComplexConsumerEnum::new(&enum_ffi)).output(),
        out_dir,
    )
    .unwrap_or_else(|err| {
        proc_macro_error::abort!(type_name.span(), "Error writing consumer file: {}", err)
    });
    enum_ffi.into()
}
