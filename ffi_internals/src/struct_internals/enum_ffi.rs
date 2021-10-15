//!
//! Contains structures describing an enum, and implementations for building the related FFI and
//! consumer implementations.
//!

use super::field_ffi::FieldFFI;
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Path};

/// Describes a variant of an enum.
///
pub struct VariantFFI<'a> {
    /// The variant's identifier.
    ///
    pub ident: &'a Ident,

    /// The variant's fields.
    ///
    pub fields: Vec<FieldFFI<'a>>,
}

impl<'a> VariantFFI<'a> {
    /// The `Ident` for initializing an enum with this variant over the FFI.
    ///
    /// We can't use a single init function for the FFI here because we need the input values for
    /// the variant's fields to be typed. So, each variant has a dedicated and accurately typed
    /// initiailzer.
    ///
    pub(crate) fn init_fn_name(&self, type_name: &Ident) -> Ident {
        format_ident!(
            "{}_{}_rust_ffi_init",
            type_name.to_string().to_snake_case(),
            self.ident.to_string().to_snake_case()
        )
    }
}

/// Represents the components of an enum for generating an FFI.
///
/// This is intended for enums that cannot be made repr(C), which generally means enums whose
/// variants have associated values. If an enum can be made repr(C) behind a feature flag, that's
/// preferable since it's a simpler pass-by-value type.
///
pub struct EnumFFI<'a> {
    /// The identifier for the FFI module to be generated.
    ///
    pub module_name: &'a Ident,

    /// The name of the enum.
    ///
    pub type_name: &'a Ident,

    /// The enum's variants.
    ///
    pub variants: Vec<VariantFFI<'a>>,

    /// Alias modules that are referenced by the types of this enum's variants' fields.
    ///
    pub alias_modules: &'a [String],

    /// Any imports that need to be included in the generated FFI module.
    ///
    pub required_imports: &'a [Path],
}

impl<'a> EnumFFI<'a> {
    /// The name of the Rust type's free function.
    ///
    #[must_use]
    pub fn free_fn_name(&self) -> Ident {
        format_ident!(
            "rust_ffi_free_{}",
            self.type_name.to_string().to_snake_case()
        )
    }

    /// The name of the Rust type's initializer function.
    ///
    #[must_use]
    pub fn init_fn_name(&self) -> Ident {
        format_ident!(
            "{}_rust_ffi_init",
            self.type_name.to_string().to_snake_case()
        )
    }

    /// The name of the function to get the variant that a pointer to an instance of this enum
    /// represents.
    ///
    #[must_use]
    pub fn get_variant_fn_name(&self) -> Ident {
        format_ident!("get_{}_variant", self.type_name.to_string().to_snake_case())
    }

    /// The name of the repr(C) enum for this type. This is used to communicate the variants of this
    /// type across the FFI boundary.
    ///
    #[must_use]
    pub fn reprc_enum(&self) -> Ident {
        format_ident!("{}Type", self.type_name)
    }

    /// The name of the clone function for this struct.
    ///
    #[must_use]
    pub fn clone_fn_name(&self) -> Ident {
        format_ident!(
            "rust_ffi_clone_{}",
            self.type_name.to_string().to_snake_case()
        )
    }
}

impl<'a> From<EnumFFI<'_>> for TokenStream {
    fn from(enum_ffi: EnumFFI<'_>) -> Self {
        let type_name = enum_ffi.type_name;
        let module_name = enum_ffi.module_name;
        let reprc_enum = enum_ffi.reprc_enum();
        let free_fn_name = enum_ffi.free_fn_name();
        let clone_fn_name = enum_ffi.clone_fn_name();
        let get_variant_fn_name = enum_ffi.get_variant_fn_name();

        let variants = enum_ffi.variants.iter().fold(quote!(), |mut acc, variant| {
            let variant_ident = &variant.ident;
            acc.extend(quote!(#variant_ident,));
            acc
        });

        let variant_value_getters = enum_ffi.variants.iter().fold(quote!(), |mut acc, variant| {
            acc.extend(variant.fields.iter().fold(quote!(), |mut acc, field| {
                acc.extend(field.getter_fn());
                acc
            }));
            acc
        });

        let get_variant_match_body = enum_ffi.variants.iter().fold(quote!(), |mut acc, variant| {
            let variant_ident = &variant.ident;
            let variant_case = if variant.fields.is_empty() {
                quote!(#variant_ident)
            } else {
                quote!(#variant_ident(..))
            };
            acc.extend(quote! {
                #type_name::#variant_case => #reprc_enum::#variant_ident,
            });
            acc
        });

        let initializers = enum_ffi.variants.iter().fold(quote!(), |mut acc, variant| {
            let variant_ident = &variant.ident;
            let init_fn_name = variant.init_fn_name(enum_ffi.type_name);
            let args: Vec<Self> = variant
                .fields
                .iter()
                .map(FieldFFI::ffi_initializer_argument)
                .collect();
            let assignment = if variant.fields.is_empty() {
                quote!()
            } else {
                let assignments: Vec<Self> = variant
                    .fields
                    .iter()
                    .map(FieldFFI::assignment_expression)
                    .collect();
                quote!((#(#assignments),*))
            };
            let init_fn = quote! {
                /// # Safety
                /// `data` must not be a null pointer, and it must point to the appropriate type for `variant`. Otherwise, this will panic.
                ///
                #[no_mangle]
                pub unsafe extern "C" fn #init_fn_name(#(#args),*) -> *const #type_name {
                    Box::into_raw(Box::new(#type_name::#variant_ident#assignment))
                }
            };
            acc.extend(init_fn);
            acc
        });

        let required_imports: Vec<Self> = enum_ffi.required_imports
            .iter()
            .map(|import| {
                quote!(use #import;)
            })
            .collect();

        quote! {
            #[allow(missing_docs)]
            pub mod #module_name {
                use ffi_common::core::{error, paste, declare_opaque_type_ffi};
                use std::any::Any;
                #(#required_imports)*
                use super::*;

                #[derive(Debug, Clone, Copy, PartialEq, ffi_common::derive::FFI)]
                #[repr(C)]
                pub enum #reprc_enum {
                    #variants
                }

                #[no_mangle]
                pub unsafe extern "C" fn #get_variant_fn_name(data: *const #type_name) -> #reprc_enum {
                    match &*data {
                        #get_variant_match_body
                    }
                }

                #variant_value_getters

                #initializers

                #[no_mangle]
                pub unsafe extern "C" fn #clone_fn_name(ptr: *const #type_name) -> *const #type_name {
                    Box::into_raw(Box::new((&*ptr).clone()))
                }

                /// # Safety
                /// `data` must not be a null pointer.
                ///
                #[no_mangle]
                pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                    drop(Box::from_raw(data as *mut #type_name));
                }

                declare_opaque_type_ffi! { #type_name }
            }
        }
    }
}
