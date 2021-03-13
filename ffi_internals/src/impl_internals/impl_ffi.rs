//!
//! Contains data structures describing an impl, and implementations for building the related FFI
//! and consumer implementations.
//!

use super::fn_ffi::{FnFFI, FnParameterFFI};
use crate::native_type_data::{NativeTypeData, UnparsedNativeTypeData};
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ImplItem, Path};

/// Describes the data required to create an `ImplFFI`.
///
/// This is an intermediate object for taking parts of the data from a `syn::ItemImpl` and
/// processing it into the data we need for generating an FFI.
///
pub struct ImplInputs {
    /// The `ImplItem`s found in the `syn::ItemImpl`
    ///
    pub items: Vec<ImplItem>,

    /// Any import paths specified in the attributes on the macro invocation.
    ///
    pub import_paths: Vec<Path>,

    /// The name of the trait that's implemented.
    ///
    /// Note that this is currently required; we don't support standalone impls right now because
    /// we're relying on the trait name + type name pair to guarantee uniqueness for the generated
    /// FFI module and consumer file. If we have a use case for exposing standalone impls, we'll
    /// have to come up with another way to ensure that uniqueness.
    ///
    pub trait_name: Ident,

    /// The name of the type that this implementation applies to.
    ///
    pub type_name: Ident,
}

impl From<ImplInputs> for ImplFFI {
    fn from(inputs: ImplInputs) -> Self {
        let fns = inputs
            .items
            .iter()
            .filter_map(|item| {
                if let syn::ImplItem::Method(method) = item {
                    let fn_name = method.sig.ident.clone();
                    let (arguments, has_receiver) = method.sig.inputs.iter().fold(
                        (Vec::<FnParameterFFI>::new(), false),
                        |mut acc, input| {
                            match input {
                                syn::FnArg::Receiver(_receiver) => acc.1 = true,
                                syn::FnArg::Typed(arg) => {
                                    let argument_name = if let syn::Pat::Ident(pat) = &*arg.pat {
                                        pat.ident.clone()
                                    } else {
                                        panic!(
                                            "Anonymous parameter (not allowed in Rust 2018): {:?}",
                                            input
                                        );
                                    };
                                    let native_type_data = NativeTypeData::from(
                                        UnparsedNativeTypeData::initial(*arg.ty.clone()),
                                    );
                                    acc.0.push(FnParameterFFI {
                                        name: argument_name,
                                        native_type_data,
                                        original_type: *arg.ty.clone(),
                                    })
                                }
                            }
                            acc
                        },
                    );

                    let return_type: Option<NativeTypeData> = match &method.sig.output {
                        syn::ReturnType::Default => None,
                        syn::ReturnType::Type(_token, ty) => Some(NativeTypeData::from(
                            UnparsedNativeTypeData::initial(*ty.clone()),
                        )),
                    };

                    Some(FnFFI {
                        fn_name,
                        has_receiver,
                        parameters: arguments,
                        return_type,
                    })
                } else {
                    None
                }
            })
            .collect();

        Self {
            trait_name: inputs.trait_name,
            type_name: inputs.type_name,
            fns,
            import_paths: inputs.import_paths,
        }
    }
}

/// A representation of a Rust impl that can be used to generate an FFI and consumer code for
/// calling that FFI.
///
#[derive(Debug)]
pub struct ImplFFI {
    /// The name of the trait that's implemented.
    ///
    /// Note that this is currently required; we don't support standalone impls right now because
    /// we're relying on the trait name + type name pair to guarantee uniqueness for the generated
    /// FFI module and consumer file. If we have a use case for exposing standalone impls, we'll
    /// have to come up with another way to ensure that uniqueness.
    ///
    trait_name: Ident,

    /// The name of the type that this implementation applies to.
    ///
    type_name: Ident,

    /// A collection of representations of the functions declared in this impl that can be used to
    /// generate an FFI and consumer code for each function.
    ///
    fns: Vec<FnFFI>,

    /// Any import paths specified in the attributes on the macro invocation.
    ///
    import_paths: Vec<Path>,
}

impl ImplFFI {
    pub fn consumer_file_name(&self) -> String {
        format!("{}_{}.swift", self.trait_name, self.type_name)
    }

    /// Generates an implementation for the consumer's type so that they'll be able to call it like
    /// `nativeTypeInstance.someMethod(with: params)`. Hardcoded to Swift for now like all the other
    /// consumer output, until we bother templating for other languages.
    ///
    /// Example output:
    /// ```ignore
    /// extension SelectedField {
    ///     func build_commodity_locations(plantings: [CLPlanting]) -> [CommodityLocation] {
    ///         [CommodityLocation].fromRust(build_commodity_locations(pointer, plantings.toRust()))
    ///     }
    /// }
    /// ```
    ///
    pub fn generate_consumer(&self) -> String {
        format!(
            r#"
extension {native_type} {{
    {functions}
}}
            "#,
            native_type = self.type_name.to_string(),
            functions = self
                .fns
                .iter()
                .map(|f| f.generate_consumer())
                .collect::<Vec<String>>()
                .join("\n"),
        )
    }
}

impl ImplFFI {
    /// Returns the name of the type the impl is for as a snake-cased string, to be used as the
    /// first parameter name in the signature of an FFI function if the native function expects a
    /// receiver (`self`, `&self`, etc.).
    ///
    fn type_name_as_parameter_name(&self) -> Ident {
        format_ident!("{}", self.type_name.to_string().to_snake_case())
    }

    /// The name for the generated module, in the pattern `trait_name_type_name_ffi`.
    ///
    fn module_name(&self) -> Ident {
        format_ident!(
            "{}_{}_ffi",
            self.trait_name.to_string().to_snake_case(),
            self.type_name.to_string().to_snake_case()
        )
    }

    pub fn generate_ffi(&self) -> TokenStream {
        let mod_name = self.module_name();
        let imports = self.import_paths.iter().fold(quote!(), |mut stream, path| {
            stream.extend(quote!(use #path::*;));
            stream
        });
        let fns = self.fns.iter().fold(quote!(), |mut stream, f| {
            stream.extend(f.generate_ffi(&self.type_name, self.type_name_as_parameter_name()));
            stream
        });
        quote! {
            pub mod #mod_name {
                use super::*;
                #imports
                #fns
            }
        }
    }
}
