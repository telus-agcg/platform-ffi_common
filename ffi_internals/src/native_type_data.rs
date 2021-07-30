//!
//! Contains data structures describing native type data, with implementations for calculating the
//! related FFI and consumer types.
//!

use crate::{parsing, parsing::WrappingType};
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

/// Describes a Rust type that is exposed via FFI (as the type of a field, or the type returned by a
/// function, or a function parameter, etc).
///
#[derive(Debug, Clone, PartialEq)]
pub enum NativeType {
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
pub struct NativeTypeData {
    /// The underlying type being exposed.
    ///
    pub native_type: NativeType,
    /// True if `native_type` is wrapped in an `Option`, otherwise false.
    ///
    pub option: bool,
    /// True if `native_type` is the type of the elements in a `Vec` or slice, otherwise false.
    ///
    pub vec: bool,
    /// True if `native_type` is the type of the `Success` variant of a `Result`, otherwise false.
    ///
    pub result: bool,
}

impl From<(NativeType, WrappingType)> for NativeTypeData {
    fn from(data: (NativeType, WrappingType)) -> Self {
        let (native_type, wrapping_type) = data;
        NativeTypeData {
            native_type,
            option: wrapping_type == WrappingType::Option
                || wrapping_type == WrappingType::OptionVec,
            vec: wrapping_type == WrappingType::Vec || wrapping_type == WrappingType::OptionVec,
            result: false,
        }
    }
}

impl NativeTypeData {
    pub fn argument_into_rust(
        &self,
        field_name: &Ident,
        has_custom_implementation: bool,
    ) -> TokenStream {
        // All FFIArrayT types have a `From<FFIArrayT> for Vec<T>` impl, so we can treat them all
        // the same for the sake of native Rust assignment.
        if self.vec {
            return quote!(#field_name.into());
        }

        match self.native_type {
            NativeType::Boxed(_) => {
                if has_custom_implementation {
                    // The expose_as type will take care of its own optionality and cloning; all
                    // we need to do is make sure the pointer is safe (if this field is optional),
                    // then let it convert with `into()`.
                    if self.option {
                        quote! {
                            unsafe {
                                if #field_name.is_null() {
                                    None
                                } else {
                                    (*Box::from_raw(#field_name)).into()
                                }
                            }
                        }
                    } else {
                        quote! {
                            unsafe { (*Box::from_raw(#field_name)).into() }
                        }
                    }
                } else if self.option {
                    quote! {
                        unsafe {
                            if #field_name.is_null() {
                                None
                            } else {
                                Some(*Box::from_raw(#field_name))
                            }
                        }
                    }
                } else {
                    quote!(unsafe { *Box::from_raw(#field_name) })
                }
            }
            NativeType::DateTime => {
                if self.option {
                    quote! {
                        unsafe {
                            if #field_name.is_null() {
                                None
                            } else {
                                Some((&*Box::from_raw(#field_name)).into())
                            }
                        }
                    }
                } else {
                    quote!(unsafe { (&*Box::from_raw(#field_name)).into() })
                }
            }
            NativeType::Raw(_) => {
                if self.option {
                    quote! {
                        unsafe {
                            if #field_name.is_null() {
                                None
                            } else {
                                Some(*Box::from_raw(#field_name))
                            }
                        }
                    }
                } else {
                    quote!(#field_name)
                }
            }
            NativeType::String => {
                if self.option {
                    quote! {
                        if #field_name.is_null() {
                            None
                        } else {
                            Some(ffi_common::ffi_core::string::string_from_c(#field_name))
                        }
                    }
                } else {
                    quote!(ffi_common::ffi_core::string::string_from_c(#field_name))
                }
            }
            NativeType::Uuid => {
                if self.option {
                    quote! {
                        if #field_name.is_null() {
                            None
                        } else {
                            Some(ffi_common::ffi_core::string::uuid_from_c(#field_name))
                        }
                    }
                } else {
                    quote!(ffi_common::ffi_core::string::uuid_from_c(#field_name))
                }
            }
        }
    }
}

/// Describes the initial state when parsing a `syn::Type`, where we have not yet determined
/// whether the underlying type is wrapped in an `Option`, `Vec`, or `Result`.
///
/// This is basically an intermediary type to make it easier to get to `NativeTypeData`. Usage
/// should look something like this:
/// ```
/// use quote::format_ident;
/// use ffi_internals::native_type_data::{UnparsedNativeTypeData, NativeTypeData, NativeType};
///
/// let ty: syn::Type = syn::parse_str("Result<Foo>").unwrap();
/// let initial = UnparsedNativeTypeData::initial(ty);
/// let native_type_data = NativeTypeData::from(initial);
/// assert_eq!(native_type_data.native_type, NativeType::Boxed(format_ident!("Foo")));
/// assert_eq!(native_type_data.result, true);
/// assert_eq!(native_type_data.option, false);
/// assert_eq!(native_type_data.vec, false);
/// ```
///
#[derive(Debug, Clone)]
pub struct UnparsedNativeTypeData {
    /// The type being parsed.
    pub ty: Type,
    /// Whether `ty` was discovered inside of an `Option`.
    pub is_option: bool,
    /// Whether `ty` was discovered inside of a `Vec`, `Array`, or slice.
    pub is_collection: bool,
    /// Whether `ty` was discovered in the `Success` variant of a `Result`.
    pub is_result: bool,
}

impl UnparsedNativeTypeData {
    /// The initial state for `UnparsedNativeTypeData`, where the `option`, `vec` and `result`
    /// fields are all set to false.
    ///
    pub fn initial(ty: Type) -> Self {
        Self {
            ty,
            is_option: false,
            is_collection: false,
            is_result: false,
        }
    }
}

enum SupportedGeneric {
    Option,
    Vec,
    Result,
}
use std::convert::TryFrom;
impl TryFrom<&str> for SupportedGeneric {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Option" => Ok(Self::Option),
            "Vec" => Ok(Self::Vec),
            "Result" => Ok(Self::Result),
            _ => {
                Err("Not a supported generic. Assume this is a non-generic type that we can parse.")
            }
        }
    }
}

impl From<UnparsedNativeTypeData> for NativeTypeData {
    fn from(mut unparsed: UnparsedNativeTypeData) -> Self {
        // Note that this match intentionally performs a partial move. If we need to call this
        // recursively, we'll be passing `unparsed` back to the same method, but we should always
        // have updated `unparsed.ty` with the newly discovered type. The partial move ensures that
        // the compiler will yell at you if you forget to assign a new value to `unparsed.ty`. (And
        // if we don't have a new type to update `unparsed.ty` with, we're not dealing with a
        // generic so we're at the point where we can convert to a `NativeTypeData`.)
        match unparsed.ty {
            Type::Array(ty) => {
                unparsed.ty = *ty.elem;
                unparsed.is_collection = true;
                Self::from(unparsed)
            }
            Type::Path(ty) => {
                let segment = ty.path.segments.last().unwrap();
                let ident = segment.ident.clone();
                if let Ok(generic) = SupportedGeneric::try_from(&*ident.to_string()) {
                    match generic {
                        SupportedGeneric::Option => {
                            unparsed.is_option = true;
                        }
                        SupportedGeneric::Vec => unparsed.is_collection = true,
                        SupportedGeneric::Result => unparsed.is_result = true,
                    };
                    // Dig the argument type out of the generics for the limited cases we're
                    // supporting right now and update `unparsed` with its element type.
                    let arguments = match &segment.arguments {
                        syn::PathArguments::AngleBracketed(arguments) => arguments,
                        syn::PathArguments::Parenthesized(_) | syn::PathArguments::None => {
                            panic!("`None` and `Parenthesized` path arguments are not currently supported.")
                        }
                    };
                    let arg = match arguments.args.first().unwrap() {
                        syn::GenericArgument::Type(ty) => ty,
                        syn::GenericArgument::Binding(_)
                        | syn::GenericArgument::Lifetime(_)
                        | syn::GenericArgument::Constraint(_)
                        | syn::GenericArgument::Const(_) => {
                            panic!("`Lifetime`, `Binding`, `Constraint`, and `Const` generic arguments are not currently supported.")
                        }
                    };
                    unparsed.ty = arg.clone();
                    Self::from(unparsed)
                } else {
                    let native_type = NativeType::from(ident);
                    NativeTypeData {
                        native_type,
                        option: unparsed.is_option,
                        vec: unparsed.is_collection,
                        result: unparsed.is_result,
                    }
                }
            }
            Type::Ptr(ty) => {
                unparsed.ty = *ty.elem;
                Self::from(unparsed)
            }
            Type::Reference(ty) => {
                unparsed.ty = *ty.elem;
                Self::from(unparsed)
            }
            Type::Slice(ty) => {
                unparsed.ty = *ty.elem;
                unparsed.is_collection = true;
                Self::from(unparsed)
            }
            Type::TraitObject(_)
            | Type::Tuple(_)
            | Type::BareFn(_)
            | Type::Group(_)
            | Type::ImplTrait(_)
            | Type::Infer(_)
            | Type::Macro(_)
            | Type::Never(_)
            | Type::Paren(_)
            | Type::Verbatim(_)
            | _ => {
                panic!("Unsupported type: {:?}", unparsed.ty);
            }
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
                    let ident = format_ident!("FFIArray{}", inner.to_string());
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

    pub fn owned_native_type(&self) -> TokenStream {
        let t = match &self.native_type {
            NativeType::Boxed(inner) => quote!(#inner),
            NativeType::DateTime => quote!(datetime),
            NativeType::Raw(inner) => quote!(#inner),
            NativeType::String => quote!(String),
            NativeType::Uuid => quote!(Uuid),
        };
        let t = if self.vec {
            quote!(Vec::<#t>)
        } else {
            quote!(#t)
        };
        let t = if self.option { quote!(Option::<#t>) } else { t };
        t
    }
}

/// Returns a `NativeTypeData` describing the native type for a custom FFI type, so we can use that
/// structure to generate the consumer structure just like we do with generated FFIs.
///
pub fn native_type_data_for_custom(ffi_type: &Type) -> NativeTypeData {
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
                        result: false,
                    }
                } else {
                    NativeTypeData {
                        native_type: NativeType::Boxed(type_name),
                        option: true,
                        vec: false,
                        result: false,
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
