//!
//! Common stuff used for generating the Rust FFI and the FFI consumer.
//!

use heck::{CamelCase, SnakeCase};
use proc_macro2::TokenStream;
use syn::Ident;
use quote::{quote, format_ident};

/// Field-level FFI helper attributes.
///
#[derive(Debug, Clone)]
pub struct FieldAttributes {
    /// If `Some`, the `Ident` of the type that this field should be exposed as. This type must meet
    /// some prerequisites:
    /// 1. It must be FFI-safe (either because it's a primitive value or derives its own FFI with
    /// `ffi_derive`).
    /// 1. It must have a `From<T> for U` impl, where `T` is the native type of the field and `U` is
    /// the type referenced by the `expose_as` `Ident`.
    /// 
    /// This is necessary for exposing remote types where we want to derive an FFI, but don't
    /// control the declaration of the type.
    /// 
    pub expose_as: Option<Ident>,

    /// Whether the field's data should be exposed as a raw value (i.e., not `Box`ed). This should
    /// only be applied to fields whose type is `repr(C)` and safe to expose over FFI.
    ///
    pub raw: bool,
}

/// The type of a field on a struct (from the perspective of generating an FFI).
///
#[derive(Debug, Clone)]
pub enum FieldType {
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

impl From<Ident> for FieldType {
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

/// Represents the components of the generated FFI for a field.
#[derive(Debug)]
pub struct FieldFFI {

    /// The type to which this field belongs.
    ///
    pub type_name: Ident,

    /// The field for which this interface is being generated.
    ///
    pub field_name: Ident,

    /// The native Rust type of the field.
    ///
    pub field_type: FieldType,

    /// The FFI helper attribute annotations on this field.
    ///
    pub attributes: FieldAttributes,

    /// True if this field is an `Option`, otherwise false.
    ///
    pub option: bool,

