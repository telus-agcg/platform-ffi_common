//!
//! Contains structures describing a fn, and implementations for building the related FFI and
//! consumer implementations.
//!

use crate::{native_type_data::{Context, NativeType, NativeTypeData, UnparsedNativeTypeData}, parsing::FieldAttributes};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ImplItemMethod, ItemFn, PatType, Type};

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
    parameters: Vec<FnParameterFFI>,

    /// The return type for this function, if any.
    pub return_type: Option<NativeTypeData>,
}

impl From<(&ImplItemMethod, Vec<Ident>, Ident)> for FnFFI {
    fn from(data: (&ImplItemMethod, Vec<Ident>, Ident)) -> Self {
        let (method, raw_types, self_type) = data;
        let fn_name = method.sig.ident.clone();
        let (arguments, has_receiver) = method.sig.inputs.iter().fold(
            (Vec::<FnParameterFFI>::new(), false),
            |mut acc, input| {
                match input {
                    syn::FnArg::Receiver(_receiver) => acc.1 = true,
                    syn::FnArg::Typed(arg) => acc.0.push(FnParameterFFI::from((arg, raw_types.clone(), Some(self_type.clone())))),
                }
                acc
            },
        );

        let return_type: Option<NativeTypeData> = match &method.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_token, ty) => Some(NativeTypeData::from(
                UnparsedNativeTypeData::initial(*ty.clone(), raw_types, Some(self_type)),
            )),
        };

        Self {
            fn_name,
            has_receiver,
            parameters: arguments,
            return_type,
        }
    }
}

