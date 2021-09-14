//!
//! Generates a wrapping type in the consumer's language, including a native initializer, a
//! deinitializer implementation that calls the appropriate `free_*` method for the Rust struct, and
//! native getters for reading properties from the Rust struct.
//!

use crate::{
    consumer::ConsumerType,
    heck::MixedCase,
    parsing::CustomAttributes,
    struct_internals::{field_ffi::FieldFFI, struct_ffi::StructFFI},
    syn::{Ident, Path, Type},
    type_ffi::TypeFFI,
};

/// Contains the data required to generate a consumer type, and associated functions for doing so.
///
pub struct ConsumerStruct {
    /// The name of the type name.
    ///
    pub type_name: String,
    /// Additional imports that this type requires.
    ///
    required_imports: Vec<Path>,
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

    fn init_impl(&self) -> String {
        if self.failable_init {
            format!(
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
                l1 = super::TAB_SIZE,
                l2 = super::TAB_SIZE * 2,
                l3 = super::TAB_SIZE * 3,
                args = self.consumer_init_args,
                ffi_init = self.init_fn_name,
                ffi_args = self.ffi_init_args,
            )
        } else {
            format!(
                "{spacer:l1$}public init(
{args}
{spacer:l1$}) {{
{spacer:l2$}self.pointer = {ffi_init}(
{ffi_args}
{spacer:l2$})
{spacer:l1$}}}",
                spacer = " ",
                l1 = super::TAB_SIZE,
                l2 = super::TAB_SIZE * 2,
                args = self.consumer_init_args,
                ffi_init = self.init_fn_name,
                ffi_args = self.ffi_init_args,
            )
        }
    }
}

impl ConsumerType for ConsumerStruct {
    /// Generates a wrapper for a struct so that the native interface in the consumer's language
    /// correctly wraps the generated FFI module.
    ///
    fn type_definition(&self) -> String {
        format!(
            "
public final class {class} {{

    internal let pointer: OpaquePointer

{init_impl}

    internal init(_ pointer: OpaquePointer) {{
        self.pointer = pointer
    }}

    deinit {{
        {free_fn_name}(pointer)
    }}
{getters}
}}
",
            class = self.type_name,
            init_impl = self.init_impl(),
            free_fn_name = self.free_fn_name,
            getters = self.consumer_getters
        )
    }

    fn ffi_array_impl(&self) -> String {
        format!(
            "
extension {array_name}: FFIArray {{
    public typealias Value = OpaquePointer?

    public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
        {array_init}(ptr, len)
    }}

    public static func free(_ array: Self) {{
        {array_free}(array)
    }}
}}
",
            array_name = self.array_name(),
            array_init = self.array_init(),
            array_free = self.array_free(),
        )
    }

    fn native_data_impl(&self) -> String {
        format!(
            "
extension {}: NativeData {{
    public typealias ForeignType = OpaquePointer?

    /// `clone()` will clone this instance (in Rust) and return a pointer to it that can be 
    /// used when calling a Rust function that takes ownership of an instance (like an initializer
    /// with a parameter of this type).
    public func clone() -> ForeignType {{
        return {}(pointer)
    }}

    /// `borrowReference()` will pass this instance's `pointer` to Rust as a reference. This
    /// must only be used when calling Rust functions that take a borrowed reference; otherwise,
    /// Rust will free `pointer` while this instance retains it.
    public func borrowReference() -> ForeignType {{
        return pointer
    }}

    /// Initializes an instance of this type from a pointer to an instance of the Rust type.
    public static func fromRust(_ foreignObject: ForeignType) -> Self {{
        return Self(foreignObject!)
    }}
}}
",
            self.type_name, self.clone_fn_name,
        )
    }

    fn option_impl(&self) -> String {
        format!(
            "
public extension Optional where Wrapped == {} {{
    func clone() -> OpaquePointer? {{
        switch self {{
        case let .some(value):
            return value.clone()
        case .none:
            return nil
        }}
    }}

    func borrowReference() -> OpaquePointer? {{
        switch self {{
        case let .some(value):
            return value.borrowReference()
        case .none:
            return nil
        }}
    }}

    static func fromRust(_ ptr: OpaquePointer?) -> Self {{
        guard let ptr = ptr else {{
            return .none
        }}
        return Wrapped.fromRust(ptr)
    }}
}}
",
            self.type_name
        )
    }

    fn native_array_data_impl(&self) -> String {
        format!(
            "
extension {}: NativeArrayData {{
    public typealias FFIArrayType = {}
}}
",
            self.type_name,
            self.array_name()
        )
    }

    fn required_imports(&self) -> &[Path] {
        &*self.required_imports
    }
}

