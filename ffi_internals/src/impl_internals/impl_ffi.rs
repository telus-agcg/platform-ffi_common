//!
//! Contains data structures describing an impl, and implementations for building the related FFI
//! and consumer implementations.
//!

use super::fn_ffi::FnFFI;
use heck::{CamelCase, SnakeCase};
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

    /// Any FFI import paths specified in the attributes on the macro invocation.
    ///
    pub ffi_imports: Vec<Path>,

    /// Any consumer import paths specified in the attributes on the macro invocation.
    ///
    pub consumer_imports: Vec<Path>,

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
                    Some(FnFFI::from(method))
                } else {
                    None
                }
            })
            .collect();

        Self {
            trait_name: inputs.trait_name,
            type_name: inputs.type_name,
            fns,
            ffi_imports: inputs.ffi_imports,
            consumer_imports: inputs.consumer_imports,
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

    /// Any FFI import paths specified in the attributes on the macro invocation.
    ///
    ffi_imports: Vec<Path>,

    /// Any consumer import paths specified in the attributes on the macro invocation.
    ///
    consumer_imports: Vec<Path>,
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
        let additional_imports: Vec<String> = self
            .consumer_imports
            .iter()
            .map(|path| {
                let crate_name = path
                    .segments
                    .first()
                    .unwrap()
                    .ident
                    .to_string()
                    .to_camel_case();
                let type_name = path
                    .segments
                    .last()
                    .unwrap()
                    .ident
                    .to_string()
                    .to_camel_case();
                format!("import class {}.{}", crate_name, type_name)
            })
            .collect();
        format!(
            r#"
{common_framework}
{additional_imports}

public extension {native_type} {{
    {functions}
}}
            "#,
            common_framework = option_env!("FFI_COMMON_FRAMEWORK")
                .map(|f| format!("import {}", f))
                .unwrap_or_default(),
            additional_imports = additional_imports.join("\n"),
            native_type = self.type_name.to_string(),
            functions = self
                .fns
                .iter()
                .map(|f| f.generate_consumer(&self.module_name()))
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
        let imports = self.ffi_imports.iter().fold(quote!(), |mut stream, path| {
            stream.extend(quote!(use #path;));
            stream
        });
        let fns = self.fns.iter().fold(quote!(), |mut stream, f| {
            stream.extend(f.generate_ffi(
                &self.module_name(),
                &self.type_name,
                self.type_name_as_parameter_name(),
            ));
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
