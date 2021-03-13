//!
//! Contains structures describing a fn, and implementations for building the related FFI and
//! consumer implementations.
//!

use crate::native_type_data::{Context, NativeTypeData};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, Type};

/// A representation of a Rust fn that can be used to generate an FFI and consumer code for
/// calling that FFI.
#[derive(Debug)]
pub struct FnFFI {
    /// The name of this function.
    pub fn_name: Ident,

    /// True if this fn takes a receiver like `self`, `&self`, etc, otherwise false.
    ///
    /// This should probably use the `syn::Receiver` type to force us to cover all the possible
    /// cases, but our use cases are simple enough for now that we don't need to worry about full
    /// support.
    ///
    pub has_receiver: bool,

    /// The parameters for this function.
    pub parameters: Vec<FnParameterFFI>,

    /// The return type for this function, if any.
    pub return_type: Option<NativeTypeData>,
}

impl FnFFI {
    /// Generates a function for calling the native fn represented by this `FnFFI` from outside of
    /// Rust.
    ///
    /// For example, a function with a signature like
    /// ```ignore
    /// fn do_something(&self, another_param: &[ParamType]) -> Vec<ReturnType> { ... }
    /// ```
    /// will produce an FFI function like
    /// ```ignore
    /// pub unsafe extern "C" fn do_something(
    ///     a_receiver: *const SelfType,
    ///     another_param: FFIArrayParamType,
    /// ) -> FFIArrayReturnType {
    ///     let data = (*a_receiver).clone();
    ///     let another_param: Vec<ParamType> = another_param.into();
    ///     let return_value = data.do_something(&*another_param);
    ///     let return_value = match return_value {
    ///         Ok(val) => val,
    ///         Err(error) => {
    ///             ::ffi_common::error::set_last_err_msg(error.to_string().as_str());
    ///             <Vec<ReturnType>>::default()
    ///         }
    ///     };
    ///     (&*return_value).into()
    /// }
    /// ```
    ///
    pub fn generate_ffi(&self, type_name: &Ident, type_as_parameter_name: Ident) -> TokenStream {
        // If the native function takes a receiver, we'll include an parameter for a pointer to an
        // instance of this type and a line in the function body for dereferencing the pointer.
        let (receiver_arg, conversion) = if self.has_receiver {
            (
                quote!(#type_as_parameter_name: *const #type_name, ),
                quote!(let data = (*#type_as_parameter_name).clone();),
            )
        } else {
            (quote!(), quote!())
        };
        let (signature_args, calling_args, parameter_conversions) =
            self.parameters
                .iter()
                .fold((receiver_arg, quote!(), conversion), |mut acc, arg| {
                    let name = arg.name.clone();
                    let ty = arg.native_type_data.ffi_type(None, &Context::Argument);
                    let signature_parameter = quote!(#name: #ty, );
                    // TODO: This assumes a collection type should always be dereferenced to a slice
                    // and borrowed, which is not necessarily the case. We should be able to figure
                    // that out from the syn collection types...just need to support them more
                    // completely instead of stripping down to "is a collection".
                    let symbols = if arg.native_type_data.vec {
                        quote!(&*)
                    } else {
                        quote!()
                    };
                    let calling_arg = quote!(#symbols#name, );

                    let native_type = arg.native_type_data.owned_native_type();
                    let conversion = quote!(let #name: #native_type = #name.into(););

                    acc.0.extend(signature_parameter);
                    acc.1.extend(calling_arg);
                    acc.2.extend(conversion);
                    acc
                });

        let fn_name = self.fn_name.clone();
        let native_call = if self.has_receiver {
            quote!(data.#fn_name)
        } else {
            quote!(#type_name::#fn_name)
        };
        let return_type = self
            .return_type
            .as_ref()
            .map(|r| r.ffi_type(None, &Context::Return));
        let call_and_return = if let Some(r) = &self.return_type {
            let assignment = quote!(let return_value = #native_call(#calling_args););
            let mut return_conversion = if r.result {
                let native_type = r.owned_native_type();
                quote!(let return_value = ffi_common::try_or_set_error!(return_value, <#native_type>::default());)
            } else {
                quote!()
            };
            if r.vec {
                return_conversion.extend(quote!((&*return_value).into()));
            } else {
                return_conversion.extend(quote!(return_value.into()));
            }
            quote! {
                #assignment
                #return_conversion
            }
        } else {
            quote!(#native_call(#calling_args);)
        };
        let ffi_fn = quote! {
            pub unsafe extern "C" fn #fn_name(#signature_args) -> #return_type {
                #parameter_conversions
                #call_and_return
            }
        };
        ffi_fn
    }

    /// Generates a consumer function for calling the foreign function produced by
    /// `self.generate_ffi(...)`.
    ///
    pub fn generate_consumer(&self) -> String {
        let (return_conversion, close_conversion, return_sig) =
            if let Some(return_type) = &self.return_type {
                let ty = return_type.consumer_type(None);
                (
                    format!("{}.fromRust(", ty),
                    format!(")"),
                    format!("-> {}", ty),
                )
            } else {
                (String::new(), String::new(), String::new())
            };
        format!(
            r#"
    func {native_fn_name}({native_parameters}) {return_sig} {{
        {return_conversion}{ffi_fn_name}({ffi_parameters}){close_conversion}
    }}
            "#,
            native_fn_name = self.fn_name.to_string(),
            native_parameters = self.consumer_parameters(),
            return_sig = return_sig,
            return_conversion = return_conversion,
            ffi_fn_name = self.fn_name.to_string(),
            ffi_parameters = self.ffi_calling_arguments(),
            close_conversion = close_conversion,
        )
    }

    fn consumer_parameters(&self) -> String {
        self.parameters
            .iter()
            .map(|arg| {
                format!(
                    "{}: {}",
                    arg.name.to_string(),
                    arg.native_type_data.consumer_type(None)
                )
            })
            .collect::<Vec<String>>()
            .join(", ")
    }

    fn ffi_calling_arguments(&self) -> String {
        let mut parameters: Vec<String> = self
            .parameters
            .iter()
            .map(|arg| format!("{}.toRust()", arg.name.to_string()))
            .collect();
        if self.has_receiver {
            let receiver_arg = "pointer".to_string();
            parameters.insert(0, receiver_arg);
        }
        parameters.join(", ")
    }
}

/// Represents a parameter for to a Rust function.
#[derive(Debug)]
pub struct FnParameterFFI {
    /// The name of this parameter.
    ///
    pub name: Ident,

    /// The type information for generating an FFI for this parameter.
    ///
    pub native_type_data: NativeTypeData,

    /// The original type of the fn parameter.
    ///
    pub original_type: Type,
}