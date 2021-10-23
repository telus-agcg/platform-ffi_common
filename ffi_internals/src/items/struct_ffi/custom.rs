//!
//! Contains structures describing a struct that has a custom FFI implementation, and
//! implementations for building boilerplate FFI support and consumer implementations.
//!

use crate::parsing::CustomAttributes;
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Ident, Path, Type};

/// Represents the components of a struct that has a custom FFI implementation (defined at
/// `custom_attributes.path`).
///
pub struct StructFFI<'a> {
    /// The name of the struct we're working with.
    ///
    pub type_name: &'a Ident,
    /// The identifier for the FFI module to be generated.
    ///
    pub module_name: &'a Ident,
    /// Paths that need to be imported into the consumer module.
    ///
    pub consumer_imports: &'a [Path],
    /// Paths that need to be imported into the FFI module.
    ///
    pub ffi_mod_imports: &'a [Path],
    /// Custom attributes set on the struct.
    ///
    pub custom_attributes: &'a CustomAttributes,
    /// The name of the initializer function in this struct.
    ///
    pub init_fn_name: Ident,
    /// The arguments that this struct's initializer function takes (each represented by a tuple of
    /// their identifier and type).
    ///
    pub init_args: Vec<(Ident, Type)>,
    /// The getter functions provided by this struct (each represented by a tuple of their
    /// identifier and type).
    ///
    pub getters: Vec<(Ident, Type)>,
    /// The name of the free function in this struct.
    ///
    pub free_fn_name: Ident,
    /// The name of the clone function in this struct.
    ///
    pub clone_fn_name: Ident,
    /// If true, do not generate a memberwise initializer for this type. Some types only allow
    /// construction via specific APIs that implemenat additional checks; in those cases, a
    /// generated memberwise init bypasses those restrictions.
    ///
    pub forbid_memberwise_init: bool,
    /// Documentation comments on this struct.
    ///
    pub doc_comments: &'a [Attribute],
}

impl<'a> StructFFI<'a> {
    /// Create a new `StructFFI` from derive macro inputs.
    ///
    #[must_use]
    pub fn new(
        type_name: &'a Ident,
        module_name: &'a Ident,
        crate_root: &str,
        custom_attributes: &'a CustomAttributes,
        consumer_imports: &'a [Path],
        ffi_mod_imports: &'a [Path],
        forbid_memberwise_init: bool,
        doc_comments: &'a [Attribute],
    ) -> Self {
        let init_fn_name = format_ident!("{}_init", &type_name.to_string().to_snake_case());
        let free_fn_name = format_ident!("{}_free", &type_name.to_string().to_snake_case());
        let clone_fn_name = format_ident!("clone_{}", &type_name.to_string().to_snake_case());
        let custom_path = &format!("{}/{}", crate_root, custom_attributes.path);
        let custom_ffi = crate::parsing::parse_custom_ffi_type(
            custom_path,
            &type_name.to_string(),
            &init_fn_name,
        );

        Self {
            type_name,
            module_name,
            consumer_imports,
            ffi_mod_imports,
            custom_attributes,
            init_fn_name,
            init_args: custom_ffi.0,
            getters: custom_ffi.1,
            free_fn_name,
            clone_fn_name,
            forbid_memberwise_init,
            doc_comments,
        }
    }
}

impl From<StructFFI<'_>> for TokenStream {
    fn from(ffi: StructFFI<'_>) -> Self {
        let module_name = ffi.module_name;
        let type_name = ffi.type_name;
        let free_fn_name = ffi.free_fn_name;
        let ffi_mod_imports: Vec<Self> = ffi
            .ffi_mod_imports
            .iter()
            .map(|import| quote!(use #import;))
            .collect();

        quote!(
            #[allow(box_pointers, missing_docs)]
            pub mod #module_name {
                use ffi_common::core::{ffi_string, declare_opaque_type_ffi, datetime::*, paste, string::FFIArrayString};
                use std::os::raw::c_char;
                use std::{ffi::{CStr, CString}, mem::ManuallyDrop, ptr};
                use super::*;
                #(#ffi_mod_imports)*

                #[no_mangle]
                pub unsafe extern "C" fn #free_fn_name(data: *const #type_name) {
                    drop(Box::from_raw(data as *mut #type_name));
                }

                declare_opaque_type_ffi! { #type_name }
            }
        )
    }
}
