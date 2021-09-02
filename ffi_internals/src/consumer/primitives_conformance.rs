//!
//! Generates the required code for conformance with the consumer's protocols (or however they
//! define the common behavior across FFI types; consumer languages without a similar language
//! feature could simply provide full implementations here.)
//!

/// Generates a string with the protocol conformances for `native_type`. This needs to be written to
/// a file that can be copied to the consumer application/library/whatever.
///
/// - `native_type`: This is the native Rust type. It's not used as a type in the consumer interface
/// at all, since we've already wrapped it in FFI types (or, if it's already safe for C interop, the
/// consumer probably has its own name for the type).
/// - `ffi_type`: This is the type we use to represent `native_type` across the FFI boundary; i.e.,
/// this is the in-between type that gets used to pass information back and forth between Rust and
/// the FFI consumer.
/// - `consumer_type`: This is the way the consumer's language represents `native_type`. For a Rust
/// `u8`, Swift will use `UInt8`, etc.
///
pub(super) fn generate(native_type: &str, ffi_type: &str, consumer_type: &str) -> String {
    [
        array_conformance(
            &format!("FFIArray{}", native_type),
            ffi_type,
            &format!("ffi_array_{}_init", native_type),
            &format!("ffi_array_{}_free", native_type),
        ),
        option_conformance(
            consumer_type,
            ffi_type,
            &format!("option_{}_init", native_type),
            &format!("option_{}_free", native_type),
        ),
        consumer_type_base(consumer_type, ffi_type),
        consumer_array_type(consumer_type, &format!("FFIArray{}", native_type)),
    ]
    .join("")
}

/// Conversion from the consumer's native array type to the `FFIArray` type for `native_type`.
///
fn array_conformance(array_name: &str, ffi_type: &str, init: &str, free: &str) -> String {
    format!(
        "
extension {}: FFIArray {{
    public typealias Value = {}

    public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
        {}(ptr, len)
    }}

    public static func free(_ array: Self) {{
        {}(array)
    }}
}}
",
        array_name, ffi_type, init, free
    )
}

/// Conversion from the consumer's native optional type to the Option type for `native_type`.
///
fn option_conformance(consumer_type: &str, ffi_type: &str, init: &str, free: &str) -> String {
    format!(
        "
public extension Optional where Wrapped == {} {{
    func clone() -> UnsafeMutablePointer<{}>? {{
        switch self {{
        case let .some(value):
            let v = value.clone()
            return UnsafeMutablePointer(mutating: {}(true, v))
        case .none:
            return nil
        }}
    }}

    func borrowReference() -> UnsafeMutablePointer<{}>? {{
        switch self {{
        case let .some(value):
            let v = value.borrowReference()
            return UnsafeMutablePointer(mutating: {}(true, v))
        case .none:
            return nil
        }}
    }}
    
    static func fromRust(_ ptr: UnsafePointer<{}>?) -> Self {{
        guard let ptr = ptr else {{
            return .none
        }}
        let value = Wrapped.fromRust(ptr.pointee)
        free(ptr)
        return value
    }}
    
    static func free(_ option: UnsafePointer<{}>?) {{
        {}(option)
    }}
}}
",
        consumer_type, ffi_type, init, ffi_type, init, ffi_type, ffi_type, free
    )
}

/// Linking between the Rust and consumer base types.
///
fn consumer_type_base(consumer_type: &str, ffi_type: &str) -> String {
    format!(
        "
extension {}: NativeData {{
    public typealias ForeignType = {}

    public func clone() -> ForeignType {{
        return self
    }}

    public func borrowReference() -> ForeignType {{
        return self
    }}

    public static func fromRust(_ foreignObject: ForeignType) -> Self {{
        return foreignObject
    }}
}}
",
        consumer_type, ffi_type
    )
}

/// Linking between the Rust and consumer array types.
///
fn consumer_array_type(consumer_type: &str, ffi_array_type: &str) -> String {
    format!(
        "
extension {}: NativeArrayData {{
    public typealias FFIArrayType = {}
}}
",
        consumer_type, ffi_array_type
    )
}
