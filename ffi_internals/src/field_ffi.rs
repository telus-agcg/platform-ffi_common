//!
//! Contains structures describing the fields of a struct, and implementations for building the
//! related FFI and consumer implementations.
//!

use crate::{
    native_type_data::{Context, NativeType, NativeTypeData},
    parsing,
};
use heck::SnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Field, Ident, Path};

/// Field-level FFI helper attributes.
///
#[derive(Debug, Clone)]
pub struct FieldAttributes {
    /// If `Some`, a path to the type that this field should be exposed as. This type must meet
    /// some prerequisites:
    /// 1. It must be FFI-safe (either because it's a primitive value or derives its own FFI with
    /// `ffi_derive`).
    /// 1. It must have a `From<T> for U` impl, where `T` is the native type of the field and `U` is
    /// the type referenced by the `expose_as` `Path`.
    ///
    /// This is necessary for exposing remote types where we want to derive an FFI, but don't
    /// control the declaration of the type.
    ///
    pub expose_as: Option<Path>,

    /// Whether the field's data should be exposed as a raw value (i.e., not `Box`ed). This should
    /// only be applied to fields whose type is `repr(C)` and safe to expose over FFI.
    ///
    pub raw: bool,
}

impl FieldAttributes {
    /// If there's an `expose_as` attribute, get the ident of the last segment in the path (i.e.,
    /// the ident of the type being referenced).
    ///
    pub fn expose_as_ident(&self) -> Option<&Ident> {
        self.expose_as
            .as_ref()
            .map(|p| p.segments.last().map(|s| &s.ident))
            .flatten()
    }
}

/// Represents the components of the generated FFI for a field.
#[derive(Debug)]
pub struct FieldFFI {
    /// The type to which this field belongs.
    ///
    pub type_name: Ident,

    /// The field for which this interface is being generated.
    ///
    pub field_name: Ident,

    /// The type information for generating an FFI for this field.
    ///
    pub native_type_data: NativeTypeData,

    /// The FFI helper attribute annotations on this field.
    ///
    pub attributes: FieldAttributes,
}

impl FieldFFI {
    /// The name of the generated getter function. This is used to generate the Rust getter
    /// function, and the body of the consumer's getter, which ensures that they're properly linked.
    ///
    #[must_use]
    pub fn getter_name(&self) -> Ident {
        if self.native_type_data.option {
            format_ident!(
                "get_optional_{}_{}",
                self.type_name.to_string().to_snake_case(),
                self.field_name.to_string().to_snake_case()
            )
        } else {
            format_ident!(
                "get_{}_{}",
                self.type_name.to_string().to_snake_case(),
                self.field_name.to_string().to_snake_case()
            )
        }
    }

