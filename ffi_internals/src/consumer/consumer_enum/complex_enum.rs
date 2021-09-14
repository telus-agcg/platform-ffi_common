use super::{CommonConsumerNames, ConsumerEnumType};
use crate::{
    consumer::{ConsumerType, TAB_SIZE},
    struct_internals::enum_ffi::EnumFFI,
    syn::Ident,
};
use heck::MixedCase;

/// Contains the data required to generate a consumer type for a complex (i.e., non-`repr(C)`) enum,
/// and associated functions for doing so.
///
pub struct ComplexConsumerEnum<'a> {
    enum_ffi: &'a EnumFFI<'a>,
}

impl<'a> ComplexConsumerEnum<'a> {
    /// Constructor for `ComplexConsumerEnum`.
    ///
    #[must_use]
    pub const fn new(enum_ffi: &'a EnumFFI<'_>) -> Self {
        ComplexConsumerEnum { enum_ffi }
    }
}

impl ConsumerEnumType for ComplexConsumerEnum<'_> {
    fn type_name(&self) -> &Ident {
        self.enum_ffi.type_name
    }
}

impl ComplexConsumerEnum<'_> {
    fn case_definitions(&self) -> String {
        self.enum_ffi
            .variants
            .iter()
            .map(|variant| {
                let ident = variant.ident.to_string().to_mixed_case();
                let field_types: Vec<String> = variant
                    .fields
                    .iter()
                    .map(|field| field.native_type_data.consumer_type(None))
                    .collect();
                // Some variants of an enum may not have an associated value (i.e., have zero
                // fields); we need to support those because an enum cannot be repr(C) if it has one
                // or more variants with associated values.
                let associated_values = if field_types.is_empty() {
                    String::default()
                } else {
                    format!("({}.FFI, {})", self.type_name(), field_types.join(", "))
                };
                format!(
                    "{spacer:l1$}case {ident}{associated_values}",
                    spacer = " ",
                    l1 = TAB_SIZE,
                    ident = ident,
                    associated_values = associated_values,
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn ffi_declaration(&self) -> String {
        format!(
            r#"
{spacer:l1$}public final class FFI {{
{spacer:l2$}internal let pointer: OpaquePointer

{spacer:l2$}internal init(_ pointer: OpaquePointer) {{
{spacer:l3$}self.pointer = pointer
{spacer:l2$}}}

{spacer:l2$}deinit {{
{spacer:l3$}{free_fn_name}(pointer)
{spacer:l2$}}}
{spacer:l1$}}}
"#,
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            l3 = TAB_SIZE * 3,
            free_fn_name = self.enum_ffi.free_fn_name(),
        )
    }

    fn enum_protocol_conformance(&self) -> String {
        format!(
            r#"// MARK: - ForeignEnum
extension {type_name}.FFI: ForeignEnum {{
{spacer:l1$}public typealias NativeEnumType = {type_name}

{spacer:l1$}public func makeNative() -> NativeEnumType {{
{spacer:l2$}switch {get_variant_fn_name}(pointer) {{
{make_native_cases}
{spacer:l2$}default:
{spacer:l3$}fatalError("Unreachable")
{spacer:l2$}}}
{spacer:l1$}}}
}}

// MARK: - NativeEnum
extension {type_name}: NativeEnum {{
{spacer:l1$}public typealias FFIType = Self.FFI

{spacer:l1$}public var ffi: FFI {{
{spacer:l2$}switch self {{
{spacer:l2$}case
{ffi_assignment}
{spacer:l2$}:
{spacer:l3$}return ffi
{spacer:l2$}}}
{spacer:l1$}}}

{spacer:l1$}public static func fromRust(pointer: FFIType.ForeignType) -> Self {{
{spacer:l2$}return FFI.fromRust(pointer).makeNative()
{spacer:l1$}}}
}}"#,
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            l3 = TAB_SIZE * 3,
            type_name = self.type_name(),
            get_variant_fn_name = self.enum_ffi.get_variant_fn_name(),
            make_native_cases = self.make_native_cases(),
            ffi_assignment = self.ffi_assignment(),
        )
    }

    fn case_inits(&self) -> String {
        self.enum_ffi
            .variants
            .iter()
            .map(|variant| {
                let (arguments, conversions) = match variant.fields.len() {
                    0 => (String::default(), String::default()),
                    1 => (
                        format!(
                            "_ data: {}",
                            variant
                                .fields
                                .first()
                                .unwrap()
                                .native_type_data
                                .consumer_type(None)
                        ),
                        "data.clone()".to_string(),
                    ),
                    _ => (
                        format!(
                            "_ data: ({})",
                            variant
                                .fields
                                .iter()
                                .map(|field| field.native_type_data.consumer_type(None))
                                .collect::<Vec<String>>()
                                .join(", ")
                        ),
                        variant
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(index, _)| format!("data.{}.clone()", index))
                            .collect::<Vec<String>>()
                            .join(","),
                    ),
                };
                format!(
                    r#"{spacer:l1$}static func {consumer_variant_name}({arguments}) -> Self {{
{spacer:l2$}FFI({variant_init_fn_name}({conversions})).makeNative()
{spacer:l1$}}}"#,
                    spacer = " ",
                    l1 = TAB_SIZE,
                    l2 = TAB_SIZE * 2,
                    arguments = arguments,
                    consumer_variant_name = variant.ident.to_string().to_mixed_case(),
                    variant_init_fn_name = variant.init_fn_name(self.type_name()),
                    conversions = conversions,
                )
            })
            .collect::<Vec<String>>()
            .join("\n\n")
    }

    fn make_native_cases(&self) -> String {
        self.enum_ffi
            .variants
            .iter()
            .map(|variant| {
                let ffi_variant_ident = format!("{}_{}", self.enum_ffi.reprc_enum(), variant.ident);
                let field_getters: Vec<String> = variant
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{spacer:l4$}{field_getter_name}(pointer)",
                            spacer = " ",
                            l4 = TAB_SIZE * 4,
                            field_getter_name = field.getter_name()
                        )
                    })
                    .collect();
                format!(
                    r#"
{spacer:l2$}case {ffi_variant_ident}:
{spacer:l3$}return .{consumer_variant_ident}(
{spacer:l4$}self,
{field_getters}
{spacer:l3$})
"#,
                    spacer = " ",
                    l2 = TAB_SIZE * 2,
                    l3 = TAB_SIZE * 3,
                    l4 = TAB_SIZE * 4,
                    ffi_variant_ident = ffi_variant_ident,
                    consumer_variant_ident = variant.ident.to_string().to_mixed_case(),
                    field_getters = field_getters.join(",\n"),
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn ffi_assignment(&self) -> String {
        self.enum_ffi
            .variants
            .iter()
            .map(|variant| {
                format!(
                    "{spacer:l3$}let .{variant_name}(ffi, {placeholders})",
                    spacer = " ",
                    l3 = TAB_SIZE * 3,
                    variant_name = variant.ident.to_string().to_mixed_case(),
                    placeholders = vec!["_"; variant.fields.len()].join(", "),
                )
            })
            .collect::<Vec<String>>()
            .join(",\n")
    }
}

impl ConsumerType for ComplexConsumerEnum<'_> {
    fn type_definition(&self) -> String {
        format!(
            r#"
/// Doesn't require any special memory behavior; it's a swift type containing other types (and those
/// contained types will manage their own memory as needed).
///
/// When accessing the associated values on this type, keep in mind that the first value is an aid
/// for the FFI; you can safely ignore it with `_` when destructuring.
///
public enum {type_name} {{
{case_definitions}

{case_inits}
}}

// MARK: - FFI
extension {type_name} {{
{ffi_declaration}
}}

{enum_protocol_conformance}
"#,
            type_name = self.type_name(),
            case_definitions = self.case_definitions(),
            case_inits = self.case_inits(),
            ffi_declaration = self.ffi_declaration(),
            enum_protocol_conformance = self.enum_protocol_conformance(),
        )
    }

    fn native_data_impl(&self) -> String {
        format!(
            r#"
// MARK: - NativeData
extension {type_name}.FFI: NativeData {{
{spacer:l1$}public typealias ForeignType = OpaquePointer?

{spacer:l1$}/// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
{spacer:l1$}/// used when calling a Rust function that takes ownership of an instance (like an initializer
{spacer:l1$}/// with a parameter of this type).
{spacer:l1$}public func clone() -> ForeignType {{
{spacer:l2$}return {clone_fn_name}(pointer)
{spacer:l1$}}}

{spacer:l1$}/// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
{spacer:l1$}/// must only be used when calling Rust functions that take a borrowed reference; otherwise,
{spacer:l1$}/// Rust will free `pointer` while this instance retains it.
{spacer:l1$}public func borrowReference() -> ForeignType {{
{spacer:l2$}return pointer
{spacer:l1$}}}

{spacer:l1$}/// Initializes an instance of this type from a pointer to an instance of the Rust type.
{spacer:l1$}public static func fromRust(_ foreignObject: ForeignType) -> Self {{
{spacer:l2$}return Self(foreignObject!)
{spacer:l1$}}}
}}

extension {type_name}: NativeData {{
{spacer:l1$}public typealias ForeignType = FFIType.ForeignType

{spacer:l1$}/// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
{spacer:l1$}/// used when calling a Rust function that takes ownership of an instance (like an initializer
{spacer:l1$}/// with a parameter of this type).
{spacer:l1$}public func clone() -> FFIType.ForeignType {{
{spacer:l2$}ffi.clone()
{spacer:l1$}}}

{spacer:l1$}/// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
{spacer:l1$}/// must only be used when calling Rust functions that take a borrowed reference; otherwise,
{spacer:l1$}/// Rust will free `pointer` while this instance retains it.
{spacer:l1$}public func borrowReference() -> FFIType.ForeignType {{
{spacer:l2$}ffi.borrowReference()
{spacer:l1$}}}

{spacer:l1$}/// Initializes an instance of this type from a pointer to an instance of the Rust type.
{spacer:l1$}public static func fromRust(_ foreignObject: FFIType.ForeignType) -> Self {{
{spacer:l2$}Self.FFIType.fromRust(foreignObject).makeNative()
{spacer:l1$}}}
}}
"#,
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            type_name = self.type_name(),
            clone_fn_name = self.enum_ffi.clone_fn_name(),
        )
    }

    fn ffi_array_impl(&self) -> String {
        format!(
            r#"
extension {array_name}: FFIArray {{
{spacer:l1$}public typealias Value = OpaquePointer?

{spacer:l1$}public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
{spacer:l2$}{array_init_fn_name}(ptr, len)
{spacer:l1$}}}

{spacer:l1$}public static func free(_ array: Self) {{
{spacer:l2$}{array_free_fn_name}(array)
{spacer:l1$}}}
}}
"#,
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            array_name = self.array_name(),
            array_init_fn_name = self.array_init_fn_name(),
            array_free_fn_name = self.array_free_fn_name()
        )
    }

    fn native_array_data_impl(&self) -> String {
        format!(
            r#"
// MARK: - NativeArrayData
extension {type_name}.FFI: NativeArrayData {{
{spacer:l1$}public typealias FFIArrayType = {array_type_name}
}}

extension {type_name}: NativeArrayData {{
{spacer:l1$}public typealias FFIArrayType = {array_type_name}
}}
"#,
            spacer = " ",
            l1 = TAB_SIZE,
            type_name = self.type_name(),
            array_type_name = self.array_name(),
        )
    }

    fn option_impl(&self) -> String {
        format!(
            r#"
// MARK: - Optional
public extension Optional where Wrapped == {type_name}.FFI {{
{spacer:l1$}func clone() -> OpaquePointer? {{
{spacer:l2$}switch self {{
{spacer:l2$}case let .some(value):
{spacer:l3$}return value.clone()
{spacer:l2$}case .none:
{spacer:l3$}return nil
{spacer:l2$}}}
{spacer:l1$}}}

{spacer:l1$}func borrowReference() -> OpaquePointer? {{
{spacer:l2$}switch self {{
{spacer:l2$}case let .some(value):
{spacer:l3$}return value.borrowReference()
{spacer:l2$}case .none:
{spacer:l3$}return nil
{spacer:l2$}}}
{spacer:l1$}}}

{spacer:l1$}static func fromRust(_ ptr: OpaquePointer?) -> Self {{
{spacer:l2$}guard let ptr = ptr else {{
{spacer:l3$}return .none
{spacer:l2$}}}
{spacer:l2$}return Wrapped.fromRust(ptr)
{spacer:l1$}}}
}}

public extension Optional where Wrapped == {type_name} {{
{spacer:l1$}func clone() -> OpaquePointer? {{
{spacer:l2$}switch self {{
{spacer:l2$}case let .some(value):
{spacer:l3$}return value.clone()
{spacer:l2$}case .none:
{spacer:l3$}return nil
{spacer:l2$}}}
{spacer:l1$}}}

{spacer:l1$}func borrowReference() -> OpaquePointer? {{
{spacer:l2$}switch self {{
{spacer:l2$}case let .some(value):
{spacer:l3$}return value.borrowReference()
{spacer:l2$}case .none:
{spacer:l3$}return nil
{spacer:l2$}}}
{spacer:l1$}}}

{spacer:l1$}static func fromRust(_ ptr: OpaquePointer?) -> Self {{
{spacer:l2$}guard let ptr = ptr else {{
{spacer:l3$}return .none
{spacer:l2$}}}
{spacer:l2$}return Wrapped.fromRust(ptr)
{spacer:l1$}}}
}}
"#,
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            l3 = TAB_SIZE * 3,
            type_name = self.type_name(),
        )
    }

    fn required_imports(&self) -> &[syn::Path] {
        self.enum_ffi.required_imports
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    mod utilities {
        use super::*;
        use crate::{
            parsing::FieldAttributes,
            quote::format_ident,
            struct_internals::{
                enum_ffi::VariantFFI,
                field_ffi::{FieldFFI, FieldIdent, FieldSource},
            },
            type_ffi::{TypeFFI, TypeIdentifier},
        };

        pub(super) fn test_mod_name() -> Ident {
            format_ident!("test_module")
        }

        pub(super) fn type_name() -> Ident {
            format_ident!("TestType")
        }

        pub(super) fn variant_1() -> Ident {
            format_ident!("variant1")
        }

        pub(super) fn variant_2() -> Ident {
            format_ident!("variant2")
        }

        pub(super) fn foo<'a>(
            test_mod_name: &'a Ident,
            type_name: &'a Ident,
            variant_1: &'a Ident,
            variant_2: &'a Ident,
        ) -> EnumFFI<'a> {
            EnumFFI {
                module_name: test_mod_name,
                type_name,
                variants: vec![
                    VariantFFI {
                        ident: variant_1,
                        fields: vec![FieldFFI {
                            type_name,
                            field_name: FieldIdent::UnnamedField(0),
                            field_source: FieldSource::Enum {
                                variant_ident: variant_1,
                                variant_fields_len: 1,
                                other_variants: vec![variant_2.clone()],
                            },
                            native_type_data: TypeFFI {
                                native_type: TypeIdentifier::Raw(format_ident!("u16")),
                                is_option: false,
                                is_vec: false,
                                is_result: false,
                                is_cow: false,
                                is_borrow: false,
                            },
                            attributes: FieldAttributes {
                                expose_as: None,
                                raw: false,
                            },
                        }],
                    },
                    VariantFFI {
                        ident: variant_2,
                        fields: vec![FieldFFI {
                            type_name,
                            field_name: FieldIdent::UnnamedField(0),
                            field_source: FieldSource::Enum {
                                variant_ident: variant_2,
                                variant_fields_len: 1,
                                other_variants: vec![variant_1.clone()],
                            },
                            native_type_data: TypeFFI {
                                native_type: TypeIdentifier::Raw(format_ident!("u8")),
                                is_option: false,
                                is_vec: false,
                                is_result: false,
                                is_cow: false,
                                is_borrow: false,
                            },
                            attributes: FieldAttributes {
                                expose_as: None,
                                raw: false,
                            },
                        }],
                    },
                ],
                alias_modules: &[],
                required_imports: &[],
            }
        }
    }

    #[test]
    fn test_type_definition() {
        let test_mod_name = utilities::test_mod_name();
        let type_name = utilities::type_name();
        let variant_1 = utilities::variant_1();
        let variant_2 = utilities::variant_2();
        let enum_ffi = utilities::foo(&test_mod_name, &type_name, &variant_1, &variant_2);
        let complex_consumer_enum = ComplexConsumerEnum {
            enum_ffi: &enum_ffi,
        };
        assert_eq!(
            complex_consumer_enum.type_definition(),
            r#"
/// Doesn't require any special memory behavior; it's a swift type containing other types (and those
/// contained types will manage their own memory as needed).
///
/// When accessing the associated values on this type, keep in mind that the first value is an aid
/// for the FFI; you can safely ignore it with `_` when destructuring.
///
public enum TestType {
    case variant1(TestType.FFI, UInt16)
    case variant2(TestType.FFI, UInt8)

    static func variant1(_ data: UInt16) -> Self {
        FFI(test_type_variant1_rust_ffi_init(data.clone())).makeNative()
    }

    static func variant2(_ data: UInt8) -> Self {
        FFI(test_type_variant2_rust_ffi_init(data.clone())).makeNative()
    }
}

// MARK: - FFI
extension TestType {

    public final class FFI {
        internal let pointer: OpaquePointer

        internal init(_ pointer: OpaquePointer) {
            self.pointer = pointer
        }

        deinit {
            rust_ffi_free_test_type(pointer)
        }
    }

}

// MARK: - ForeignEnum
extension TestType.FFI: ForeignEnum {
    public typealias NativeEnumType = TestType

    public func makeNative() -> NativeEnumType {
        switch get_test_type_variant(pointer) {

        case TestTypeType_variant1:
            return .variant1(
                self,
                get_test_type_variant1_unnamed_field_0(pointer)
            )


        case TestTypeType_variant2:
            return .variant2(
                self,
                get_test_type_variant2_unnamed_field_0(pointer)
            )

        default:
            fatalError("Unreachable")
        }
    }
}

// MARK: - NativeEnum
extension TestType: NativeEnum {
    public typealias FFIType = Self.FFI

    public var ffi: FFI {
        switch self {
        case
            let .variant1(ffi, _),
            let .variant2(ffi, _)
        :
            return ffi
        }
    }

    public static func fromRust(pointer: FFIType.ForeignType) -> Self {
        return FFI.fromRust(pointer).makeNative()
    }
}
"#
        );
    }

    #[test]
    fn native_data_impl() {
        let test_mod_name = utilities::test_mod_name();
        let type_name = utilities::type_name();
        let variant_1 = utilities::variant_1();
        let variant_2 = utilities::variant_2();
        let enum_ffi = utilities::foo(&test_mod_name, &type_name, &variant_1, &variant_2);
        let complex_consumer_enum = ComplexConsumerEnum {
            enum_ffi: &enum_ffi,
        };
        assert_eq!(
            complex_consumer_enum.native_data_impl(),
            r#"
// MARK: - NativeData
extension TestType.FFI: NativeData {
    public typealias ForeignType = OpaquePointer?

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> ForeignType {
        return rust_ffi_clone_test_type(pointer)
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> ForeignType {
        return pointer
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: ForeignType) -> Self {
        return Self(foreignObject!)
    }
}

extension TestType: NativeData {
    public typealias ForeignType = FFIType.ForeignType

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> FFIType.ForeignType {
        ffi.clone()
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> FFIType.ForeignType {
        ffi.borrowReference()
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: FFIType.ForeignType) -> Self {
        Self.FFIType.fromRust(foreignObject).makeNative()
    }
}
"#
        );
    }

    #[test]
    fn ffi_array_impl() {
        let test_mod_name = utilities::test_mod_name();
        let type_name = utilities::type_name();
        let variant_1 = utilities::variant_1();
        let variant_2 = utilities::variant_2();
        let enum_ffi = utilities::foo(&test_mod_name, &type_name, &variant_1, &variant_2);
        let complex_consumer_enum = ComplexConsumerEnum {
            enum_ffi: &enum_ffi,
        };
        assert_eq!(
            complex_consumer_enum.native_data_impl(),
            r#"
// MARK: - NativeData
extension TestType.FFI: NativeData {
    public typealias ForeignType = OpaquePointer?

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> ForeignType {
        return rust_ffi_clone_test_type(pointer)
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> ForeignType {
        return pointer
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: ForeignType) -> Self {
        return Self(foreignObject!)
    }
}

extension TestType: NativeData {
    public typealias ForeignType = FFIType.ForeignType

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> FFIType.ForeignType {
        ffi.clone()
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> FFIType.ForeignType {
        ffi.borrowReference()
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: FFIType.ForeignType) -> Self {
        Self.FFIType.fromRust(foreignObject).makeNative()
    }
}
"#
        );
    }

    #[test]
    fn native_array_data_impl() {
        let test_mod_name = utilities::test_mod_name();
        let type_name = utilities::type_name();
        let variant_1 = utilities::variant_1();
        let variant_2 = utilities::variant_2();
        let enum_ffi = utilities::foo(&test_mod_name, &type_name, &variant_1, &variant_2);
        let complex_consumer_enum = ComplexConsumerEnum {
            enum_ffi: &enum_ffi,
        };
        assert_eq!(
            complex_consumer_enum.native_data_impl(),
            r#"
// MARK: - NativeData
extension TestType.FFI: NativeData {
    public typealias ForeignType = OpaquePointer?

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> ForeignType {
        return rust_ffi_clone_test_type(pointer)
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> ForeignType {
        return pointer
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: ForeignType) -> Self {
        return Self(foreignObject!)
    }
}

extension TestType: NativeData {
    public typealias ForeignType = FFIType.ForeignType

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> FFIType.ForeignType {
        ffi.clone()
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> FFIType.ForeignType {
        ffi.borrowReference()
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: FFIType.ForeignType) -> Self {
        Self.FFIType.fromRust(foreignObject).makeNative()
    }
}
"#
        );
    }

    #[test]
    fn option_impl() {
        let test_mod_name = utilities::test_mod_name();
        let type_name = utilities::type_name();
        let variant_1 = utilities::variant_1();
        let variant_2 = utilities::variant_2();
        let enum_ffi = utilities::foo(&test_mod_name, &type_name, &variant_1, &variant_2);
        let complex_consumer_enum = ComplexConsumerEnum {
            enum_ffi: &enum_ffi,
        };
        assert_eq!(
            complex_consumer_enum.native_data_impl(),
            r#"
// MARK: - NativeData
extension TestType.FFI: NativeData {
    public typealias ForeignType = OpaquePointer?

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> ForeignType {
        return rust_ffi_clone_test_type(pointer)
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> ForeignType {
        return pointer
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: ForeignType) -> Self {
        return Self(foreignObject!)
    }
}

extension TestType: NativeData {
    public typealias ForeignType = FFIType.ForeignType

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> FFIType.ForeignType {
        ffi.clone()
    }

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> FFIType.ForeignType {
        ffi.borrowReference()
    }

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: FFIType.ForeignType) -> Self {
        Self.FFIType.fromRust(foreignObject).makeNative()
    }
}
"#
        );
    }
}
