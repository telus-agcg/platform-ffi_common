//!
//! Contains structures describing a struct, and implementations for building the related FFI and
//! consumer implementations.
//!

use crate::struct_internals::field_ffi::{FieldFFI, FieldSource};
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::{spanned::Spanned, Fields, Ident, Path};

/// Represents the components of a struct for generating an FFI.
///
pub struct StructFFI<'a> {
    /// The identifier for the FFI module to be generated.
    ///
    module: &'a Ident,
    /// The name of the struct.
    ///
    pub name: &'a Ident,
    /// Any imports that need to be included in the generated FFI module.
    ///
    pub required_imports: Vec<Path>,
    /// The generated FFI for each of this struct's fields.
    ///
    pub fields: Vec<FieldFFI<'a>>,
    /// If true, do not generate a memberwise initializer for this type. Some types only allow
    /// construction via specific APIs that implemenat additional checks; in those cases, a
    /// generated memberwise init bypasses those restrictions.
    ///
    pub forbid_memberwise_init: bool,
    /// The initializer arguments, as a `TokenStream` that we can just inject into the right place
    /// in the generated module's initializer.
    ///
    init_arguments: TokenStream,
    /// The assignment expressions to convert the initializer arguments into native types, as a
    /// `TokenStream` that we can just inject into the right place in the generated module's
    /// initializer.
    ///
    assignment_expressions: TokenStream,
    /// The getter functions to include in this module, pre-generated and provided here as a
    /// `TokenStream`.
    ///
    getter_fns: TokenStream,
}

impl StructFFI<'_> {
    /// The name of the initializer function for this struct.
    ///
    #[must_use]
    pub fn init_fn_name(&self) -> Ident {
        format_ident!("{}_rust_ffi_init", self.name.to_string().to_snake_case())
    }

    /// The name of the free function for this struct.
    ///
    #[must_use]
    pub fn free_fn_name(&self) -> Ident {
        format_ident!("{}_rust_ffi_free", self.name.to_string().to_snake_case())
    }

    /// The name of the clone function for this struct.
    ///
    #[must_use]
    pub fn clone_fn_name(&self) -> Ident {
        format_ident!("rust_ffi_clone_{}", self.name.to_string().to_snake_case())
    }

    /// Find any extra imports from `expose_as` attributes on this struct's fields, and return them
    /// as a `TokenStream`.
    ///
    fn extra_imports(&self) -> TokenStream {
        self.fields
            .as_slice()
            .iter()
            .filter_map(|f| f.attributes.expose_as.as_ref())
            .collect::<HashSet<&Path>>()
            .iter()
            .fold(quote!(), |mut acc, path| {
                acc.extend(quote!(use #path;));
                acc
            })
    }
}

/// Representes the inputs for building a `StructFFI`.
///
pub struct StructInputs<'a> {
    /// The identifier for the FFI module to be generated.
    ///
    pub module_name: &'a Ident,
    /// The name of the struct.
    ///
    pub type_name: &'a Ident,
    /// The struct's parsed data structure.
    ///
    pub data: &'a syn::DataStruct,
    /// Alias modules that are referenced by the types of this struct's fields.
    ///
    pub alias_modules: &'a [String],
    /// Any imports that need to be included in the generated FFI module.
    ///
    pub required_imports: &'a [Path],
    /// If true, do not generate a memberwise initializer for this type. Some types only allow
    /// construction via specific APIs that implemenat additional checks; in those cases, a
    /// generated memberwise init bypasses those restrictions.
    ///
    pub forbid_memberwise_init: bool,
}

impl<'a> From<&StructInputs<'a>> for StructFFI<'a> {
    fn from(derive: &StructInputs<'a>) -> Self {
        let fields: Vec<FieldFFI<'_>> = match &derive.data.fields {
            Fields::Named(fields) => super::field_ffi::field_inputs_from_named_fields(
                fields,
                &FieldSource::Struct,
                derive.type_name,
                derive.alias_modules,
            ),
            Fields::Unnamed(fields) => super::field_ffi::field_inputs_from_unnamed_fields(
                fields,
                &FieldSource::Struct,
                derive.type_name,
                derive.alias_modules,
            ),
            Fields::Unit => proc_macro_error::abort!(
                derive.data.fields.span(),
                "Unit fields are not supported."
            ),
        }
        .into_iter()
        .map(FieldFFI::from)
        .collect();

        let (init_arguments, assignment_expressions, getter_fns) =
            fields
                .iter()
                .fold((quote!(), quote!(), quote!()), |mut acc, field_ffi| {
                    acc.0.extend(field_ffi.ffi_initializer_argument());
                    acc.1.extend(field_ffi.assignment_expression());
                    acc.2.extend(field_ffi.getter_fn());
                    acc
                });

        Self {
            module: derive.module_name,
            name: derive.type_name,
            required_imports: derive.required_imports.to_owned(),
            fields,
            init_arguments,
            assignment_expressions,
            getter_fns,
            forbid_memberwise_init: derive.forbid_memberwise_init,
        }
    }
}

impl<'a> From<StructFFI<'_>> for TokenStream {
    fn from(struct_ffi: StructFFI<'_>) -> Self {
        let module_name = &struct_ffi.module;
        let type_name = &struct_ffi.name;
        let init_arguments = &struct_ffi.init_arguments;
        let assignment_expressions = &struct_ffi.assignment_expressions;
        let getter_fns = &struct_ffi.getter_fns;
        let extra_imports = struct_ffi.extra_imports();
        let free_fn_name = struct_ffi.free_fn_name();
        let init_fn_name = struct_ffi.init_fn_name();
        let clone_fn_name = struct_ffi.clone_fn_name();
        let required_imports: Vec<Self> = struct_ffi.required_imports
            .iter()
            .map(|import| {
                quote!(use #import;)
            })
            .collect();

        let initializer = if struct_ffi.forbid_memberwise_init {
            quote!()
        } else {
            quote! {
                #[no_mangle]
                pub unsafe extern "C" fn #init_fn_name(
                    #init_arguments
                ) -> *const #type_name {
                    let data = #type_name {
                        #assignment_expressions
                    };
                    Box::into_raw(Box::new(data))
                }
            }
        };

        // Create a new module for the FFI for this type.
        quote!(
            #[allow(box_pointers, missing_docs)]
            pub mod #module_name {
                use ffi_common::core::{*, paste, datetime::*, string::FFIArrayString};
                use std::os::raw::c_char;
                use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
                #extra_imports
                #(#required_imports)*
                use super::*;

                #[no_mangle]
                pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                    drop(Box::from_raw(data as *mut #type_name));
                }

                declare_opaque_type_ffi! { #type_name }

                #initializer

                #[no_mangle]
                pub unsafe extern "C" fn #clone_fn_name(ptr: *const #type_name) -> *const #type_name {
                    Box::into_raw(Box::new((&*ptr).clone()))
                }

                #getter_fns
            }
        )
    }
}
