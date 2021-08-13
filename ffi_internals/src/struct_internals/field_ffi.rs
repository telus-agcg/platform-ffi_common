//!
//! Contains structures describing the fields of a struct, and implementations for building the
//! related FFI and consumer implementations.
//!

use crate::{
    alias_resolution,
    native_type_data::{Context, NativeType, NativeTypeData},
    parsing,
};
use heck::SnakeCase;
use parsing::FieldAttributes;
use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Field, Ident};

/// Represents the components of the generated FFI for a field.
#[derive(Debug)]
pub struct FieldFFI {
    /// The type to which this field belongs.
    ///
    pub type_name: Ident,

    /// The field for which this interface is being generated.
    ///
    pub field_name: Ident,

    /// The type information for generating an FFI for this field.
    ///
    pub native_type_data: NativeTypeData,

    /// The FFI helper attribute annotations on this field.
    ///
    pub attributes: FieldAttributes,
}

impl FieldFFI {
    /// The name of the generated getter function. This is used to generate the Rust getter
    /// function, and the body of the consumer's getter, which ensures that they're properly linked.
    ///
    #[must_use]
    pub fn getter_name(&self) -> Ident {
        if self.native_type_data.is_option {
            format_ident!(
                "get_optional_{}_{}",
                self.type_name.to_string().to_snake_case(),
                self.field_name.to_string().to_snake_case()
            )
        } else {
            format_ident!(
                "get_{}_{}",
                self.type_name.to_string().to_snake_case(),
                self.field_name.to_string().to_snake_case()
            )
        }
    }

    /// An extern "C" function for returning the value of this field through the FFI. This takes a
    /// pointer to the struct and returns the field's value as an FFI-safe type, as in
    /// `pub extern "C" fn get_some_type_field(ptr: *const SomeType) -> FFIType`.
    ///
    #[must_use]
    pub fn getter_fn(&self) -> TokenStream {
        let field_name = &self.field_name;
        let type_name = &self.type_name;
        let getter_name = &self.getter_name();
        let ffi_type = &self
            .native_type_data
            .ffi_type(self.attributes.expose_as_ident(), Context::Return);
        let accessor = quote!(data.#field_name);
        let conversion = self
            .native_type_data
            .rust_to_ffi_value(&accessor, &self.attributes);

        quote! {
            ffi_common::core::paste! {
                #[no_mangle]
                #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                pub unsafe extern "C" fn #getter_name(
                    ptr: *const #type_name
                ) -> #ffi_type {
                    let data = &*ptr;
                    #conversion
                }
            }
        }
    }

    /// The memberwise initializer argument for passing a value for this field in to an FFI
    /// initializer.
    ///
    #[must_use]
    pub fn ffi_initializer_argument(&self) -> TokenStream {
        let field_name = &self.field_name;
        let ffi_type = &self
            .native_type_data
            .ffi_type(self.attributes.expose_as_ident(), Context::Argument);
        quote!(#field_name: #ffi_type,)
    }

    /// Expression for assigning an argument to a field (with any required type conversion
    /// included).
    #[must_use]
    pub fn assignment_expression(&self) -> TokenStream {
        let field_name = &self.field_name;
        let conversion = self
            .native_type_data
            .argument_into_rust(&self.field_name, self.attributes.expose_as.is_some());
        quote!(#field_name: #conversion,)
    }
}

impl From<(Ident, &Field, &[String])> for FieldFFI {
    fn from(inputs: (Ident, &Field, &[String])) -> Self {
        let (type_name, field, alias_modules) = inputs;
        let field_name = field
            .ident
            .as_ref()
            .unwrap_or_else(|| abort!(field.span(), "Expected field to have an identifier."))
            .clone();
        let attributes = FieldAttributes::from(&*field.attrs);
        let (wrapping_type, unaliased_field_type) = match parsing::get_segment_for_field(&field.ty)
        {
            Some(segment) => {
                let (ident, wrapping_type) =
                    parsing::separate_wrapping_type_from_inner_type(segment);
                (
                    wrapping_type,
                    alias_resolution::resolve_type_alias(&ident, alias_modules, None)
                        .unwrap_or_else(|err| {
                            abort!(field.span(), "Alias resolution error: {}", err)
                        }),
                )
            }
            None => {
                abort!(field.ty.span(), "No path segment (field without a type?")
            }
        };

        // If this has a raw attribute, bypass the normal `NativeType` logic and use `NativeType::raw`.
        let field_type = if attributes.raw {
            NativeType::Raw(unaliased_field_type)
        } else {
            NativeType::from(unaliased_field_type)
        };

        let native_type_data = NativeTypeData::from((field_type, wrapping_type));

        Self {
            type_name,
            field_name,
            native_type_data,
            attributes,
        }
    }
}
