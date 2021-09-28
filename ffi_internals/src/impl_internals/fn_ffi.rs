//!
//! Contains structures describing a fn, and implementations for building the related FFI and
//! consumer implementations.
//!

use crate::{
    parsing::{FieldAttributes, TypeAttributes},
    type_ffi::{Context, TypeFFI, TypeIdentifier},
};
use lazy_static::__Deref;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{spanned::Spanned, Ident, ImplItemMethod, ItemFn, PatType, Type};

/// Describes the various kinds of receivers we may encounter when parsing a function.
///
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FnReceiver {
    /// No receiver (i.e. the function does not take any kind of `self` argument).
    ///
    None,
    /// The function takes an owned receiver (i.e. `self`).
    ///
    Owned,
    /// The function takes a borrowed receiver (i.e. `&self`).
    ///
    Borrowed,
}

/// A representation of a Rust fn that can be used to generate an FFI and consumer code for
/// calling that FFI.
#[derive(Debug)]
pub struct FnFFI {
    /// The name of this function.
    ///
    pub fn_name: Ident,

    /// True if this fn takes a receiver like `self`, `&self`, etc, otherwise false.
    ///
    /// This should probably use the `syn::Receiver` type to force us to cover all the possible
    /// cases, but our use cases are simple enough for now that we don't need to worry about full
    /// support.
    ///
    pub receiver: FnReceiver,

    /// The parameters for this function.
    pub(crate) parameters: Vec<FnParameterFFI>,

    /// The return type for this function, if any.
    pub return_type: Option<TypeFFI>,
}

/// Representes the inputs for building a `FnFFI`.
///
pub struct FnFFIInputs<'a> {
    /// The impl's parsed data structure.
    ///
    pub method: &'a ImplItemMethod,
    /// Any types referenced in the impl that should be passed through the FFI without wrapping,
    /// such as numerics or `repr(C)` enums/structs.
    ///
    pub raw_types: Vec<Ident>,
    /// The type of `Self` in this impl.
    ///
    pub self_type: Ident,
    /// A map of local aliases, where the key is the newtype's identifier and the value is the
    /// underlying type.
    ///
    pub local_aliases: HashMap<Ident, Type>,
}

impl<'a> FnFFIInputs<'a> {
    fn strip_local_alias(&self, ty: &Type) -> Type {
        if let Type::Path(type_path) = ty {
            self.local_aliases
                .get(&type_path.path.segments.last().unwrap().ident)
                .map_or_else(|| ty.deref().clone(), std::clone::Clone::clone)
        } else {
            ty.deref().clone()
        }
    }
}

impl<'a> From<FnFFIInputs<'a>> for FnFFI {
    fn from(inputs: FnFFIInputs<'_>) -> Self {
        let fn_name = inputs.method.sig.ident.clone();
        let (arguments, receiver) = inputs.method.sig.inputs.iter().fold(
            (Vec::<FnParameterFFI>::new(), FnReceiver::None),
            |mut acc, input| {
                match input {
                    syn::FnArg::Receiver(receiver) => {
                        acc.1 = if receiver.reference.is_some() {
                            FnReceiver::Borrowed
                        } else {
                            FnReceiver::Owned
                        }
                    }
                    syn::FnArg::Typed(arg) => {
                        acc.0.push(FnParameterFFI::from(FnParameterFFIInputs {
                            arg,
                            raw_types: inputs.raw_types.clone(),
                            self_type: Some(inputs.self_type.clone()),
                        }));
                    }
                }
                acc
            },
        );

        let return_type: Option<TypeFFI> = match &inputs.method.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_token, ty) => {
                let dealiased = inputs.strip_local_alias(&*ty);
                Some(TypeFFI::from(TypeAttributes::initial(
                    dealiased,
                    inputs.raw_types,
                    Some(inputs.self_type),
                )))
            }
        };

        Self {
            fn_name,
            receiver,
            parameters: arguments,
            return_type,
        }
    }
}