/// Representes the inputs for building a customm consumer struct.
///
pub struct CustomConsumerStructInputs<'a> {
    /// The name of the struct we're working with.
    ///
    pub type_name: String,
    /// Any required imports that the consumer will need.
    ///
    pub required_imports: &'a [Path],
    /// Custom attributes set on the struct.
    ///
    pub custom_attributes: &'a CustomAttributes,
    /// The name of the initializer function in this struct.
    ///
    pub init_fn_name: String,
    /// The arguments that this struct's initializer function takes (each represented by a tuple of
    /// their identifier and type).
    ///
    pub init_args: &'a [(Ident, Type)],
    /// The getter functions provided by this struct (each represented by a tuple of their
    /// identifier and type).
    ///
    pub getters: &'a [(Ident, Type)],
    /// The name of the free function in this struct.
    ///
    pub free_fn_name: String,
    /// The name of the clone function in this struct.
    ///
    pub clone_fn_name: String,
}

impl<'a> From<CustomConsumerStructInputs<'a>> for ConsumerStruct {
    /// Returns a `ConsumerStruct` for a type that defines its own custom FFI.
    ///
    fn from(inputs: CustomConsumerStructInputs<'_>) -> Self {
        let arg_count = inputs.init_args.len();
        let (consumer_init_args, ffi_init_args) = inputs.init_args.iter().enumerate().fold(
            (String::new(), String::new()),
            |mut acc, (index, (arg_ident, arg_type))| {
                // Swift rejects trailing commas on argument lists.
                let trailing_punctuation = if index < arg_count - 1 { ",\n" } else { "" };
                let arg_ident_string = arg_ident.to_string();
                let (required, arg_ident_string) = arg_ident_string
                    .strip_prefix("required_")
                    .map_or((false, &*arg_ident_string), |stripped| (true, stripped));
                let consumer_type = TypeFFI::from((arg_type, required)).consumer_type(None);
                // This looks like `foo: Bar,`.
                acc.0.push_str(&format!(
                    "{:indent_level$}{}: {}{}",
                    " ",
                    arg_ident_string,
                    consumer_type,
                    trailing_punctuation,
                    indent_level = super::TAB_SIZE * 2,
                ));
                // It's worth noting here that we always clone when calling an initializer -- the
                // new Rust instance needs to take ownership of the data because it will be owned by
                // a new Swift instance whose lifetime is unrelated to the lifetime of the
                // parameters passed to it.
                // This looks like `foo.clone(),`.
                acc.1.push_str(&format!(
                    "{:indent_level$}{}.clone(){}",
                    " ",
                    arg_ident_string,
                    trailing_punctuation,
                    indent_level = super::TAB_SIZE * 3,
                ));
                acc
            },
        );

        let type_prefix = format!("get_{}_", inputs.type_name);
        let failable_fns: Vec<&Ident> = inputs
            .custom_attributes
            .failable_fns
            .iter()
            .map(|x| super::get_segment_ident(x.segments.last()))
            .collect();
        let consumer_getters =
            inputs
                .getters
                .iter()
                .fold(String::new(), |mut acc, (getter_ident, getter_type)| {
                    // We're going to give things an internal access modifier if they're failable on the
                    // Rust side. This will require some additional (handwritten) Swift code for error
                    // handling before they can be accessed outside of the framework that contains the
                    // generated code.
                    let access_modifier = if failable_fns.contains(&getter_ident) {
                        "internal"
                    } else {
                        "public"
                    };
                    let consumer_type = TypeFFI::from((getter_type, false)).consumer_type(None);

                    let consumer_getter_name = match getter_ident
                        .to_string()
                        .split(&type_prefix)
                        .last()
                        .map(MixedCase::to_mixed_case)
                    {
                        Some(s) => s,
                        None => proc_macro_error::abort!(getter_ident.span(), "Bad string segment"),
                    };

                    acc.push_str(&format!(
                        "
{spacer:l1$}{access_modifier} var {consumer_getter_name}: {consumer_type} {{
{spacer:l2$}{consumer_type}.fromRust({getter_ident}(pointer))
{spacer:l1$}}}
",
                        spacer = " ",
                        l1 = super::TAB_SIZE,
                        l2 = super::TAB_SIZE * 2,
                        access_modifier = access_modifier,
                        consumer_getter_name = consumer_getter_name,
                        consumer_type = consumer_type,
                        getter_ident = getter_ident.to_string()
                    ));
                    acc
                });

        Self {
            type_name: inputs.type_name,
            required_imports: inputs.required_imports.to_owned(),
            consumer_init_args,
            ffi_init_args,
            consumer_getters,
            init_fn_name: inputs.init_fn_name,
            free_fn_name: inputs.free_fn_name,
            clone_fn_name: inputs.clone_fn_name,
            failable_init: inputs.custom_attributes.failable_init,
        }
    }
}

