//!
//! Contains structures describing the data for a Rust type, and implementations for building the
//! related FFI and consumer implementations.
//!

use crate::{
    parsing,
    parsing::{FieldAttributes, WrappingType},
};
use heck::SnakeCase;
use proc_macro2::TokenStream;
use proc_macro_error::{abort, OptionExt};
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Ident, Type};

const STRING: &str = "String";
const STR: &str = "str";
const DATETIME: &str = "NaiveDateTime";
const UUID: &str = "Uuid";
const BOOL: &str = "bool";
const U8: &str = "u8";
const U16: &str = "u16";
const U32: &str = "u32";
const U64: &str = "u64";
const I8: &str = "i8";
const I16: &str = "i16";
const I32: &str = "i32";
const I64: &str = "i64";
const F32: &str = "f32";
const F64: &str = "f64";

/// Describes a Rust type that is exposed via FFI (as the type of a field, or the type returned by a
/// function, or a function parameter, etc).
///
#[derive(Debug, Clone, PartialEq)]
pub enum TypeIdentifier {
    /// A type that should be exposed behind on opaque pointer; we'll make this available as a
    /// `*const T`, and consumers of that interface will be able to initialize, free, and access
    /// properties on the type from getter functions.
    ///
    Boxed(Ident),
    /// A timestamp that's safe to expose across the FFI (see `ffi_core::datetime`).
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

impl From<Ident> for TypeIdentifier {
    fn from(type_path: Ident) -> Self {
        match &*type_path.to_string() {
            DATETIME => Self::DateTime,
            STRING | STR => Self::String,
            UUID => Self::Uuid,
            BOOL | U8 | U16 | U32 | U64 | I8 | I16 | I32 | I64 | F32 | F64 => Self::Raw(type_path),
            _other => Self::Boxed(type_path),
        }
    }
}

/// The context in which a type is being referenced. Sometimes the mutability of a reference, or
/// even the type it's exposed as is different depending on whether it's being returned to the
/// consumer or passed in as an argument to a Rust function.
///
#[derive(Debug, Clone, Copy)]
pub enum Context {
    /// Type is being used as an argument to a Rust function.
    Argument,
    /// Type is being returned to the consumer.
    Return,
}

/// Describes the supported language-level generic wrappers around a `NativeType`, so that we can
/// expose an `Option<Foo>` or even a `Result<Vec<Foo>>`.
///
/// It's worth noting that these are only supported one level deep; we won't be able to expose a
/// `Vec<Vec<Foo>>` without making some larger improvements to the way we parse types.
///
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct TypeFFI {
    /// The underlying type being exposed.
    ///
    pub native_type: TypeIdentifier,
    /// True if `native_type` is wrapped in an `Option`, otherwise false.
    ///
    pub is_option: bool,
    /// True if `native_type` is the type of the elements in a `Vec` or slice, otherwise false.
    ///
    pub is_vec: bool,
    /// True if `native_type` is the type of the `Success` variant of a `Result`, otherwise false.
    ///
    pub is_result: bool,
    /// True if `native_type` is wrapped in a `Cow`, otherwise false.
    ///
    pub is_cow: bool,
    /// True if we're dealing with a borrowed reference to `native_type`./
    ///
    pub is_borrow: bool,
}

impl From<(TypeIdentifier, WrappingType)> for TypeFFI {
    fn from(data: (TypeIdentifier, WrappingType)) -> Self {
        let (native_type, wrapping_type) = data;
        Self {
            native_type,
            is_option: wrapping_type == WrappingType::Option
                || wrapping_type == WrappingType::OptionVec,
            is_vec: wrapping_type == WrappingType::Vec || wrapping_type == WrappingType::OptionVec,
            is_result: false,
            is_cow: false,
            is_borrow: false,
        }
    }
}

impl TypeFFI {
    /// Generates a `TokenStream` for turning an argument of the FFI type represented by `self` into
    /// a native Rust type.
    ///
    #[must_use]
    pub fn argument_into_rust(
        &self,
        field_name: &TokenStream,
        has_custom_implementation: bool,
    ) -> TokenStream {
        // All FFIArrayT types have a `From<FFIArrayT> for Vec<T>` impl, so we can treat them all
        // the same for the sake of native Rust assignment.
        if self.is_vec {
            return quote!(#field_name.into());
        }

        match self.native_type {
            TypeIdentifier::Boxed(_) if has_custom_implementation => {
                // The expose_as type will take care of its own optionality and cloning; all
                // we need to do is make sure the pointer is safe (if this field is optional),
                // then let it convert with `into()`.
                let (conversion_or_borrow, none) = if self.is_borrow {
                    (quote!(&*#field_name), quote!(&None))
                } else {
                    (quote!((*Box::from_raw(#field_name)).into()), quote!(None))
                };
                if self.is_option {
                    quote! {
                        if #field_name.is_null() {
                            #none
                        } else {
                            #conversion_or_borrow
                        }
                    }
                } else {
                    quote! {
                        #conversion_or_borrow
                    }
                }
            }
            TypeIdentifier::Boxed(_) if self.is_option => {
                let (conversion_or_borrow, none) = if self.is_borrow {
                    (quote!(Some(&*#field_name)), quote!(&None))
                } else {
                    (quote!(Some(*Box::from_raw(#field_name))), quote!(None))
                };
                quote! {
                    if #field_name.is_null() {
                        #none
                    } else {
                        #conversion_or_borrow
                    }
                }
            }
            TypeIdentifier::Boxed(_) => {
                let conversion_or_borrow = if self.is_borrow {
                    quote!(&*#field_name)
                } else {
                    quote!(*Box::from_raw(#field_name))
                };
                quote!(#conversion_or_borrow)
            }
            TypeIdentifier::DateTime if self.is_option => {
                quote! {
                    if #field_name.is_null() {
                        None
                    } else {
                        Some((&*Box::from_raw(#field_name)).into())
                    }
                }
            }
            TypeIdentifier::DateTime => {
                quote!((&*Box::from_raw(#field_name)).into())
            }
            TypeIdentifier::Raw(_) if self.is_option => {
                quote! {
                    if #field_name.is_null() {
                        None
                    } else {
                        Some(*Box::from_raw(#field_name))
                    }
                }
            }
            TypeIdentifier::Raw(_) => {
                quote!(#field_name)
            }
            TypeIdentifier::String if self.is_option => {
                quote! {
                    if #field_name.is_null() {
                        None
                    } else {
                        Some(ffi_common::core::string::string_from_c(#field_name))
                    }
                }
            }
            TypeIdentifier::String => {
                quote!(ffi_common::core::string::string_from_c(#field_name))
            }
            TypeIdentifier::Uuid if self.is_option => {
                quote! {
                    if #field_name.is_null() {
                        None
                    } else {
                        Some(ffi_common::core::string::uuid_from_c(#field_name))
                    }
                }
            }
            TypeIdentifier::Uuid => {
                quote!(ffi_common::core::string::uuid_from_c(#field_name))
            }
        }
    }

    /// Generates a `TokenStream` for turning an argument of the Rust type represented by `self` into
    /// an FFI type.
    ///
    #[must_use]
    pub fn rust_to_ffi_value(
        &self,
        accessor: &TokenStream,
        attributes: &FieldAttributes,
    ) -> TokenStream {
        if self.is_vec {
            if self.is_option {
                quote!(#accessor.as_deref().into())
            } else {
                quote!((&*#accessor).into())
            }
        } else {
            match &self.native_type {
                TypeIdentifier::Boxed(_) => {
                    if self.is_option {
                        let mut return_value = quote!(f.clone());
                        // If this field is exposed as a different type for FFI, convert it back to
                        // the native type.
                        if attributes.expose_as.is_some() {
                            return_value = quote!(#return_value.into());
                        }
                        quote!(
                            #accessor.as_ref().map_or(ptr::null(), |f| {
                                Box::into_raw(Box::new(#return_value))
                            })
                        )
                    } else {
                        let mut return_value = quote!(#accessor.clone());
                        // If this field is exposed as a different type for FFI, convert it back to
                        // the native type.
                        if attributes.expose_as.is_some() {
                            return_value = quote!(#return_value.into());
                        }
                        quote!(Box::into_raw(Box::new(#return_value)))
                    }
                }
                TypeIdentifier::DateTime => {
                    if self.is_option {
                        quote!(
                            #accessor.as_ref().map_or(ptr::null(), |f| {
                                Box::into_raw(Box::new(f.into()))
                            })
                        )
                    } else {
                        quote!(Box::into_raw(Box::new((&#accessor).into())))
                    }
                }
                TypeIdentifier::Raw(inner) => {
                    if self.is_option {
                        let boxer =
                            format_ident!("option_{}_init", inner.to_string().to_snake_case());
                        quote!(
                            match #accessor {
                                Some(data) => #boxer(true, data),
                                None => #boxer(false, #inner::default()),
                            }
                        )
                    } else {
                        quote!(#accessor.clone().into())
                    }
                }
                TypeIdentifier::String | TypeIdentifier::Uuid => {
                    if self.is_option {
                        quote!(
                            #accessor.as_ref().map_or(ptr::null(), |s| {
                                ffi_common::core::ffi_string!(s.to_string())
                            })
                        )
                    } else {
                        quote!(ffi_common::core::ffi_string!(#accessor.to_string()))
                    }
                }
            }
        }
    }

    /// Returns true if we support borrowed arguments for this variant of `NativeType`, otherwise
    /// false.
    ///
    pub(crate) const fn argument_borrows_supported(&self) -> bool {
        // If it's not a borrow, or we're dealing with a collection type, it's not borrowed. We'll
        // probably add support for collection types eventually, but it's not essential yet.
        if !self.is_borrow || self.is_vec {
            return false;
        }
        match self.native_type {
            // Boxed and DateTime types are always exposed via pointer, so they're fine to borrow.
            TypeIdentifier::Boxed(_) | TypeIdentifier::DateTime => true,
            // Raw types are passed through the FFI by value; there's no reason to borrow them.
            // String/Uuid are certainly worth supporting borrows for, but we're not there yet.
            TypeIdentifier::Raw(_) | TypeIdentifier::String | TypeIdentifier::Uuid => false,
        }
    }

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
    pub fn ffi_type(&self, expose_as: Option<&Ident>, context: Context) -> TokenStream {
        let ptr_type = match context {
            Context::Argument => quote!(*mut),
            Context::Return => quote!(*const),
        };
        match &self.native_type {
            TypeIdentifier::Boxed(inner) => {
                // Replace the inner type for FFI with whatever the `expose_as` told us to use.
                let inner = expose_as.unwrap_or(inner);
                if self.is_vec {
                    let ident = format_ident!("FFIArray{}", inner);
                    quote!(#ident)
                } else {
                    quote!(#ptr_type #inner)
                }
            }
            TypeIdentifier::DateTime => {
                if self.is_vec {
                    quote!(FFIArrayTimeStamp)
                } else {
                    quote!(#ptr_type TimeStamp)
                }
            }
            TypeIdentifier::Raw(inner) => {
                // Replace the inner type for FFI with whatever the `expose_as` told us to use.
                let inner = expose_as.unwrap_or(inner);
                if self.is_vec {
                    let ident = format_ident!("FFIArray{}", inner.to_string());
                    quote!(#ident)
                } else if self.is_option {
                    // Option types are behind a pointer, because embedding structs in parameter
                    // lists caused issues for Swift.
                    quote!(#ptr_type #inner)
                } else {
                    quote!(#inner)
                }
            }
            TypeIdentifier::String | TypeIdentifier::Uuid => {
                if self.is_vec {
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
        let mut t = expose_as.map_or_else(
            {
                || match &self.native_type {
                    TypeIdentifier::Boxed(inner) => inner.to_string(),
                    TypeIdentifier::Raw(inner) => {
                        crate::consumer_type_for(&inner.to_string(), false)
                    }
                    TypeIdentifier::DateTime => "Date".to_string(),
                    TypeIdentifier::String | TypeIdentifier::Uuid => "String".to_string(),
                }
            },
            std::string::ToString::to_string,
        );

        if self.is_vec {
            t = format!("[{}]", t);
        }

        if self.is_option {
            t = format!("{}?", t);
        }

        t
    }

    /// Generates a `TokenStream` of `self` as a native Rust type, for converting an FFI type back
    /// into native Rust (generally to call a function or initialize a struct).
    ///
    #[must_use]
    pub fn native_type(&self) -> TokenStream {
        let t = match &self.native_type {
            TypeIdentifier::Boxed(inner) | TypeIdentifier::Raw(inner) => quote!(#inner),
            TypeIdentifier::DateTime => quote!(datetime),
            TypeIdentifier::String => quote!(String),
            TypeIdentifier::Uuid => quote!(Uuid),
        };
        let t = if self.is_vec {
            quote!(Vec::<#t>)
        } else if self.is_borrow {
            if self.native_type == TypeIdentifier::String {
                quote!(&str)
            } else {
                quote!(&#t)
            }
        } else {
            t
        };
        if self.is_option {
            quote!(Option::<#t>)
        } else {
            t
        }
    }

    /// Returns a tuple containing 1) the conversion operation to perform for this type on the
    /// consumer side, 2) a closing parenthesis, and 3) the signature for returning this type from a
    /// consumer function.
    ///
    pub(crate) fn consumer_return_type_components(&self) -> (String, String, String) {
        let ty = self.consumer_type(None);
        if self.is_result {
            (
                "handle(result: ".to_string(),
                ")".to_string(),
                format!("-> Result<{}, RustError>", ty),
            )
        } else {
            (
                format!("{}.fromRust(", ty),
                ")".to_string(),
                format!("-> {}", ty),
            )
        }
    }
}

impl From<(&Type, bool)> for TypeFFI {
    /// Returns a `NativeTypeData` describing the native type for a custom FFI type, so we can use that
    /// structure to generate the consumer structure just like we do with generated FFIs.
    ///
    fn from(value: (&Type, bool)) -> Self {
        let (ffi_type, required) = value;
        match ffi_type {
            Type::Path(type_path) => {
                let (ident, wrapping_type) = parsing::separate_wrapping_type_from_inner_type(
                    type_path
                        .path
                        .segments
                        .first()
                        .expect_or_abort("msg")
                        .clone(),
                );
                Self::from((TypeIdentifier::from(ident), wrapping_type))
            }
            Type::Ptr(p) => {
                if let Type::Path(path) = p.elem.as_ref() {
                    let type_name = path
                        .path
                        .segments
                        .first()
                        .expect_or_abort("msg")
                        .ident
                        .clone();
                    // Unless a parameter is explicitly labeled as "required", treat pointer types
                    // as potentially optional. Since this is divorced from the struct, and we can't
                    // annotate items that we're not deriving from, we can't make any guarantees
                    // about the nullability of getter fn return types or assume that parameters are
                    // required.
                    let is_option = !required;
                    let native_type = if type_name == "c_char" {
                        TypeIdentifier::String
                    } else {
                        TypeIdentifier::Boxed(type_name)
                    };
                    Self {
                        native_type,
                        is_option,
                        is_vec: false,
                        is_result: false,
                        is_cow: false,
                        is_borrow: false,
                    }
                } else {
                    abort!(p.span(), "No segment in {:?}?", p);
                }
            }
            _ => {
                abort!(ffi_type.span(), "Unsupported type: {:?}", ffi_type);
            }
        }
    }
}
