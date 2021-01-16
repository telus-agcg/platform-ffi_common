//!
//! Contains structures describing a struct, and implementations for building the related FFI and
//! consumer implementations.
//!

use crate::field_ffi::FieldFFI;
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::{collections::HashSet, convert::TryFrom};
use syn::{Fields, Ident};

pub struct StructFFI {
    module: Ident,
    pub name: Ident,
    pub fields: Vec<FieldFFI>,
    init_arguments: TokenStream,
    assignment_expressions: TokenStream,
    getter_fns: TokenStream,
}

impl StructFFI {
    pub fn init_fn_name(&self) -> Ident {
        format_ident!("{}_init", self.name.to_string().to_snake_case())
    }

    pub fn free_fn_name(&self) -> Ident {
        format_ident!("{}_free", self.name.to_string().to_snake_case())
    }

    pub fn clone_fn_name(&self) -> Ident {
        format_ident!("clone_{}", self.name.to_string().to_snake_case())
    }

    /// Find any extra imports from `expose_as` attributes on this struct's fields, and return them
    /// as a `TokenStream`.
    ///
    fn extra_imports(&self) -> TokenStream {
        self.fields
            .as_slice()
            .iter()
            .filter_map(|f| f.attributes.expose_as.as_ref())
            .collect::<HashSet<&syn::Path>>()
            .iter()
            .fold(quote!(), |mut acc, path| {
                acc.extend(quote!(use #path;));
                acc
            })
    }
}

pub struct StructInputs {
    pub module_name: Ident,
    pub type_name: Ident,
    pub data: syn::DataStruct,
    pub alias_modules: Vec<String>,
}

#[derive(Debug)]
pub enum DeriveError {
    UnnamedFields,
    UnitFields,
}

impl TryFrom<&StructInputs> for StructFFI {
    type Error = DeriveError;

    fn try_from(derive: &StructInputs) -> Result<Self, Self::Error> {
        // Map the fields of the struct into initializer arguments, assignment expressions, and
        // getter functions.
        let fields: Vec<FieldFFI> = match &derive.data.fields {
            Fields::Named(named) => named,
            Fields::Unnamed(_) => return Err(DeriveError::UnnamedFields),
            Fields::Unit => return Err(DeriveError::UnitFields),
        }
        .named
        .iter()
        .map(|field| FieldFFI::from((derive.type_name.clone(), field, &*derive.alias_modules)))
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

        let module = derive.module_name.clone();
        let name = derive.type_name.clone();
        Ok(Self {
            module,
            name,
            fields,
            init_arguments,
            assignment_expressions,
            getter_fns,
        })
    }
}

impl From<StructFFI> for TokenStream {
    fn from(struct_ffi: StructFFI) -> Self {
        let module_name = &struct_ffi.module;
        let type_name = &struct_ffi.name;
        let init_arguments = &struct_ffi.init_arguments;
        let assignment_expressions = &struct_ffi.assignment_expressions;
        let getter_fns = &struct_ffi.getter_fns;
        let extra_imports = struct_ffi.extra_imports();
        let free_fn_name = struct_ffi.free_fn_name();
        let init_fn_name = struct_ffi.init_fn_name();
        let clone_fn_name = struct_ffi.clone_fn_name();

        // Create a new module for the FFI for this type.
        quote!(
            #[allow(box_pointers, missing_docs)]
            pub mod #module_name {
                use ffi_common::{*, datetime::*, ffi_string, string::FFIArrayString};
                use std::os::raw::c_char;
                use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
                use paste::paste;
                #extra_imports
                use super::*;

                #[no_mangle]
                pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                    let _ = Box::from_raw(data as *mut #type_name);
                }

                declare_opaque_type_ffi! { #type_name }

                #[no_mangle]
                pub extern "C" fn #init_fn_name(
                    #init_arguments
                ) -> *const #type_name {
                    let data = #type_name {
                        #assignment_expressions
                    };
                    Box::into_raw(Box::new(data))
                }

                #[no_mangle]
                pub extern "C" fn #clone_fn_name(ptr: *const #type_name) -> *const #type_name {
                    unsafe { Box::into_raw(Box::new((&*ptr).clone())) }
                }

                #getter_fns
            }
        )
    }
}