impl<'a> From<&StructFFI<'_>> for ConsumerStruct {
    fn from(struct_ffi: &StructFFI<'_>) -> Self {
        let (consumer_init_args, ffi_init_args, consumer_getters) =
            expand_fields(&*struct_ffi.fields);
        Self {
            type_name: struct_ffi.name.to_string(),
            required_imports: struct_ffi.required_imports.clone(),
            consumer_init_args,
            ffi_init_args,
            consumer_getters,
            init_fn_name: struct_ffi.init_fn_name().to_string(),
            free_fn_name: struct_ffi.free_fn_name().to_string(),
            clone_fn_name: struct_ffi.clone_fn_name().to_string(),
            failable_init: false,
        }
    }
}

/// Expands a `&[FieldFFI]` to a tuple of consumer initializer arguments, FFI initializer
/// arguments, and consumer getters for accessing the Rust fields.
///
fn expand_fields(fields_ffi: &[FieldFFI<'_>]) -> (String, String, String) {
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
                "{spacer:level$}{field}: {type_name}{punct}",
                spacer = " ",
                level = super::TAB_SIZE * 2,
                field = f.field_name.consumer_ident(),
                type_name = f
                    .native_type_data
                    .consumer_type(f.attributes.expose_as_ident()),
                punct = trailing_punctuation
            ));
            let clone_or_borrow = if f.native_type_data.is_borrow {
                "borrowReference"
            } else {
                "clone"
            };
            // This looks like `foo.clone(),` or `foo.borrowReference(),`.
            acc.1.push_str(&format!(
                "{:level$}{}.{}(){}",
                " ",
                f.field_name.consumer_ident(),
                clone_or_borrow,
                trailing_punctuation,
                level = super::TAB_SIZE * 3,
            ));
            // This looks like `public var foo: Bar { Bar.fromRust(get_bar_foo(pointer) }`.
            acc.2.push_str(&format!(
                "
{spacer:l1$}public var {field}: {type_name} {{
{spacer:l2$}{type_name}.fromRust({getter}(pointer))
{spacer:l1$}}}
",
                spacer = " ",
                l1 = super::TAB_SIZE,
                l2 = super::TAB_SIZE * 2,
                field = f.field_name.consumer_ident(),
                type_name = f
                    .native_type_data
                    .consumer_type(f.attributes.expose_as_ident()),
                getter = f.getter_name().to_string()
            ));
            acc
        },
    )
}
