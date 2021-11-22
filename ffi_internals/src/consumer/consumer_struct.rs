//!
//! Generates a wrapping type in the consumer's language, including a native initializer, a
//! deinitializer implementation that calls the appropriate `free_*` method for the Rust struct, and
//! native getters for reading properties from the Rust struct.
//!

use crate::{
    consumer::{ConsumerType, TAB_SIZE},
    syn::Path,
};

mod custom;
mod standard;

/// Contains the data required to generate a consumer type, and associated functions for doing so.
///
pub struct ConsumerStruct {
    /// The name of the type name.
    ///
    pub type_name: String,
    /// Additional imports that this type requires.
    ///
    pub consumer_imports: Vec<Path>,
    /// The arguments for the consumer type's initializer.
    ///
    consumer_init_args: String,
    /// The arguments the consumer needs to pass to the FFI initializer.
    ///
    ffi_init_args: String,
    /// The consumer getters (readonly variables that wrap calls to Rust functions for reading
    /// struct field values).
    ///
    consumer_getters: String,
    /// The name of the Rust type's initializer function.
    ///
    pub init_fn_name: String,
    /// The name of the Rust type's free function.
    ///
    pub free_fn_name: String,
    /// The name of the Rust type's clone function.
    ///
    pub clone_fn_name: String,
    /// True if the Rust initializer is failable. This is only relevant for types exposed through a
    /// custom (i.e., non-derived) FFI implementation.
    ///
    failable_init: bool,
    /// If true, do not generate a memberwise initializer for this type. Some types only allow
    /// construction via specific APIs that implemenat additional checks; in those cases, a
    /// generated memberwise init bypasses those restrictions.
    ///
    forbid_memberwise_init: bool,
    /// Documentation comments on this struct.
    ///
    docs: String,
}

impl ConsumerStruct {
    fn array_name(&self) -> String {
        format!("FFIArray{}", self.type_name)
    }

    fn array_init(&self) -> String {
        format!("ffi_array_{}_init", self.type_name)
    }

    fn array_free(&self) -> String {
        format!("ffi_array_{}_free", self.type_name)
    }

    fn init_impl(&self) -> Option<String> {
        if self.forbid_memberwise_init {
            return None;
        }
        if self.failable_init {
            Some(format!(
                "{spacer:l1$}internal init?(
{args}
{spacer:l1$}) {{
{spacer:l2$}guard let pointer = {ffi_init}(
{ffi_args}
{spacer:l2$}) else {{
{spacer:l3$}return nil
{spacer:l2$}}}
{spacer:l2$}self.pointer = pointer
{spacer:l1$}}}",
                spacer = " ",
                l1 = TAB_SIZE,
                l2 = TAB_SIZE * 2,
                l3 = TAB_SIZE * 3,
                args = self.consumer_init_args,
                ffi_init = self.init_fn_name,
                ffi_args = self.ffi_init_args,
            ))
        } else {
            Some(format!(
                "{spacer:l1$}public init(
{args}
{spacer:l1$}) {{
{spacer:l2$}self.pointer = {ffi_init}(
{ffi_args}
{spacer:l2$})
{spacer:l1$}}}",
                spacer = " ",
                l1 = TAB_SIZE,
                l2 = TAB_SIZE * 2,
                args = self.consumer_init_args,
                ffi_init = self.init_fn_name,
                ffi_args = self.ffi_init_args,
            ))
        }
    }
}

impl ConsumerType for ConsumerStruct {
    fn type_name(&self) -> String {
        self.type_name.clone()
    }

    /// Generates a wrapper for a struct so that the native interface in the consumer's language
    /// correctly wraps the generated FFI module.
    ///
    fn type_definition(&self) -> Option<String> {
        let mut result = self.docs.clone();
        result.push_str(&format!(
            "public final class {class} {{

{spacer:l1$}internal let pointer: OpaquePointer",
            spacer = " ",
            l1 = TAB_SIZE,
            class = self.type_name,
        ));
        // Newline after the internal property declaration, and an empty line after that.
        result.push_str("\n\n");

        // If we have an init_impl, push it and another pair of newlines.
        if let Some(init_impl) = self.init_impl() {
            result.push_str(&init_impl);
            result.push_str("\n\n");
        }

        // Push the internal init, deinit, and getters.
        result.push_str(&format!(
            "{spacer:l1$}internal init(_ pointer: OpaquePointer) {{
{spacer:l2$}self.pointer = pointer
{spacer:l1$}}}

{spacer:l1$}deinit {{
{spacer:l2$}{free_fn_name}(pointer)
{spacer:l1$}}}

{getters}
}}",
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            free_fn_name = self.free_fn_name,
            getters = self.consumer_getters
        ));
        Some(result)
    }

    fn native_data_impl(&self) -> String {
        format!(
"// MARK: - NativeData
extension {type_name}: NativeData {{
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
}}",
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            type_name = self.type_name,
            clone_fn_name = self.clone_fn_name,
        )
    }

    fn ffi_array_impl(&self) -> String {
        format!(
            "// MARK: - FFIArray
extension {array_name}: FFIArray {{
{spacer:l1$}public typealias Value = OpaquePointer?

{spacer:l1$}public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
{spacer:l2$}{array_init}(ptr, len)
{spacer:l1$}}}

{spacer:l1$}public static func free(_ array: Self) {{
{spacer:l2$}{array_free}(array)
{spacer:l1$}}}
}}",
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            array_name = self.array_name(),
            array_init = self.array_init(),
            array_free = self.array_free(),
        )
    }

    fn native_array_data_impl(&self) -> String {
        format!(
            "// MARK: - NativeArrayData
extension {type_name}: NativeArrayData {{
{spacer:l1$}public typealias FFIArrayType = {array_name}
}}",
            spacer = " ",
            l1 = TAB_SIZE,
            type_name = self.type_name,
            array_name = self.array_name()
        )
    }

    fn option_impl(&self) -> String {
        format!(
            "// MARK: - Optional
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
}}",
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            l3 = TAB_SIZE * 3,
            type_name = self.type_name,
        )
    }

    fn consumer_imports(&self) -> &[Path] {
        &*self.consumer_imports
    }
}
