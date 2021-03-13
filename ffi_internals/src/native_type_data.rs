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
    pub option: bool,
    /// Whether `ty` was discovered inside of a `Vec`, `Array`, or slice.
    pub vec: bool,
    /// Whether `ty` was discovered in the `Success` variant of a `Result`.
    pub result: bool,
}

impl UnparsedNativeTypeData {
    /// The initial state for `UnparsedNativeTypeData`, where the `option`, `vec` and `result`
    /// fields are all set to false.
    ///
    pub fn initial(ty: Type) -> Self {
        Self {
            ty,
            option: false,
            vec: false,
            result: false,
        }
    }
}

impl From<UnparsedNativeTypeData> for NativeTypeData {
    fn from(unparsed: UnparsedNativeTypeData) -> Self {
        match unparsed.ty {
            Type::Array(ty) => Self::from(UnparsedNativeTypeData {
                ty: *ty.elem,
                option: unparsed.option,
                vec: true,
                result: unparsed.result,
            }),
            Type::Path(ty) => {
                let segment = ty.path.segments.last().unwrap();
                let ident = segment.ident.clone();
                if ident == format_ident!("Option") {
                    if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                        if let syn::GenericArgument::Type(arg) = arguments.args.first().unwrap() {
                            Self::from(UnparsedNativeTypeData {
                                ty: arg.clone(),
                                option: true,
                                vec: unparsed.vec,
                                result: unparsed.result,
                            })
                        } else {
                            panic!("Unexpected arguments for Option: {:?}", arguments);
                        }
                    } else {
                        panic!("Unexpected segment contents for Option: {:?}", segment);
                    }
                } else if ident == format_ident!("Result") {
                    if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                        // Take the first generic argument, which is the success type.
                        if let syn::GenericArgument::Type(arg) = arguments.args.first().unwrap() {
                            Self::from(UnparsedNativeTypeData {
                                ty: arg.clone(),
                                option: unparsed.option,
                                vec: unparsed.vec,
                                result: true,
                            })
                        } else {
                            panic!("Unexpected arguments for Result: {:?}", arguments);
                        }
                    } else {
                        panic!("Unexpected segment contents for Result: {:?}", segment);
                    }
                } else if ident == format_ident!("Vec") {
                    if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                        if let syn::GenericArgument::Type(arg) = arguments.args.first().unwrap() {
                            Self::from(UnparsedNativeTypeData {
                                ty: arg.clone(),
                                option: unparsed.option,
                                vec: true,
                                result: unparsed.result,
                            })
                        } else {
                            panic!("Unexpected arguments for Vec: {:?}", arguments);
                        }
                    } else {
                        panic!("Unexpected segment contents for Vec: {:?}", segment);
                    }
                } else {
                    let native_type = NativeType::from(ident);
                    NativeTypeData {
                        native_type,
                        option: unparsed.option,
                        vec: unparsed.vec,
                        result: unparsed.result,
                    }
                }
            }
            Type::Ptr(ty) => Self::from(UnparsedNativeTypeData {
                ty: *ty.elem,
                option: unparsed.option,
                vec: unparsed.vec,
                result: unparsed.result,
            }),
            Type::Reference(ty) => Self::from(UnparsedNativeTypeData {
                ty: *ty.elem,
                option: unparsed.option,
                vec: unparsed.vec,
                result: unparsed.result,
            }),
            Type::Slice(ty) => Self::from(UnparsedNativeTypeData {
                ty: *ty.elem,
                option: unparsed.option,
                vec: true,
                result: unparsed.result,
            }),
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
