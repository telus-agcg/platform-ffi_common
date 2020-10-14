//!
//! Generates a wrapping type in the consumer's language, including a native initializer, a
//! deinitializer implementation that calls the appropriate `free_*` method for the Rust struct, and
//! native getters for reading properties from the Rust struct.
//!

use ffi_common::codegen_helpers::FieldFFI;
use heck::{MixedCase, SnakeCase};

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
    let (consumer_init_args, ffi_init_args, consumer_getters) = expand_fields(&*fields_ffi);
    consumer.push_str(&consumer_type(
        type_name,
        &consumer_init_args,
        &ffi_init_args,
        &consumer_getters,
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

/// Generates a consumer wrapper for a type that has a custom FFI implementation.
///
#[must_use]
pub fn generate_custom(
    type_name: &str,
    init_fn_name: &str,
    init_args: &[(syn::Ident, syn::Type)],
    getters: &[(syn::Ident, syn::Type)],
    free_fn_name: &str,
) -> String {
    let mut consumer = crate::header();
    let array_name = format!("FFIArray{}", type_name);

    let arg_count = init_args.len();
    let (consumer_init_args, ffi_init_args) = init_args.iter().enumerate().fold(
        (String::new(), String::new()),
        |mut acc, (index, (i, t))| {
            // Swift rejects trailing commas on argument lists.
            let trailing_punctuation = if index < arg_count - 1 { ",\n" } else { "" };
            // This looks like `foo: Bar,`.
            acc.0.push_str(&format!(
                "{:<8}: {}{}",
                i.to_string(),
                consumer_type_for_ffi_type(t),
                trailing_punctuation
            ));
            // This looks like `foo.toRust(),`.
            acc.1.push_str(&format!(
                "{:<12}.toRust(){}",
                i.to_string(),
                trailing_punctuation
            ));
            acc
        },
    );

    let type_prefix = format!("get_{}_", type_name.to_snake_case());
    let consumer_getters = getters.iter().fold(String::new(), |mut acc, (i, t)| {
        let consumer_type = consumer_type_for_ffi_type(t);
        let consumer_getter_name = i
            .to_string()
            .split(&type_prefix)
            .last()
            .unwrap()
            .to_string()
            .to_mixed_case();
        acc.push_str(&format!(
            "public var {:<4}: {} {{
{:<8}.fromRust({}(pointer))
}}

",
            consumer_getter_name,
            consumer_type,
            consumer_type,
            i.to_string()
        ));
        acc
    });

    consumer.push_str(&consumer_type(
        type_name,
        &consumer_init_args,
        &ffi_init_args,
        &consumer_getters,
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

fn consumer_type_for_ffi_type(ffi_type: &syn::Type) -> String {
    match ffi_type {
        syn::Type::Path(path) => {
            // TODO: This is extra terrible/hacky, won't work for many cases. We need DEV-13175.
            ffi_common::codegen_helpers::consumer_type_for(
                &path.path.segments.first().unwrap().ident.to_string(),
                false,
            )
        }
        syn::Type::Ptr(p) => {
            if let syn::Type::Path(path) = p.elem.as_ref() {
                let type_name = path.path.segments.first().unwrap().ident.to_string();
                if type_name == "c_char" {
                    "String".to_string()
                } else {
                    type_name
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

/// Expands a `&[FieldFFI]` to a tuple of consumer initializer arguments, FFI initializer
/// arguments, and consumer getters for accessing the Rust fields.
///
fn expand_fields(fields_ffi: &[FieldFFI]) -> (String, String, String) {
    fields_ffi.iter().enumerate().fold(
        (String::new(), String::new(), String::new()),
        |mut acc, (index, f)| {
            // Swift rejects trailing commas on argument lists.
            let trailing_punctuation = if index < fields_ffi.len() - 1 {
                ",\n"
            } else {
                ""
            };
            // This looks like `foo: Bar,`.
            acc.0.push_str(&format!(
                "{:<8}: {}{}",
                f.field_name.to_string(),
                f.consumer_type(),
                trailing_punctuation
            ));
            // This looks like `foo.toRust(),`.
            acc.1.push_str(&format!(
                "{:<12}.toRust(){}",
                f.field_name.to_string(),
                trailing_punctuation
            ));
            // This looks like `public var foo: Bar { Bar.fromRust(get_bar_foo(pointer) }`.
            acc.2.push_str(&format!(
                "public var {:<4}: {} {{
{:<8}.fromRust({}(pointer))
}}

",
                f.field_name.to_string(),
                f.consumer_type(),
                f.consumer_type(),
                f.getter_name().to_string()
            ));
            acc
        },
    )
}

/// Generates a wrapper for a struct so that the native interface in the consumer's language
/// correctly wraps the generated FFI module.
///
fn consumer_type(
    type_name: &str,
    consumer_init_args: &str,
    ffi_init_args: &str,
    consumer_getters: &str,
    init_fn_name: &str,
    free_fn_name: &str,
) -> String {
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
        type_name, consumer_init_args, init_fn_name, ffi_init_args, free_fn_name, consumer_getters
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