    /// True if this field is a `Vec`, otherwise false.
    ///
    pub vec: bool,
}

impl FieldFFI {
    /// The name of the generated getter function. This is used to generate the Rust getter
    /// function, and the body of the consumer's getter, which ensures that they're properly linked.
    ///
    #[must_use]
    pub fn getter_name(&self) -> Ident {
        if self.option {
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

    /// Returns the name of the type used for communicating this field's data across the FFI
    /// boundary.
    ///
    #[must_use]
    pub fn ffi_type(&self) -> TokenStream {
        match &self.field_type {
            FieldType::Boxed(inner) => {
                // Replace the inner type for FFI with whatever the `expose_as` told us to use.
                let inner = self.attributes.expose_as.as_ref().unwrap_or(inner);
                if self.vec {
                    let ident = format_ident!("FFIArray{}", inner);
                    quote!(#ident)
                } else {                    
                    quote!(*const #inner)
                }
            }
            FieldType::DateTime => {
                if self.vec {
                    quote!(FFIArrayTimeStamp)
                } else {
                    let mut t = format_ident!("TimeStamp");
                    if self.option { t = format_ident!("Option{}", t) }
                    quote!(#t)
                }
            }
            FieldType::Raw(inner) => {
                // Replace the inner type for FFI with whatever the `expose_as` told us to use.
                let inner = self.attributes.expose_as.as_ref().unwrap_or(inner);
                if self.vec {
                    let ident = format_ident!("FFIArray{}", inner.to_string().to_camel_case());
                    quote!(#ident)
                } else if self.option {
                    let ident = format_ident!("Option{}", inner.to_string().to_camel_case());
                    quote!(#ident)
                } else {
                    quote!(#inner)
                }
            }
            FieldType::String | FieldType::Uuid => {
                if self.vec {
                    quote!(FFIArrayString)
                } else {
                    quote!(*const std::os::raw::c_char)
                }
            }
        }
    }

    /// An extern "C" function for returning the value of the field through the FFI. This takes a
    /// pointer to the struct and returns the field's value as an FFI-safe type, as in
    /// `pub extern "C" fn get_some_type_field(ptr: *const SomeType) -> FFIType`.
    ///
    #[must_use]
    pub fn getter_body(&self) -> TokenStream {
        let field_name = &self.field_name;
        let type_name = &self.type_name;
        let getter_name = &self.getter_name();
        let ffi_type = &self.ffi_type();
        let conversion: TokenStream = if self.vec {
            if self.option {
                quote!(data.#field_name.as_deref().into()) 
            } else {
                quote!((&*data.#field_name).into()) 
            }
        } else {
            match self.field_type {
                FieldType::Boxed(_) => {
                    if self.option {
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
                FieldType::DateTime => {
                    if self.option {
                        quote!(data.#field_name.as_ref().into())
                    } else {
                        quote!((&data.#field_name).into())
                    }
                }
                FieldType::Raw(_) => {
                    if self.option {
                        quote!(data.#field_name.as_ref().into())
                    } else {
                        quote!(data.#field_name.clone().into())
                    }
                }
                FieldType::String | FieldType::Uuid=> {
                    if self.option {
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
        let ffi_type = &self.ffi_type();
        quote!(#field_name: #ffi_type,)
    }

    /// Expression for assigning an argument to a field (with any required type conversion
    /// included).
    #[must_use]
    pub fn assignment_expression(&self) -> TokenStream {
        let field_name = &self.field_name;

        // All FFIArrayT types have a `From<FFIArrayT> for Vec<T>` impl, so we can treat them all
        // the same for the sake of native Rust assignment.
        if self.vec {
            return quote!(#field_name: #field_name.into(),);
        }

        match self.field_type {
            FieldType::Boxed(_) => {
                if self.attributes.expose_as.is_some() {
                    // The expose_as type will take care of its own optionality and cloning; all
                    // we need to do is make sure the pointer is safe (if this field is optional),
                    // then let it convert with `into()`.
                    if self.option {
                        quote! {
                            #field_name: unsafe {
                                if #field_name.is_null() {
                                    None
                                } else {
                                    (&*#field_name).into()
                                }
                            },
                        }
                    } else {
                        quote! {
                            #field_name: unsafe { (&*#field_name).into() },
                        }
                    }
                } else if self.option {
                    quote! {
                        #field_name: unsafe {
                            if #field_name.is_null() {
                                None
                            } else {
                                Some((*#field_name).clone())
                            }
                        },
                    }
                } else {
                    quote!(#field_name: unsafe { (*#field_name).clone() },)
                }
            }
            // `DateTime` always uses `into` because it has special logic with `From` impls for
            // everything.
            FieldType::DateTime => quote!(#field_name: #field_name.into(),),
            FieldType::Raw(_) => {
                if self.option {
                    quote!(#field_name: #field_name.into(),)
                } else {
                    quote!(#field_name: #field_name,)
                } 
            }
            FieldType::String => {
                if self.option {
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
            FieldType::Uuid => {
                if self.option {
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

    /// Returns the name of this type in the consumer's language.
    ///
    #[must_use]
    pub fn consumer_type(&self) -> String {
        let mut t = match &self.field_type {
            FieldType::Boxed(inner) => inner.to_string(),
            FieldType::Raw(inner) => consumer_type_for(&inner.to_string(), false),
            FieldType::DateTime => "Date".to_string(),
            FieldType::String | FieldType::Uuid => "String".to_string(),
        };

        if self.vec { t = format!("[{}]", t) }

        if self.option { t = format!("{}?", t) }

        t
    }
}

/// Creates a consumer directory at `out_dir` and returns its path.
///
/// # Errors
///
/// Returns a `std::io::Error` if anything prevents us from creating `dir`.
///
pub fn create_consumer_dir(dir: &str) -> Result<&str, std::io::Error> {
    std::fs::create_dir_all(dir)?;
    Ok(dir)
}

/// Given a native type, this will return the type the consumer will use. If `native_type` is a
/// primitive, we'll match it with the corresponding primitive on the consumer's side. Otherwise,
/// we'll just return the type.
///
#[must_use]
pub fn consumer_type_for(native_type: &str, option: bool) -> String {
    let mut converted = match native_type {
        "u8" => "UInt8".to_string(),
        "u16" => "UInt16".to_string(),
        "u32" => "UInt32".to_string(),
        "u64" => "UInt64".to_string(),
        "i8" => "Int8".to_string(),
        "i16" => "Int16".to_string(),
        "i32" => "Int32".to_string(),
        "i64" => "Int64".to_string(),
        "f32" => "Float32".to_string(),
        "f64" => "Double".to_string(),
        "bool" => "Bool".to_string(),
        _ => native_type.to_string(),
    };
    if option {
        converted.push('?');
    }
    converted
}
