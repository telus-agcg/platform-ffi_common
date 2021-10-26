//!
//! Contains structures describing a repr(C) enum, and implementations for building the wrapping
//! consumer implementation.
//!

use crate::{
    consumer::{
        consumer_enum::{CommonConsumerNames, ConsumerEnumType},
        ConsumerType,
    },
    items::enum_ffi::reprc,
    syn::Ident,
};

/// Contains the data required to generate a consumer type for `repr(C)` enums, which ought to be
/// any enums that don't have associated values, and associated functions for doing so.
///
pub struct ReprCConsumerEnum<'a> {
    ident: &'a Ident,
}

impl<'a> ReprCConsumerEnum<'a> {
    /// Constructor for `ReprCConsumerEnum`.
    ///
    #[must_use]
    pub const fn new(ident: &'a Ident) -> Self {
        ReprCConsumerEnum { ident }
    }
}

impl ConsumerEnumType for ReprCConsumerEnum<'_> {
    fn type_name_ident(&self) -> &Ident {
        self.ident
    }
}

impl ConsumerType for ReprCConsumerEnum<'_> {
    fn type_name(&self) -> String {
        self.type_name_ident().to_string()
    }

    fn type_definition(&self) -> String {
        // There's no type definition for repr(C) enums; instead, we extend the FFI enum since it's
        // usable as-is.
        String::default()
    }

    fn native_data_impl(&self) -> String {
        format!(
"// MARK: - NativeData
extension {type_name}: NativeData {{
    public typealias ForeignType = {type_name}

    public func clone() -> ForeignType {{
        return self
    }}

    public func borrowReference() -> ForeignType {{
        return self
    }}

    public static func fromRust(_ foreignObject: ForeignType) -> Self {{
        return foreignObject
    }}
}}",
            type_name = self.type_name_ident(),
        )
    }

    fn ffi_array_impl(&self) -> String {
        format!(
"// MARK: - FFIArray
extension {array_name}: FFIArray {{
    public typealias Value = {type_name}

    public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
        {array_init_fn_name}(ptr, len)
    }}

    public static func free(_ array: Self) {{
        {array_free_fn_name}(array)
    }}
}}",
            array_name = self.array_name(),
            type_name = self.type_name_ident(),
            array_init_fn_name = self.array_init_fn_name(),
            array_free_fn_name = self.array_free_fn_name(),
        )
    }

    fn native_array_data_impl(&self) -> String {
        format!(
"// MARK: - NativeArrayData
extension {type_name}: NativeArrayData {{
    public typealias FFIArrayType = {array_name}
}}",
            type_name = self.type_name_ident(),
            array_name = self.array_name(),
        )
    }

    fn option_impl(&self) -> String {
        format!(
"// MARK: - Optional
public extension Optional where Wrapped == {type_name} {{
    func clone() -> UnsafeMutablePointer<{type_name}>? {{
        switch self {{
        case let .some(value):
            let v = value.clone()
            return UnsafeMutablePointer(mutating: {option_init_fn_name}(true, v))
        case .none:
            return nil
        }}
    }}

    func borrowReference() -> UnsafeMutablePointer<{type_name}>? {{
        switch self {{
        case let .some(value):
            let v = value.borrowReference()
            return UnsafeMutablePointer(mutating: {option_init_fn_name}(true, v))
        case .none:
            return nil
        }}
    }}

    static func fromRust(_ ptr: UnsafePointer<{type_name}>?) -> Self {{
        guard let ptr = ptr else {{
            return .none
        }}
        let value = Wrapped.fromRust(ptr.pointee)
        free(ptr)
        return value
    }}

    static func free(_ option: UnsafePointer<{type_name}>?) {{
        {option_free_fn_name}(option)
    }}
}}",
            type_name = self.type_name_ident(),
            option_init_fn_name = self.option_init_fn_name(),
            option_free_fn_name = self.option_free_fn_name(),
        )
    }

    fn consumer_imports(&self) -> &[syn::Path] {
        &[]
    }
}

impl<'a> From<&reprc::EnumFFI<'a>> for ReprCConsumerEnum<'a> {
    fn from(ffi: &reprc::EnumFFI<'a>) -> Self {
        Self {
            ident: ffi.type_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quote::format_ident;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_type_definition() {
        let ident = format_ident!("TestType");
        let repr_c_enum = ReprCConsumerEnum::new(&ident);
        assert!(repr_c_enum.type_definition().is_empty());
    }

    #[test]
    fn native_data_impl() {
        let ident = format_ident!("TestType");
        let repr_c_enum = ReprCConsumerEnum::new(&ident);
        assert_eq!(
            repr_c_enum.native_data_impl(),
r#"// MARK: - NativeData
extension TestType: NativeData {
    public typealias ForeignType = TestType

    public func clone() -> ForeignType {
        return self
    }

    public func borrowReference() -> ForeignType {
        return self
    }

    public static func fromRust(_ foreignObject: ForeignType) -> Self {
        return foreignObject
    }
}"#
        );
    }

    #[test]
    fn ffi_array_impl() {
        let ident = format_ident!("TestType");
        let repr_c_enum = ReprCConsumerEnum::new(&ident);
        assert_eq!(
            repr_c_enum.ffi_array_impl(),
r#"// MARK: - FFIArray
extension FFIArrayTestType: FFIArray {
    public typealias Value = TestType

    public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {
        ffi_array_TestType_init(ptr, len)
    }

    public static func free(_ array: Self) {
        ffi_array_TestType_free(array)
    }
}"#
        );
    }

    #[test]
    fn native_array_data_impl() {
        let ident = format_ident!("TestType");
        let repr_c_enum = ReprCConsumerEnum::new(&ident);
        assert_eq!(
            repr_c_enum.native_array_data_impl(),
r#"// MARK: - NativeArrayData
extension TestType: NativeArrayData {
    public typealias FFIArrayType = FFIArrayTestType
}"#
        );
    }

    #[test]
    fn option_impl() {
        let ident = format_ident!("TestType");
        let repr_c_enum = ReprCConsumerEnum::new(&ident);
        assert_eq!(
            repr_c_enum.option_impl(),
r#"// MARK: - Optional
public extension Optional where Wrapped == TestType {
    func clone() -> UnsafeMutablePointer<TestType>? {
        switch self {
        case let .some(value):
            let v = value.clone()
            return UnsafeMutablePointer(mutating: option_TestType_init(true, v))
        case .none:
            return nil
        }
    }

    func borrowReference() -> UnsafeMutablePointer<TestType>? {
        switch self {
        case let .some(value):
            let v = value.borrowReference()
            return UnsafeMutablePointer(mutating: option_TestType_init(true, v))
        case .none:
            return nil
        }
    }

    static func fromRust(_ ptr: UnsafePointer<TestType>?) -> Self {
        guard let ptr = ptr else {
            return .none
        }
        let value = Wrapped.fromRust(ptr.pointee)
        free(ptr)
        return value
    }

    static func free(_ option: UnsafePointer<TestType>?) {
        option_TestType_free(option)
    }
}"#
        );
    }
}
