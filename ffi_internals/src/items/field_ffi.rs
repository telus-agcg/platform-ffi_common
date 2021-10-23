//!
//! Contains structures describing the fields of a struct, and implementations for building the
//! related FFI and consumer implementations.
//!

use crate::{
    alias_resolution, parsing,
    type_ffi::{Context, TypeFFI, TypeIdentifier},
};
use heck::SnakeCase;
use parsing::FieldAttributes;
use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Attribute, Fields, Ident};

#[derive(Debug, Clone)]
pub(crate) enum FieldSource<'a> {
    Struct,
    Enum {
        variant_ident: &'a Ident,
        variant_fields_len: usize,
        other_variants: Vec<(Ident, usize)>,
    },
}

/// Represents the components of a field for generating an FFI.
///
#[derive(Debug)]
pub struct FieldFFI<'a> {
    /// The type to which this field belongs.
    ///
    pub(crate) type_name: &'a Ident,

    /// The field for which this interface is being generated.
    ///
    pub field_name: FieldIdent,

    /// The type of structure to which this field belongs (i.e. a struct or an enum).
    ///
    pub(crate) field_source: FieldSource<'a>,

    /// The type information for generating an FFI for this field.
    ///
    pub native_type_data: TypeFFI,

    /// The FFI helper attribute annotations on this field.
    ///
    pub attributes: FieldAttributes,

    /// Documentation comments on this field.
    ///
    pub(crate) doc_comments: Vec<Attribute>,
}

impl<'a> FieldFFI<'a> {
    /// The name of the generated getter function. This is used to generate the Rust getter
    /// function, and the body of the consumer's getter, which ensures that they're properly linked.
    ///
    #[must_use]
    pub fn getter_name(&self) -> Ident {
        let mut getter_name = "get_".to_string();
        if self.native_type_data.is_option {
            getter_name.push_str("optional_");
        }
        getter_name.push_str(&self.type_name.to_string().to_snake_case());
        getter_name.push('_');
        if let FieldSource::Enum {
            variant_ident,
            variant_fields_len: _,
            other_variants: _,
        } = &self.field_source
        {
            getter_name.push_str(&variant_ident.to_string().to_snake_case());
            getter_name.push('_');
        }
        getter_name.push_str(&self.field_name.ffi_ident().to_string().to_snake_case());
        format_ident!("{}", getter_name)
    }