impl From<(&ItemFn, Vec<Ident>)> for FnFFI {
    fn from(data: (&ItemFn, Vec<Ident>)) -> Self {
        let (method, raw_types) = data;
        let fn_name = method.sig.ident.clone();
        let (arguments, has_receiver) = method.sig.inputs.iter().fold(
            (Vec::<FnParameterFFI>::new(), false),
            |mut acc, input| {
                match input {
                    syn::FnArg::Receiver(_receiver) => acc.1 = true,
                    syn::FnArg::Typed(arg) => acc.0.push(FnParameterFFI::from((arg, raw_types.clone(), None))),
                }
                acc
            },
        );

        let return_type: Option<NativeTypeData> = match &method.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_token, ty) => Some(NativeTypeData::from(
                UnparsedNativeTypeData::initial(*ty.clone(), raw_types, None),
            )),
        };

        Self {
            fn_name,
            has_receiver,
            parameters: arguments,
            return_type,
        }
    }
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
    ///             ::ffi_core::error::set_last_err_msg(error.to_string().as_str());
    ///             <Vec<ReturnType>>::default()
    ///         }
    ///     };
    ///     (&*return_value).into()
    /// }
    /// ```
    ///
    pub fn generate_ffi(
        &self,
        module_name: &Ident,
        type_name: Option<&Ident>,
        type_as_parameter_name: Option<&Ident>,
    ) -> TokenStream {
        // If the native function takes a receiver, we'll include an parameter for a pointer to an
        // instance of this type and a line in the function body for dereferencing the pointer.
        let (receiver_arg, receiver_conversion) = if self.has_receiver {
            (
                quote!(#type_as_parameter_name: *const #type_name, ),
                quote!(let data = (*#type_as_parameter_name).clone();),
            )
        } else {
            (quote!(), quote!())
        };
        let (signature_args, calling_args, parameter_conversions) = self.parameters.iter().fold(
            (receiver_arg, quote!(), receiver_conversion),
            |mut acc, arg| {
                let name = arg.name.clone();
                let ty = arg.native_type_data.ffi_type(None, &Context::Argument);
                let signature_parameter = quote!(#name: #ty, );
                // TODO: This assumes a collection type should always be dereferenced to a slice
                // and borrowed when passed to the native function, which is not necessarily the
                // case. We should be able to figure that out from the syn collection types...we
                // just need to support them more completely instead of stripping down to "is a
                // collection".
                let symbols = if arg.native_type_data.is_vec {
                    quote!(&*)
                } else {
                    quote!()
                };
                let calling_arg = quote!(#symbols#name, );

                println!("NATIVE TYPE DATA: {:?}", arg.native_type_data);
                // For strings borrowed in the function call, we're converting them from the FFI string into `&String` (incorrectly marking the variable as borrowed). We need turn it into a real string, and then borrow (and in this case dereference) it when calling the native function. This should maybe be a whole separate function since we don't necessarily want an "owned" native type here?
                let native_type = arg.native_type_data.owned_native_type();
                let conversion = arg.native_type_data.argument_into_rust(&name, false);
                // This needs to respect whether the data is borrowed for the native call.
                // Right now wise_units gets `*Box::from_raw(lhs)`, but it just needs to borrow the
                // deref'd value, so it ought to be `&*lhs`.
                let assignment_and_conversion = quote!(let #name: #native_type = #conversion;);
                acc.0.extend(signature_parameter);
                acc.1.extend(calling_arg);
                acc.2.extend(assignment_and_conversion);
                acc
            },
        );

        let ffi_fn_name = self.ffi_fn_name(module_name);
        let native_fn_name = &self.fn_name;
        let native_call = if self.has_receiver {
            quote!(data.#native_fn_name)
        } else {
            if type_name.is_some() {
                quote!(#type_name::#native_fn_name)
            } else {
                quote!(#native_fn_name)
            }
        };
        let return_type = self
            .return_type
            .as_ref()
            .map(|r| r.ffi_type(None, &Context::Return));
        let call_and_return = if let Some(r) = &self.return_type {
            let assignment = quote!(let return_value = #native_call(#calling_args););
            let return_conversion = if r.is_result {
                match &r.native_type {
                    NativeType::Boxed(_) | 
                    NativeType::String | 
                    NativeType::DateTime 
                    if !r.is_vec => {
                        let conversion = r.rust_to_ffi_value(quote!(r), &FieldAttributes { expose_as: None, raw: false} );
                        let map = quote!(ffi_common::ffi_core::try_or_set_error!(return_value.map(|r| #conversion)));
                        map
                    },
                    _ => {
                        let native_type = r.owned_native_type();
                        let conversion = r.rust_to_ffi_value(quote!(r), &FieldAttributes { expose_as: None, raw: false} );
                        let map = quote!(ffi_common::ffi_core::try_or_set_error!(return_value.map(|r| #conversion), <#native_type>::default()));
                        if r.is_vec {
                            quote! {
                                use std::ops::Deref;
                                #map.deref().into()
                            }
                        } else {
                            map
                        }
                    },
                }
            } else {
                let accessor = quote!(return_value);
                r.rust_to_ffi_value(accessor, &FieldAttributes { expose_as: None, raw: false} )
            };
            quote! {
                #assignment
                #return_conversion
            }
        } else {
            quote!(#native_call(#calling_args);)
        };
        quote! {
            #[no_mangle]
            pub unsafe extern "C" fn #ffi_fn_name(#signature_args) -> #return_type {
                #parameter_conversions
                #call_and_return
            }
        }
    }

    fn ffi_fn_name(&self, module_name: &Ident) -> Ident {
        format_ident!("{}_{}", module_name, self.fn_name)
    }

    /// Generates a consumer function for calling the foreign function produced by
    /// `self.generate_ffi(...)`.
    ///
    pub(super) fn generate_consumer(&self, module_name: &Ident) -> String {
        // Include the keyword `static` if this function doesn't take a receiver.
        let static_keyword = if !self.has_receiver { "static" } else { "" };
        let (return_conversion, close_conversion, return_sig) =
            if let Some(return_type) = &self.return_type {
                let ty = return_type.consumer_type(None);
                (
                    if return_type.is_result {
                        "handle(result: ".to_string() 
                   } else {
                       format!("{}.fromRust(", ty) 
                   },
                   format!(")"),
                   if return_type.is_result {
                       format!("-> Result<{}, RustError>", ty)
                   } else {
                       format!("-> {}", ty)
                   },
                )
            } else {
                (String::new(), String::new(), String::new())
            };
        format!(
            r#"
    {static_keyword} func {native_fn_name}({native_parameters}) {return_sig} {{
        {return_conversion}{ffi_fn_name}({ffi_parameters}){close_conversion}
    }}
            "#,
            static_keyword = static_keyword,
            native_fn_name = self.fn_name.to_string(),
            native_parameters = self.consumer_parameters(),
            return_sig = return_sig,
            return_conversion = return_conversion,
            ffi_fn_name = self.ffi_fn_name(module_name).to_string(),
            ffi_parameters = self.ffi_calling_arguments(),
            close_conversion = close_conversion,
        )
    }

    pub fn generate_consumer_extension(&self, header: &str, consumer_type: &str, module_name: &Ident, imports: Option<&str>) -> String {
        // Include the keyword `static` if this function doesn't take a receiver.
        let static_keyword = if !self.has_receiver { "static" } else { "" };
        let (return_conversion, close_conversion, return_sig) =
            if let Some(return_type) = &self.return_type {
                let ty = return_type.consumer_type(None);
                (
                    if return_type.is_result {
                         "handle(result: ".to_string() 
                    } else {
                        format!("{}.fromRust(", ty) 
                    },
                    format!(")"),
                    if return_type.is_result {
                        format!("-> Result<{}, RustError>", ty)
                    } else {
                        format!("-> {}", ty)
                    },
                )
            } else {
                (String::new(), String::new(), String::new())
            };
        format!(
            r#"
{header}
{imports}

extension {consumer_type} {{
    {static_keyword} func {native_fn_name}({native_parameters}) {return_sig} {{
        {return_conversion}{ffi_fn_name}({ffi_parameters}){close_conversion}
    }}
}}
            "#,
            static_keyword = static_keyword,
            header = header,
            consumer_type = consumer_type,
            imports = imports.unwrap_or_default(),
            native_fn_name = self.fn_name.to_string(),
            native_parameters = self.consumer_parameters(),
            return_sig = return_sig,
            return_conversion = return_conversion,
            ffi_fn_name = self.ffi_fn_name(module_name).to_string(),
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
struct FnParameterFFI {
    /// The name of this parameter.
    ///
    name: Ident,

    /// The type information for generating an FFI for this parameter.
    ///
    native_type_data: NativeTypeData,

    /// The original type of the fn parameter.
    ///
    original_type: Type,
}

impl From<(&PatType, Vec<Ident>, Option<Ident>)> for FnParameterFFI {
    fn from(data: (&PatType, Vec<Ident>, Option<Ident>)) -> Self {
        let (arg, raw_types, self_type) = data;
        let name = if let syn::Pat::Ident(pat) = &*arg.pat {
            pat.ident.clone()
        } else {
            panic!("Anonymous parameter (not allowed in Rust 2018): {:?}", arg);
        };
        let native_type_data =
            NativeTypeData::from(UnparsedNativeTypeData::initial(*arg.ty.clone(), raw_types, self_type));
        Self {
            name,
            native_type_data,
            original_type: *arg.ty.clone(),
        }
    }
}