    /// An extern "C" function for returning the value of this field through the FFI. This takes a
    /// pointer to the struct and returns the field's value as an FFI-safe type, as in
    /// `pub extern "C" fn get_some_type_field(ptr: *const SomeType) -> FFIType`.
    ///
    #[must_use]
    pub fn getter_fn(&self) -> TokenStream {
        let field_name = &self.field_name;
        let type_name = &self.type_name;
        let getter_name = &self.getter_name();
        let ffi_type = &self
            .native_type_data
            .ffi_type(self.attributes.expose_as_ident(), &Context::Return);
        let conversion: TokenStream = if self.native_type_data.vec {
            if self.native_type_data.option {
                quote!(data.#field_name.as_deref().into())
            } else {
                quote!((&*data.#field_name).into())
            }
        } else {
            match &self.native_type_data.native_type {
                NativeType::Boxed(_) => {
                    if self.native_type_data.option {
                        let mut return_value = quote!(f.clone());
                        // If this field is exposed as a different type for FFI, convert it back to
                        // the native type.
                        if self.attributes.expose_as.is_some() {
                            return_value = quote!(#return_value.into())
                        }
                        quote!(
                            data.#field_name.as_ref().map_or(ptr::null(), |f| {
                                Box::into_raw(Box::new(#return_value))
                            })
                        )
                    } else {
                        let mut return_value = quote!(data.#field_name.clone());
                        // If this field is exposed as a different type for FFI, convert it back to
                        // the native type.
                        if self.attributes.expose_as.is_some() {
                            return_value = quote!(#return_value.into())
                        }
                        quote!(Box::into_raw(Box::new(#return_value)))
                    }
                }
                NativeType::DateTime => {
                    if self.native_type_data.option {
                        quote!(
                            data.#field_name.as_ref().map_or(ptr::null(), |f| {
                                Box::into_raw(Box::new(f.into()))
                            })
                        )
                    } else {
                        quote!(Box::into_raw(Box::new((&data.#field_name).into())))
                    }
                }
                NativeType::Raw(inner) => {
                    if self.native_type_data.option {
                        let boxer =
                            format_ident!("option_{}_init", inner.to_string().to_snake_case());
                        quote!(
                            match data.#field_name {
                                Some(data) => #boxer(true, data),
                                None => #boxer(false, #inner::default()),
                            }
                        )
                    } else {
                        quote!(data.#field_name.clone().into())
                    }
                }
                NativeType::String | NativeType::Uuid => {
                    if self.native_type_data.option {
                        quote!(
                            data.#field_name.as_ref().map_or(ptr::null(), |s| {
                                ffi_common::ffi_string!(s.to_string())
                            })
                        )
                    } else {
                        quote!(ffi_string!(data.#field_name.to_string()))
                    }
                }
            }
        };

        quote! {
            paste! {
                #[no_mangle]
                #[doc = "Get `" #field_name "` for this `" #type_name"`."]
                pub unsafe extern "C" fn #getter_name(
                    ptr: *const #type_name
                ) -> #ffi_type {
                    let data = &*ptr;
                    #conversion
                }
            }
        }
    }

    /// The memberwise initializer argument for passing a value for this field in to an FFI
    /// initializer.
    ///
    #[must_use]
    pub fn ffi_initializer_argument(&self) -> TokenStream {
        let field_name = &self.field_name;
        let ffi_type = &self
            .native_type_data
            .ffi_type(self.attributes.expose_as_ident(), &Context::Argument);
        quote!(#field_name: #ffi_type,)
    }

    /// Expression for assigning an argument to a field (with any required type conversion
    /// included).
    #[must_use]
    pub fn assignment_expression(&self) -> TokenStream {
        let field_name = &self.field_name;

        // All FFIArrayT types have a `From<FFIArrayT> for Vec<T>` impl, so we can treat them all
        // the same for the sake of native Rust assignment.
        if self.native_type_data.vec {
            return quote!(#field_name: #field_name.into(),);
        }

        match self.native_type_data.native_type {
            NativeType::Boxed(_) => {
                if self.attributes.expose_as.is_some() {
                    // The expose_as type will take care of its own optionality and cloning; all
                    // we need to do is make sure the pointer is safe (if this field is optional),
                    // then let it convert with `into()`.
                    if self.native_type_data.option {
                        quote! {
                            #field_name: unsafe {
                                if #field_name.is_null() {
                                    None
                                } else {
                                    (*Box::from_raw(#field_name)).into()
                                }
                            },
                        }
                    } else {
                        quote! {
                            #field_name: unsafe { (*Box::from_raw(#field_name)).into() },
                        }
                    }
                } else if self.native_type_data.option {
                    quote! {
                        #field_name: unsafe {
                            if #field_name.is_null() {
                                None
                            } else {
                                Some(*Box::from_raw(#field_name))
                            }
                        },
                    }
                } else {
                    quote!(#field_name: unsafe { *Box::from_raw(#field_name) },)
                }
            }
            NativeType::DateTime => {
                if self.native_type_data.option {
                    quote! {
                        #field_name: unsafe {
                            if #field_name.is_null() {
                                None
                            } else {
                                Some((&*Box::from_raw(#field_name)).into())
                            }
                        },
                    }
                } else {
                    quote!(#field_name: unsafe { (&*Box::from_raw(#field_name)).into() },)
                }
            }
            NativeType::Raw(_) => {
                if self.native_type_data.option {
                    quote! {
                        #field_name: unsafe {
                            if #field_name.is_null() {
                                None
                            } else {
                                Some(*Box::from_raw(#field_name))
                            }
                        },
                    }
                } else {
                    quote!(#field_name: #field_name,)
                }
            }
            NativeType::String => {
                if self.native_type_data.option {
                    quote! {
                        #field_name: if #field_name.is_null() {
                            None
                        } else {
                            Some(ffi_common::string::string_from_c(#field_name))
                        },
                    }
                } else {
                    quote!(#field_name: ffi_common::string::string_from_c(#field_name),)
                }
            }
            NativeType::Uuid => {
                if self.native_type_data.option {
                    quote! {
                        #field_name: if #field_name.is_null() {
                            None
                        } else {
                            Some(ffi_common::string::uuid_from_c(#field_name))
                        },
                    }
                } else {
                    quote!(#field_name: ffi_common::string::uuid_from_c(#field_name),)
                }
            }
        }
    }
}

impl From<(Ident, &Field, &HashMap<Ident, Ident>)> for FieldFFI {
    fn from(inputs: (Ident, &Field, &HashMap<Ident, Ident>)) -> Self {
        let (type_name, field, alias_map) = inputs;
        let field_name = field
            .ident
            .as_ref()
            .unwrap_or_else(|| {
                panic!(format!(
                    "Expected field: {:?} to have an identifier.",
                    &field
                ))
            })
            .clone();
        let attributes = parsing::parse_field_attributes(&field.attrs);
        let (wrapping_type, unaliased_field_type) = match parsing::get_segment_for_field(&field.ty)
        {
            Some(segment) => {
                let (ident, wrapping_type) =
                    parsing::separate_wrapping_type_from_inner_type(segment);
                (wrapping_type, resolve_type_alias(&ident, alias_map))
            }
            None => panic!("No path segment (field without a type?)"),
        };

        // If this has a raw attribute, bypass the normal `NativeType` logic and use `NativeType::raw`.
        let field_type = if attributes.raw {
            NativeType::Raw(unaliased_field_type)
        } else {
            NativeType::from(unaliased_field_type)
        };

        let native_type_data = NativeTypeData::from((field_type, wrapping_type));

        FieldFFI {
            type_name,
            field_name,
            native_type_data,
            attributes,
        }
    }
}

/// If `field_type` is an alias in `alias_map`, returns the underlying type (resolving aliases
/// recursively, so if someone is weird and defines typealiases over other typealiases, we'll still
/// find the underlying type, as long as they were all specified in the `alias_paths` helper
/// attribute).
///
fn resolve_type_alias(field_type: &Ident, alias_map: &HashMap<Ident, Ident>) -> Ident {
    match alias_map.get(field_type) {
        Some(alias) => resolve_type_alias(alias, alias_map),
        None => field_type.clone(),
    }
}
