//!
//! Contains data structures describing native type data, with implementations for calculating the
//! related FFI and consumer types.
//!

use crate::{parsing, parsing::WrappingType};
use heck::CamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Type};

static STRING: &str = "String";
static DATETIME: &str = "NaiveDateTime";
static UUID: &str = "Uuid";
static BOOL: &str = "bool";
static U8: &str = "u8";
static U16: &str = "u16";
static U32: &str = "u32";
static U64: &str = "u64";
static I8: &str = "i8";
static I16: &str = "i16";
static I32: &str = "i32";
static I64: &str = "i64";
static F32: &str = "f32";
static F64: &str = "f64";

/// The type of a field on a struct (from the perspective of generating an FFI).
///
#[derive(Debug, Clone)]
pub enum NativeType {
    /// A type that should be exposed behind on opaque pointer; we'll make this available as a
    /// `*const T`, and consumers of that interface will be able to initialize, free, and access
    /// properties on the type from getter functions.
    ///
    Boxed(Ident),
    /// A timestamp that's safe to expose across the FFI (see `ffi_common::datetime`).
    ///
    DateTime,
    /// A type that should be exposed as a raw value (like an i32, or a repr(C) enum).
    ///
    Raw(Ident),
    /// A String.
    ///
    String,
    /// A Uuid.
    ///
    Uuid,
}

impl From<Ident> for NativeType {
    fn from(type_path: Ident) -> Self {
        match type_path {
            t if t == DATETIME => Self::DateTime,
            t if t == STRING => Self::String,
            t if t == UUID => Self::Uuid,
            t if t == BOOL
                || t == U8
                || t == U16
                || t == U32
                || t == U64
                || t == I8
                || t == I16
                || t == I32
                || t == I64
                || t == F32
                || t == F64 =>
            {
                Self::Raw(t)
            }
            t => Self::Boxed(t),
        }
    }
}

/// The context in which a type is being referenced. Sometimes the mutability of a reference, or
/// even the type it's exposed as is different depending on whether it's being returned to the
/// consumer or passed in as an argument to a Rust function.
///
#[derive(Debug)]
pub enum Context {
    /// Type is being used as an argument to a Rust function.
    Argument,
    /// Type is being returned to the consumer.
    Return,
}

#[derive(Debug)]
pub struct NativeTypeData {
    /// The native Rust type of the field.
    ///
    pub native_type: NativeType,
    /// True if this field is an `Option`, otherwise false.
    ///
    pub option: bool,
    /// True if this field is a `Vec`, otherwise false.
    ///
    pub vec: bool,
}

impl From<(NativeType, WrappingType)> for NativeTypeData {
    fn from(data: (NativeType, WrappingType)) -> Self {
        let (native_type, wrapping_type) = data;
        NativeTypeData {
            native_type,
            option: wrapping_type == WrappingType::Option
                || wrapping_type == WrappingType::OptionVec,
            vec: wrapping_type == WrappingType::Vec || wrapping_type == WrappingType::OptionVec,
        }
    }
}

impl NativeTypeData {
    /// Returns the name of the type used for communicating this field's data across the FFI
    /// boundary.
    ///
    /// When `mutable` is `true`, if `self` is exposed as a non-string reference type (such as a
    /// `Box`ed struct or `Box`ed optional primitive), this will produce a token stream like
    /// `*mut T`. This is mostly for the sake of the generated initializer, which takes mutable
    /// pointers to indicate that the pointer will be consumed.
    ///
    /// When `mutable` is `false`, if `self is exposed as a reference type, this will produce a
    /// token stream like `*const T`.
    ///
    #[must_use]
    pub fn ffi_type(&self, expose_as: Option<&Ident>, context: &Context) -> TokenStream {
        let ptr_type = match context {
            Context::Argument => quote!(*mut),
            Context::Return => quote!(*const),
        };
        match &self.native_type {
            NativeType::Boxed(inner) => {
                // Replace the inner type for FFI with whatever the `expose_as` told us to use.
                let inner = expose_as.unwrap_or(inner);
                if self.vec {
                    let ident = format_ident!("FFIArray{}", inner);
                    quote!(#ident)
                } else {
                    quote!(#ptr_type #inner)
                }
            }
            NativeType::DateTime => {
                if self.vec {
                    quote!(FFIArrayTimeStamp)
                } else {
                    quote!(#ptr_type TimeStamp)
                }
            }
            NativeType::Raw(inner) => {
                // Replace the inner type for FFI with whatever the `expose_as` told us to use.
                let inner = expose_as.unwrap_or(inner);
                if self.vec {
                    let ident = format_ident!("FFIArray{}", inner.to_string().to_camel_case());
                    quote!(#ident)
                } else if self.option {
                    // Option types are behind a pointer, because embedding structs in parameter
                    // lists caused issues for Swift.
                    quote!(#ptr_type #inner)
                } else {
                    quote!(#inner)
                }
            }
            NativeType::String | NativeType::Uuid => {
                if self.vec {
                    quote!(FFIArrayString)
                } else {
                    // Strings are always `*const`, unlike other reference types, because they're
                    // managed by the caller (since there's already language support for
                    // initializing a `String` from a view of foreign data, we don't need the
                    // preliminary step of allocating the data in Rust, which means we don't need to
                    // reclaim that memory here).
                    quote!(*const std::os::raw::c_char)
                }
            }
        }
    }

    /// Returns the name of this type in the consumer's language.
    ///
    #[must_use]
    pub fn consumer_type(&self, expose_as: Option<&Ident>) -> String {
        if let Some(expose_as) = expose_as {
            return expose_as.to_string();
        }
        let mut t = match &self.native_type {
            NativeType::Boxed(inner) => inner.to_string(),
            NativeType::Raw(inner) => crate::consumer_type_for(&inner.to_string(), false),
            NativeType::DateTime => "Date".to_string(),
            NativeType::String | NativeType::Uuid => "String".to_string(),
        };

        if self.vec {
            t = format!("[{}]", t)
        }

        if self.option {
            t = format!("{}?", t)
        }

        t
    }
}

/// Returns a `NativeTypeData` describing the native type for a custom FFI type, so we can use that
/// structure to generate the consumer structure just like we do with generated FFIs.
///
pub fn native_type_data_for_custom(ffi_type: &syn::Type) -> NativeTypeData {
    match ffi_type {
        Type::Path(type_path) => {
            let (ident, wrapping_type) = parsing::separate_wrapping_type_from_inner_type(
                type_path.path.segments.first().unwrap().clone(),
            );
            NativeTypeData::from((NativeType::from(ident), wrapping_type))
        }
        Type::Ptr(p) => {
            if let Type::Path(path) = p.elem.as_ref() {
                let type_name = path.path.segments.first().unwrap().ident.clone();
                // Treat pointer types as potentially optional. Since this is divorced from the
                // struct and we can't annotate items that we're not deriving from, we can't make
                // any guarantees about it's nullability.
                if type_name == "c_char" {
                    NativeTypeData {
                        native_type: NativeType::String,
                        option: true,
                        vec: false,
                    }
                } else {
                    NativeTypeData {
                        native_type: NativeType::Boxed(type_name),
                        option: true,
                        vec: false,
                    }
                }
            } else {
                panic!("No segment in {:?}?", p);
            }
        }
        _ => {
            panic!("Unsupported type: {:?}", ffi_type);
        }
    }
}
