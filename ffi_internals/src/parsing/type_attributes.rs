//!
//! Contains data structures for describing and implementations for parsing a Rust type. Unlike the
//! other `*_attributes` modules, this doesn't deal with procedural macro attributes; instead, it
//! parses the low level types (represented by the `syn::Type` enum) into an
//! `ffi_internals::type_ffi::TypeFFI` so that the rest of `ffi_internals` has something relatively
//! straightforward to work with.
//!

use proc_macro_error::{abort, OptionExt};
use quote::format_ident;
use std::convert::TryFrom;
use syn::{spanned::Spanned, Ident, Type};

/// Describes the initial state when parsing a `syn::Type`, where we have not yet determined
/// whether the underlying type is wrapped in an `Option`, `Vec`, or `Result`.
///
/// This is basically an intermediary type to make it easier to get to `NativeTypeData`. Usage
/// should look something like this:
/// ```
/// use quote::format_ident;
/// use ffi_internals::{parsing::TypeAttributes, type_ffi::{TypeIdentifier, TypeFFI}};
///
/// let ty: syn::Type = syn::parse_str("Result<Foo>").unwrap();
/// let initial = TypeAttributes::initial(ty, vec![], None);
/// let native_type_data = TypeFFI::from(initial);
/// assert_eq!(native_type_data.native_type, TypeIdentifier::Boxed(format_ident!("Foo")));
/// assert_eq!(native_type_data.is_result, true);
/// assert_eq!(native_type_data.is_option, false);
/// assert_eq!(native_type_data.is_vec, false);
/// ```
///
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct TypeAttributes {
    /// The type being parsed.
    pub ty: Type,

    /// Whether `ty` was discovered inside of an `Option`.
    ///
    pub is_option: bool,

    /// Whether `ty` was discovered inside of a `Vec`, `Array`, or slice.
    ///
    pub is_collection: bool,

    /// Whether `ty` was discovered in the `Success` variant of a `Result`.
    ///
    pub is_result: bool,

    /// Whether `ty` was discovered inside of a `Cow`.
    ///
    pub is_cow: bool,

    /// Whether this field or parameter is a borrowed reference to a `ty`.
    ///
    pub is_borrow: bool,

    /// `Ident`s of types that ought to be exposed directly to the FFI in a `NativeType::Raw`, as
    /// opposed to being wrapped in a `Box`.
    ///
    pub raw_types: Vec<Ident>,

    self_type: Option<Ident>,
}

impl TypeAttributes {
    /// The initial state for `Unparsed`, where the `option`, `vec` and `result`
    /// fields are all set to false.
    ///
    #[must_use]
    pub fn initial(ty: Type, raw_types: Vec<Ident>, self_type: Option<Ident>) -> Self {
        Self {
            ty,
            is_option: false,
            is_collection: false,
            is_result: false,
            is_cow: false,
            is_borrow: false,
            raw_types,
            self_type,
        }
    }
}

#[derive(Debug)]
enum SupportedGeneric {
    Option,
    Vec,
    Result,
    Cow,
}

impl TryFrom<&str> for SupportedGeneric {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "Option" => Ok(Self::Option),
            "Vec" => Ok(Self::Vec),
            "Result" => Ok(Self::Result),
            "Cow" => Ok(Self::Cow),
            _ => {
                Err("Not a supported generic. Assume this is a non-generic type that we can parse.")
            }
        }
    }
}

impl From<TypeAttributes> for crate::type_ffi::TypeFFI {
    fn from(mut unparsed: TypeAttributes) -> Self {
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
                let segment = ty
                    .path
                    .segments
                    .last()
                    .expect_or_abort("Type path has zero segments.");
                let ident = segment.ident.clone();
                if let Ok(generic) = SupportedGeneric::try_from(&*ident.to_string()) {
                    match generic {
                        SupportedGeneric::Option => unparsed.is_option = true,
                        SupportedGeneric::Vec => unparsed.is_collection = true,
                        SupportedGeneric::Result => unparsed.is_result = true,
                        SupportedGeneric::Cow => unparsed.is_cow = true,
                    };
                    // Dig the argument type out of the generics for the limited cases we're
                    // supporting right now and update `unparsed` with its element type.
                    let arguments = match &segment.arguments {
                        syn::PathArguments::AngleBracketed(arguments) => arguments,
                        syn::PathArguments::Parenthesized(_) | syn::PathArguments::None => {
                            abort!(segment.arguments.span(), "`None` and `Parenthesized` path arguments are not currently supported.")
                        }
                    };
                    // If we're looking at a `Cow`, the type wrapped in the smart pointer is the
                    // last argument. Otherwise we're looking at a `Vec`, `Option`, or `Result`, in
                    // which case the type we want is the first argument.
                    let type_argument = if unparsed.is_cow {
                        arguments.args.last()
                    } else {
                        arguments.args.first()
                    }
                    .expect_or_abort("Generic type has no arguments");
                    let arg = match type_argument {
                        syn::GenericArgument::Type(ty) => ty,
                        syn::GenericArgument::Binding(_)
                        | syn::GenericArgument::Lifetime(_)
                        | syn::GenericArgument::Constraint(_)
                        | syn::GenericArgument::Const(_) => {
                            abort!(type_argument.span(), "`Lifetime`, `Binding`, `Constraint`, and `Const` generic arguments are not currently supported.")
                        }
                    };
                    unparsed.ty = arg.clone();
                    Self::from(unparsed)
                } else {
                    let mut ident = ident;
                    // If we have a `Self`, replace that with the actual type, since `Self` won't be
                    // in scope in our FFI module.
                    if ident == format_ident!("Self") {
                        ident = unparsed.self_type.expect(
                            "Found 'Self' type, but no `self_type` provided to replace it with.",
                        );
                    }
                    let native_type = if unparsed.raw_types.contains(&ident) {
                        crate::type_ffi::TypeIdentifier::Raw(ident)
                    } else {
                        crate::type_ffi::TypeIdentifier::from(ident)
                    };

                    Self {
                        native_type,
                        is_option: unparsed.is_option,
                        is_vec: unparsed.is_collection,
                        is_result: unparsed.is_result,
                        is_cow: unparsed.is_cow,
                        is_borrow: unparsed.is_borrow,
                    }
                }
            }
            Type::Ptr(ty) => {
                unparsed.ty = *ty.elem;
                Self::from(unparsed)
            }
            Type::Reference(ty) => {
                unparsed.is_borrow = true;
                unparsed.ty = *ty.elem;
                Self::from(unparsed)
            }
            Type::Slice(ty) => {
                unparsed.ty = *ty.elem;
                unparsed.is_collection = true;
                Self::from(unparsed)
            }
            _ => {
                abort!(unparsed.ty.span(), "Unsupported type.")
            }
        }
    }
}
