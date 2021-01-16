//!
//! # `ffi_internals`
//!
//! Contains all the parsing and common data structures used by `ffi_derive` and `ffi_consumer`, so
//! they can be shared between the codegen crates without needing to expose them in `ffi_common`,
//! which has more general FFI stuff.
//!

pub mod alias_resolution;
pub mod field_ffi;
pub mod native_type_data;
pub mod parsing;
pub mod struct_ffi;

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

pub fn write_consumer_files(type_name: &syn::Ident, consumer: String, out_dir: &str) {
    let consumer_dir = create_consumer_dir(out_dir)
        .unwrap_or_else(|e| panic!("Failed to create dir at {} with error {}.", out_dir, e));
    let output_file = format!("{}/{}.swift", consumer_dir, type_name.to_string());
    std::fs::write(&output_file, consumer)
        .unwrap_or_else(|e| panic!("Failed to write {} with error {}", output_file, e));
}