impl From<(&ItemFn, Vec<Ident>)> for FnFFI {
    /// Converts a tuple of `syn::ItemFn` (the function to build an FFI for) and `Vec<Ident>` (a
    /// collection of raw types that can be exposed directly) to a `FnFFI`.
    ///
    /// When building an `FnFFI` for a function inside of an impl, as with
    /// `ffi_derive::expose_impl`, use `FnFFI::from(&FnFFIInputs)` instead, since `FnFFIInputs`
    /// captures additional information available in the impl that may be necessary to build the
    /// FFI function.
    ///
    fn from(data: (&ItemFn, Vec<Ident>)) -> Self {
        let (method, raw_types) = data;
        let fn_name = method.sig.ident.clone();
        let (arguments, receiver) = method.sig.inputs.iter().fold(
            (Vec::<FnParameterFFI>::new(), FnReceiver::None),
            |mut acc, input| {
                match input {
                    syn::FnArg::Receiver(receiver) => {
                        acc.1 = if receiver.reference.is_some() {
                            FnReceiver::Borrowed
                        } else {
                            FnReceiver::Owned
                        }
                    }
                    syn::FnArg::Typed(arg) => {
                        acc.0.push(FnParameterFFI::from(FnParameterFFIInputs {
                            arg,
                            raw_types: raw_types.clone(),
                            self_type: None,
                        }));
                    }
                }
                acc
            },
        );

        let return_type: Option<TypeFFI> = match &method.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_token, ty) => Some(TypeFFI::from(TypeAttributes::initial(
                *ty.clone(),
                raw_types,
                None,
            ))),
        };

        Self {
            fn_name,
            receiver,
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
    ///             ::core::error::set_last_err_msg(error.to_string().as_str());
    ///             <Vec<ReturnType>>::default()
    ///         }
    ///     };
    ///     (&*return_value).into()
    /// }
    /// ```
    ///
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn generate_ffi(
        &self,
        module_name: &Ident,
        type_name: Option<&Ident>,
        type_as_parameter_name: Option<&Ident>,
    ) -> TokenStream {
        // If the native function takes a receiver, we'll include an parameter for a pointer to an
        // instance of this type and a line in the function body for dereferencing the pointer.
        let (receiver_arg, receiver_conversion) = match self.receiver {
            FnReceiver::None => (quote!(), quote!()),
            FnReceiver::Owned => (
                quote!(#type_as_parameter_name: *const #type_name, ),
                quote!(let data = (*#type_as_parameter_name).clone();),
            ),
            FnReceiver::Borrowed => (
                quote!(#type_as_parameter_name: *const #type_name, ),
                quote!(let data = (&*#type_as_parameter_name);),
            ),
        };
        let (signature_args, calling_args, parameter_conversions) = self.parameters.iter().fold(
            (receiver_arg, quote!(), receiver_conversion),
            |mut acc, arg| {
                let name = arg.name.clone();
                let ty = arg.native_type_data.ffi_type(None, Context::Argument);
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

                let native_type = arg.native_type_data.native_type();
                let conversion = arg
                    .native_type_data
                    .argument_into_rust(&quote!(#name), false);
                let conversion = if arg.native_type_data.is_borrow
                    && arg.native_type_data.native_type == TypeIdentifier::String
                {
                    quote!(&*#conversion)
                } else {
                    conversion
                };
                let assignment_and_conversion = quote!(let #name: #native_type = #conversion;);
                acc.0.extend(signature_parameter);
                acc.1.extend(calling_arg);
                acc.2.extend(assignment_and_conversion);
                acc
            },
        );

        let ffi_fn_name = self.ffi_fn_name(module_name);
        let native_fn_name = &self.fn_name;
        let native_call = if self.receiver == FnReceiver::None {
            if type_name.is_some() {
                quote!(#type_name::#native_fn_name)
            } else {
                quote!(#native_fn_name)
            }
        } else {
            quote!(data.#native_fn_name)
        };
        let return_type = self
            .return_type
            .as_ref()
            .map(|r| r.ffi_type(None, Context::Return));
        let call_and_return = if let Some(r) = &self.return_type {
            let assignment = quote!(let return_value = #native_call(#calling_args););
            let return_conversion = if r.is_result {
                match &r.native_type {
                    TypeIdentifier::Boxed(_)
                    | TypeIdentifier::String
                    | TypeIdentifier::DateTime
                        if !r.is_vec =>
                    {
                        let conversion = r.rust_to_ffi_value(
                            &quote!(r),
                            &FieldAttributes {
                                expose_as: None,
                                raw: false,
                            },
                        );
                        quote!(
                            ffi_common::core::try_or_set_error!(return_value.map(|r| #conversion))
                        )
                    }
                    _ => {
                        let native_type = r.native_type();
                        let conversion = r.rust_to_ffi_value(
                            &quote!(r),
                            &FieldAttributes {
                                expose_as: None,
                                raw: false,
                            },
                        );
                        let map = quote!(
                            ffi_common::core::try_or_set_error!(return_value.map(|r| #conversion), <#native_type>::default())
                        );
                        if r.is_vec {
                            quote! {
                                use std::ops::Deref;
                                #map.deref().into()
                            }
                        } else {
                            map
                        }
                    }
                }
            } else {
                let accessor = quote!(return_value);
                r.rust_to_ffi_value(
                    &accessor,
                    &FieldAttributes {
                        expose_as: None,
                        raw: false,
                    },
                )
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

    pub(crate) fn ffi_fn_name(&self, module_name: &Ident) -> Ident {
        format_ident!("{}_{}", module_name, self.fn_name)
    }
}

/// Represents a parameter for to a Rust function.
#[derive(Debug)]
pub(crate) struct FnParameterFFI {
    /// The name of this parameter.
    ///
    pub(crate) name: Ident,

    /// The type information for generating an FFI for this parameter.
    ///
    pub(crate) native_type_data: TypeFFI,

    /// The original type of the fn parameter.
    ///
    pub(crate) original_type: Type,
}

/// Representes the inputs for building a `FnParameterFFI`.
///
pub struct FnParameterFFIInputs<'a> {
    /// The parameter argument, as in `foo: i32`.
    ///
    arg: &'a PatType,
    /// Any types that we should expose directly through the FFI.
    ///
    raw_types: Vec<Ident>,
    /// The type of `self` in this context, if available.
    ///
    self_type: Option<Ident>,
}

impl<'a> From<FnParameterFFIInputs<'a>> for FnParameterFFI {
    fn from(inputs: FnParameterFFIInputs<'_>) -> Self {
        let name = if let syn::Pat::Ident(pat) = &*inputs.arg.pat {
            pat.ident.clone()
        } else {
            proc_macro_error::abort!(
                inputs.arg.span(),
                "Anonymous parameter (not allowed in Rust 2018): {:?}"
            );
        };
        let native_type_data = TypeFFI::from(TypeAttributes::initial(
            *inputs.arg.ty.clone(),
            inputs.raw_types,
            inputs.self_type,
        ));
        Self {
            name,
            native_type_data,
            original_type: *inputs.arg.ty.clone(),
        }
    }
}
