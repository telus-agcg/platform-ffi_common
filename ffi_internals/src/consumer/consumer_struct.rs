//!
//! Generates a wrapping type in the consumer's language, including a native initializer, a
//! deinitializer implementation that calls the appropriate `free_*` method for the Rust struct, and
//! native getters for reading properties from the Rust struct.
//!

use crate::{
    heck::MixedCase,
    native_type_data::NativeTypeData,
    parsing::CustomAttributes,
    struct_internals::{field_ffi::FieldFFI, struct_ffi::StructFFI},
    syn::{Ident, Path, Type},
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

    /// Generates a wrapper for a struct so that the native interface in the consumer's language
    /// correctly wraps the generated FFI module.
    ///
    fn type_definition(&self) -> String {
        let additional_imports = super::build_imports(&*self.required_imports).join("\n");
        format!(
            r#"
{common_framework}
{additional_imports}

public final class {class} {{
    internal let pointer: OpaquePointer

    {init_impl}

    private init(_ pointer: OpaquePointer) {{
        self.pointer = pointer
    }}

    deinit {{
        {free}(pointer)
    }}
{getters}
}}
"#,
            common_framework = option_env!("FFI_COMMON_FRAMEWORK")
                .map(|f| format!("import {}", f))
                .unwrap_or_default(),
            additional_imports = additional_imports,
            class = self.type_name,
            init_impl = self.init_impl(),
            free = self.free_fn_name,
            getters = self.consumer_getters
        )
    }

    fn ffi_array_impl(&self) -> String {
        format!(
            r#"
extension {array_name}: FFIArray {{
    public typealias Value = OpaquePointer?

    public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {{
        {array_init}(ptr, len)
    }}

    public static func free(_ array: Self) {{
        {array_free}(array)
    }}
}}
"#,
            array_name = self.array_name(),
            array_init = self.array_init(),
            array_free = self.array_free(),
        )
    }

    fn native_data_impl(&self) -> String {
        format!(
            r#"
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
"#,
            self.type_name, self.clone_fn_name,
        )
    }

    fn option_impl(&self) -> String {
        format!(
            r#"
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
"#,
            self.type_name
        )
    }

    fn array_impl(&self) -> String {
        format!(
            r#"
extension {}: NativeArrayData {{
    public typealias FFIArrayType = {}
}}
"#,
            self.type_name,
            self.array_name()
        )
    }

    fn init_impl(&self) -> String {
        if self.failable_init {
            format!(
                r#"
    internal init?(
        {args}
    ) {{
        guard let pointer = {ffi_init}(
            {ffi_args}
        ) else {{
            return nil
        }}
        self.pointer = pointer
    }}
                "#,
                args = self.consumer_init_args,
                ffi_init = self.init_fn_name,
                ffi_args = self.ffi_init_args,
            )
        } else {
            format!(
                r#"
    public init(
        {args}
    ) {{
        self.pointer = {ffi_init}(
            {ffi_args}
        )
    }}
                "#,
                args = self.consumer_init_args,
                ffi_init = self.init_fn_name,
                ffi_args = self.ffi_init_args,
            )
        }
    }
}

pub struct CustomConsumerStructInputs<'a> {
    pub type_name: String,
    pub required_imports: &'a [Path],
    pub custom_attributes: &'a CustomAttributes,
    pub init_fn_name: String,
    pub init_args: &'a [(Ident, Type)],
    pub getters: &'a [(Ident, Type)],
    pub free_fn_name: String,
    pub clone_fn_name: String,
}

impl<'a> From<CustomConsumerStructInputs<'a>> for ConsumerStruct {
    /// Returns a `ConsumerStruct` for a type that defines its own custom FFI.
    ///
    fn from(inputs: CustomConsumerStructInputs<'_>) -> Self {
        let arg_count = inputs.init_args.len();
        let (consumer_init_args, ffi_init_args) = inputs.init_args.iter().enumerate().fold(
            (String::new(), String::new()),
            |mut acc, (index, (i, t))| {
                // Swift rejects trailing commas on argument lists.
                let trailing_punctuation = if index < arg_count - 1 { ",\n" } else { "" };
                // This looks like `foo: Bar,`.
                // TODO: This is where we get the expression: String? for the Unit init. Wrong.
                let consumer_type = NativeTypeData::from(t).consumer_type(None);
                acc.0.push_str(&format!(
                    "        {}: {}{}",
                    i.to_string(),
                    consumer_type,
                    trailing_punctuation
                ));
                // It's worth noting here that we always clone when calling an initializer -- the
                // new Rust instance needs to take ownership of the data because it will be owned by
                // a new Swift instance whose lifetime is unrelated to the lifetime of the
                // parameters passed to it.
                // This looks like `foo.clone(),`.
                acc.1.push_str(&format!(
                    "            {}.clone(){}",
                    i.to_string(),
                    trailing_punctuation
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
                    let consumer_type = NativeTypeData::from(getter_type).consumer_type(None);

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
    {} var {}: {} {{
        {}.fromRust({}(pointer))
    }}
    ",
                        access_modifier,
                        consumer_getter_name,
                        consumer_type,
                        consumer_type,
                        getter_ident.to_string()
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

impl From<&StructFFI> for ConsumerStruct {
    fn from(struct_ffi: &StructFFI) -> Self {
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

impl From<&ConsumerStruct> for String {
    fn from(consumer: &ConsumerStruct) -> Self {
        [
            super::HEADER,
            &consumer.type_definition(),
            &consumer.ffi_array_impl(),
            &consumer.native_data_impl(),
            &consumer.option_impl(),
            &consumer.array_impl(),
        ]
        .join("")
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
                "        {field}: {type_name}{punct}",
                field = f.field_name.to_string(),
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
                "            {}.{}(){}",
                f.field_name.to_string(),
                clone_or_borrow,
                trailing_punctuation
            ));
            // This looks like `public var foo: Bar { Bar.fromRust(get_bar_foo(pointer) }`.
            acc.2.push_str(&format!(
                r#"
    public var {field}: {type_name} {{
        {type_name}.fromRust({getter}(pointer))
    }}
"#,
                field = f.field_name.to_string(),
                type_name = f
                    .native_type_data
                    .consumer_type(f.attributes.expose_as_ident()),
                getter = f.getter_name().to_string()
            ));
            acc
        },
    )
}
