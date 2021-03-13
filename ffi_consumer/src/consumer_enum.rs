//!
//! Generates boilerplate code for using a repr(C) enum in the consumer's language.
//!

use heck::SnakeCase;

/// Contains the data required generate consumer support for a repr(C) enum.
///
pub struct ConsumerEnum {
    /// The name of the enum type.
    pub type_name: String,
}

impl ConsumerEnum {
    fn array_name(&self) -> String {
        format!("FFIArray{}", self.type_name)
    }

    fn array_init(&self) -> String {
        format!("ffi_array_{}_init", self.type_name.to_snake_case())
    }

    fn array_free(&self) -> String {
        format!("ffi_array_{}_free", self.type_name.to_snake_case())
    }

    fn option_init(&self) -> String {
        format!("option_{}_init", self.type_name.to_snake_case())
    }

    fn option_free(&self) -> String {
        format!("option_{}_free", self.type_name.to_snake_case())
    }

    fn array_conformance(&self) -> String {
        format!(
            r#"
extension {}: FFIArray {{
    typealias Value = {}

    static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
        {}(ptr, len)
    }}

    static func free(_ array: Self) {{
        {}(array)
    }}
}}
"#,
            self.array_name(),
            self.type_name,
            self.array_init(),
            self.array_free()
        )
    }

    fn option_conformance(&self) -> String {
        format!(
            r#"
extension Optional where Wrapped == {} {{
    func toRust() -> UnsafeMutablePointer<{}>? {{
        switch self {{
        case let .some(value):
            let v = value.toRust()
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

"#,
            self.type_name,
            self.type_name,
            self.option_init(),
            self.type_name,
            self.type_name,
            self.option_free(),
        )
    }

    /// Linking between the Rust and consumer base types.
    ///
    fn native_data_impl(&self) -> String {
        format!(
            r#"
extension {}: NativeData {{
    typealias ForeignType = {}

    func toRust() -> ForeignType {{
        return self
    }}

    static func fromRust(_ foreignObject: ForeignType) -> Self {{
        return foreignObject
    }}
}}
"#,
            self.type_name, self.type_name
        )
    }

    /// Linking between the Rust and consumer array types.
    ///
    fn consumer_array_type(&self) -> String {
        format!(
            r#"
extension {}: NativeArrayData {{
    typealias FFIArrayType = {}
}}
"#,
            self.type_name,
            self.array_name()
        )
    }
}

impl From<ConsumerEnum> for String {
    fn from(consumer: ConsumerEnum) -> Self {
        let mut result = crate::header();
        result.push_str(&consumer.native_data_impl());
        result.push_str(&consumer.array_conformance());
        result.push_str(&consumer.consumer_array_type());
        result.push_str(&consumer.option_conformance());
        result
    }
}
