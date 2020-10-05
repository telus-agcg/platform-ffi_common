//!
//! Common stuff used for generating the Rust FFI and the FFI consumer.
//!

use proc_macro2::TokenStream;
use syn::Ident;

/// Represents the components of the generated FFI for a field.
pub struct FieldFFI {
    /// The field for which this interface is being generated.
    ///
    pub field: Ident,

    /// An argument for passing a value for this field in to an FFI initializer. This should look
    /// like `field_name: FFIType,` -- *including the trailing comma*.
    pub argument: TokenStream,

    /// Expression for assigning an argument to a field (with any required type conversion
    /// included). This should look like `field_name: argument.into_native_field_type(),` --
    /// *including the trailing comma*.
    ///
    pub assignment_expression: TokenStream,

    /// The name of the generated getter function in `getter`. This **must** match the name of the
    /// function in `getter`, or FFI consumers will fail (either when building or at runtime,
    /// depending on the consumer language).
    ///
    pub getter_name: Ident,

    /// The type in which this field should be represented in the FFI consumer's native language.
    /// For example, in Swift a raw `Box<T>` pointer should be represented as an `OpaquePointer`, a
    /// string should be represented as a `String`, etc.
    ///
    pub consumer_type: String,

    /// An extern "C" function for returning the value of the field through the FFI. This should
    /// take a pointer to the struct and return the field's value as an FFI-safe type, as in `pub
    /// extern "C" fn get_some_type_field(ptr: *const SomeType) -> FFIType`.
    ///
    pub getter: TokenStream,
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