    /// An extern "C" function for returning the value of this field through the FFI. This takes a
    /// pointer to the struct and returns the field's value as an FFI-safe type, as in
    /// `pub extern "C" fn get_some_type_field(ptr: *const SomeType) -> FFIType`.
    ///
    #[must_use]
    pub fn getter_fn(&self) -> TokenStream {
        let type_name = self.type_name;
        let getter_name = self.getter_name();
        let ffi_type = self
            .native_type_data
            .ffi_type(self.attributes.expose_as_ident(), Context::Return);
        match &self.field_source {
            FieldSource::Struct => {
                let field_name = &self.field_name.rust_token();
                let ffi_type = &self
                    .native_type_data
                    .ffi_type(self.attributes.expose_as_ident(), Context::Return);
                let accessor = quote!(data.#field_name);
                let conversion = self
                    .native_type_data
                    .rust_to_ffi_value(&accessor, &self.attributes);

                quote! {
                    ffi_common::core::paste! {
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
            FieldSource::Enum {
                variant_ident,
                variant_fields_len: _,
                other_variants,
            } => {
                if other_variants.iter().any(|v| &&v.0 == variant_ident) {
                    proc_macro_error::abort!(
                        self.type_name.span(),
                        "Internal error: `other_variants` contains `variant`"
                    );
                }
                let accessor = quote!(data);
                let conversion = self
                    .native_type_data
                    .rust_to_ffi_value(&accessor, &self.attributes);

                let valid_arm = quote!(#type_name::#variant_ident(#accessor) => #conversion,);

                let invalid_arms = other_variants
                    .iter()
                    .fold(quote!(), |mut acc, variant| {
                        let (variant_ident, variant_field_count) = &variant;
                        let argument = if *variant_field_count == 0 {
                            quote!()
                        } else {
                            quote!((..))
                        };
                        acc.extend(quote!(#type_name::#variant_ident#argument => unreachable!("This arm is unreachable."),));
                        acc
                    });

                quote! {
                    ffi_common::core::paste! {
                        #[no_mangle]
                        pub unsafe extern "C" fn #getter_name(
                            ptr: *const #type_name
                        ) -> #ffi_type {
                            match &*ptr {
                                #valid_arm
                                #invalid_arms
                            }
                        }
                    }
                }
            }
        }
    }

    /// The memberwise initializer argument for passing a value for this field in to an FFI
    /// initializer.
    ///
    #[must_use]
    pub fn ffi_initializer_argument(&self) -> TokenStream {
        let field_name = &self.field_name.ffi_ident();
        let ffi_type = &self
            .native_type_data
            .ffi_type(self.attributes.expose_as_ident(), Context::Argument);
        quote!(#field_name: #ffi_type,)
    }

    /// Expression for assigning an argument to a field (with any required type conversion
    /// included).
    #[must_use]
    pub fn assignment_expression(&self) -> TokenStream {
        let ffi_ident = &self.field_name.ffi_ident();
        let conversion = self
            .native_type_data
            .argument_into_rust(&quote!(#ffi_ident), self.attributes.expose_as.is_some());
        match &self.field_source {
            FieldSource::Struct => {
                let field_name = &self.field_name.rust_token();
                quote!(#field_name: #conversion,)
            }
            FieldSource::Enum {
                variant_ident: _,
                variant_fields_len: _,
                other_variants: _,
            } => quote!(#conversion,),
        }
    }
}

/// The type of field identifier, which may be identified by the field's name, or, in the case of a
/// tuple struct, its index.
///
#[derive(Debug, Clone)]
#[allow(variant_size_differences)]
pub enum FieldIdent {
    /// A named field like `bar` in  `struct Foo { bar: Baz }`. This variant contains the field's
    /// identifier.
    ///
    NamedField(Ident),
    /// An unnamed field in a tuple struct like `struct Foo(Bar)`. This variant contains the field's
    /// index.
    ///
    UnnamedField(usize),
}

impl FieldIdent {
    /// Returns the Rust identifier for accessing this field. (Note that this is a `TokenStream`
    /// rather than an `Ident` because `0` is not a valid `Ident`.)
    ///
    #[must_use]
    fn rust_token(&self) -> TokenStream {
        match self {
            FieldIdent::NamedField(ident) => quote!(#ident),
            FieldIdent::UnnamedField(index) => {
                let index = syn::Index::from(index.to_owned());
                quote!(#index)
            }
        }
    }

    /// Returns the FFI identifier for accessing this field. In the case of a
    /// `FieldIdent::NamedField`, this will simply be the field's name. In the case of a
    /// `FieldIdent::UnnamedField`, we can't just use an index like `0` to reference it, so we
    /// construct an identifier like `unnamed_field_0`.
    ///
    #[must_use]
    fn ffi_ident(&self) -> Ident {
        match self {
            FieldIdent::NamedField(ident) => ident.clone(),
            FieldIdent::UnnamedField(index) => quote::format_ident!("unnamed_field_{}", index),
        }
    }

    /// Returns the consumer identifier for accessing this field. The consumer must be able to call
    /// the FFI using matching identifiers, so this is just `ffi_ident()` converted to a `String`.
    ///
    #[must_use]
    pub(crate) fn consumer_ident(&self) -> String {
        self.ffi_ident().to_string()
    }
}

pub(super) struct FieldInputs<'a> {
    pub type_ident: &'a Ident,
    pub field_ident: FieldIdent,
    pub field_type: &'a syn::Type,
    pub field_source: FieldSource<'a>,
    pub field_attrs: &'a [Attribute],
    pub alias_modules: &'a [String],
}

#[must_use]
pub(super) fn field_inputs_from_unnamed_fields<'a>(
    fields: &'a syn::FieldsUnnamed,
    field_source: &FieldSource<'a>,
    type_name: &'a Ident,
    alias_modules: &'a [String],
) -> Vec<FieldInputs<'a>> {
    fields
        .unnamed
        .iter()
        .enumerate()
        .map(|(index, field)| FieldInputs {
            type_ident: type_name,
            field_ident: FieldIdent::UnnamedField(index),
            field_type: &field.ty,
            field_source: field_source.clone(),
            field_attrs: &*field.attrs,
            alias_modules,
        })
        .collect()
}

#[must_use]
pub(super) fn field_inputs_from_named_fields<'a>(
    fields: &'a syn::FieldsNamed,
    field_source: &FieldSource<'a>,
    type_name: &'a Ident,
    alias_modules: &'a [String],
) -> Vec<FieldInputs<'a>> {
    fields
        .named
        .iter()
        .map(|field| {
            let field_ident = field.ident.as_ref().unwrap_or_else(|| {
                proc_macro_error::abort!(field.span(), "Expected field to have an identifier.")
            });
            FieldInputs {
                type_ident: type_name,
                field_ident: FieldIdent::NamedField(field_ident.clone()),
                field_type: &field.ty,
                field_source: field_source.clone(),
                field_attrs: &*field.attrs,
                alias_modules,
            }
        })
        .collect()
}

/// Builds a `Vec` of `FieldFFI` for the fields of an enum variant.
///
#[must_use]
pub fn fields_for_variant<'a>(
    type_name: &'a Ident,
    alias_modules: &'a [String],
    variant_ident: &'a Ident,
    variant_fields: &'a Fields,
    other_variants: Vec<(Ident, usize)>,
) -> Vec<FieldFFI<'a>> {
    match &variant_fields {
        Fields::Named(fields) => field_inputs_from_named_fields(
            fields,
            &FieldSource::Enum {
                variant_ident,
                variant_fields_len: fields.named.len(),
                other_variants,
            },
            type_name,
            alias_modules,
        ),
        Fields::Unnamed(fields) => field_inputs_from_unnamed_fields(
            fields,
            &FieldSource::Enum {
                variant_ident,
                variant_fields_len: fields.unnamed.len(),
                other_variants,
            },
            type_name,
            alias_modules,
        ),
        Fields::Unit => {
            return Vec::new();
        }
    }
    .into_iter()
    .map(FieldFFI::from)
    .collect()
}

impl<'a> From<FieldInputs<'a>> for FieldFFI<'a> {
    fn from(inputs: FieldInputs<'a>) -> Self {
        let attributes = FieldAttributes::from(inputs.field_attrs);
        let (wrapping_type, unaliased_field_type) =
            match parsing::get_segment_for_field(inputs.field_type) {
                Some(segment) => {
                    let (ident, wrapping_type) =
                        parsing::separate_wrapping_type_from_inner_type(segment);
                    (
                        wrapping_type,
                        alias_resolution::resolve_type_alias(&ident, inputs.alias_modules, None)
                            .unwrap_or_else(|err| {
                                abort!(&inputs.field_type.span(), "Alias resolution error: {}", err)
                            }),
                    )
                }
                None => {
                    abort!(
                        inputs.field_type.span(),
                        "No path segment (field without a type?"
                    )
                }
            };

        // If this has a raw attribute, bypass the normal `NativeType` logic and use `NativeType::raw`.
        let field_type = if attributes.raw {
            TypeIdentifier::Raw(unaliased_field_type)
        } else {
            TypeIdentifier::from(unaliased_field_type)
        };

        let native_type_data = TypeFFI::from((field_type, wrapping_type));

        Self {
            type_name: inputs.type_ident,
            field_name: inputs.field_ident,
            field_source: inputs.field_source,
            native_type_data,
            attributes,
            doc_comments: parsing::parse_doc_comments(inputs.field_attrs),
        }
    }
}
