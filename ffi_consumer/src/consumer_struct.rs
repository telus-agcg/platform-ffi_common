//!
//! Generates a wrapping type in the consumer's language, including a native initializer, a
//! deinitializer implementation that calls the appropriate `free_*` method for the Rust struct, and
//! native getters for reading properties from the Rust struct.
//!

use ffi_common::codegen_helpers::FieldFFI;
use heck::SnakeCase;

/// Returns a string for a consumer type that wraps `type_name`. This needs to be written to a file
/// that can be copied to the consumer library/application/whatever.
///
#[must_use]
pub fn generate(
    type_name: &str,
    fields_ffi: &[FieldFFI],
    init_fn_name: &str,
    free_fn_name: &str,
) -> String {
    let mut consumer = crate::header();
    let array_name = format!("FFIArray{}", type_name);
    consumer.push_str(&consumer_type(
        type_name,
        fields_ffi,
        init_fn_name,
        free_fn_name,
    ));
    consumer.push_str(&ffi_array_impl(
        &array_name,
        &format!("ffi_array_{}_init", type_name.to_snake_case()),
        &format!("free_ffi_array_{}", type_name.to_snake_case()),
    ));
    consumer.push_str(&consumer_base_impl(type_name));
    consumer.push_str(&consumer_option_impl(type_name));
    consumer.push_str(&consumer_array_impl(type_name, &array_name));
    consumer
}

/// Generates a wrapper for a struct so that the native interface in the consumer's language
/// correctly wraps the generated FFI module.
///
fn consumer_type(
    type_name: &str,
    fields_ffi: &[FieldFFI],
    init_fn_name: &str,
    free_fn_name: &str,
) -> String {
    let (native_init_arguments, ffi_init_arguments, field_access) =
        fields_ffi.iter().enumerate().fold(
            (String::new(), String::new(), String::new()),
            |mut acc, (index, f)| {
                // Swift rejects trailing commas on argument lists.
                let trailing_punctuation = if index < fields_ffi.len() - 1 {
                    ",\n"
                } else {
                    ""
                };
                acc.0.push_str(&format!(
                    "{:<8}: {}{}",
                    f.field.to_string(),
                    f.consumer_type.to_string(),
                    trailing_punctuation
                ));
                acc.1.push_str(&format!(
                    "{:<12}.toRust(){}",
                    f.field.to_string(),
                    trailing_punctuation
                ));
                acc.2.push_str(&format!(
                    "public var {:<4}: {} {{
{:<8}.fromRust({}(pointer))
    }}

",
                    f.field.to_string(),
                    f.consumer_type.to_string(),
                    f.consumer_type.to_string(),
                    f.getter_name.to_string()
                ));
                acc
            },
        );
    let wrapper = format!(
        r#"public final class {} {{
    private let pointer: OpaquePointer

    public init(
{:<4}
    ) {{
        self.pointer = {}(
{:<12}
        )
    }}

    private init(_ pointer: OpaquePointer) {{
        self.pointer = pointer
    }}

    deinit {{
{:<8}(pointer)
    }}

{:<4}}}
"#,
        type_name,
        native_init_arguments,
        init_fn_name,
        ffi_init_arguments,
        free_fn_name,
        field_access
    );

    wrapper
}

fn ffi_array_impl(array_name: &str, array_init_fn: &str, array_free_fn: &str) -> String {
    format!(
        r#"
extension {}: FFIArray {{
    typealias Value = OpaquePointer?

    static var defaultValue: Value {{ nil }}

    static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
{:<8}(ptr, len)
    }}

    static func free(_ array: Self) {{
{:<8}(array)
    }}
}}
"#,
        array_name, array_init_fn, array_free_fn
    )
}

fn consumer_base_impl(type_name: &str) -> String {
    format!(
        r#"
extension {}: NativeData {{
    typealias ForeignType = OpaquePointer?

    static var defaultValue: Self {{ fatalError() }}

    func toRust() -> ForeignType {{
        return pointer
    }}

    static func fromRust(_ foreignObject: ForeignType) -> Self {{
        return Self(foreignObject!)
    }}
}}
"#,
        type_name
    )
}

fn consumer_option_impl(type_name: &str) -> String {
    format!(
        r#"
extension {}: NativeOptionData {{
    typealias FFIOptionType = OpaquePointer?
}}
"#,
        type_name
    )
}

fn consumer_array_impl(type_name: &str, array_name: &str) -> String {
    format!(
        r#"
extension {}: NativeArrayData {{
    typealias FFIArrayType = {}
}}
"#,
        type_name, array_name
    )
}
