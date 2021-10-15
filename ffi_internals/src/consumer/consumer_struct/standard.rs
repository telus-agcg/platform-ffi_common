use crate::{
    consumer::{consumer_struct::ConsumerStruct, TAB_SIZE},
    struct_internals::struct_ffi::StructFFI,
};

#[derive(Debug, Clone, PartialEq)]
struct ExpandedFields {
    consumer_init_args: String,
    ffi_init_args: String,
    consumer_getters: String,
}

impl StructFFI<'_> {
    /// Expands this struct's fields to their corresponding consumer initializer arguments, FFI
    /// initializer arguments, and consumer getters.
    ///
    fn expand_fields(&self) -> ExpandedFields {
        let (consumer_init_args, ffi_init_args, consumer_getters) =
            self.fields.iter().enumerate().fold(
                (String::new(), String::new(), String::new()),
                |mut acc, (index, f)| {
                    // Swift rejects trailing commas on argument lists.
                    let trailing_punctuation = if index < self.fields.len() - 1 {
                        ",\n"
                    } else {
                        ""
                    };
                    // This looks like `foo: Bar,`.
                    acc.0.push_str(&format!(
                        "{spacer:level$}{field}: {type_name}{punct}",
                        spacer = " ",
                        level = TAB_SIZE * 2,
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
                        level = TAB_SIZE * 3,
                    ));
                    // This looks like `public var foo: Bar { Bar.fromRust(get_bar_foo(pointer) }`.
                    acc.2.push_str(&format!(
                        "
{spacer:l1$}public var {field}: {type_name} {{
{spacer:l2$}{type_name}.fromRust({getter}(pointer))
{spacer:l1$}}}
",
                        spacer = " ",
                        l1 = TAB_SIZE,
                        l2 = TAB_SIZE * 2,
                        field = f.field_name.consumer_ident(),
                        type_name = f
                            .native_type_data
                            .consumer_type(f.attributes.expose_as_ident()),
                        getter = f.getter_name().to_string()
                    ));
                    acc
                },
            );

        ExpandedFields {
            consumer_init_args,
            ffi_init_args,
            consumer_getters,
        }
    }
}

impl From<&StructFFI<'_>> for ConsumerStruct {
    fn from(struct_ffi: &StructFFI<'_>) -> Self {
        let expanded_fields = struct_ffi.expand_fields();
        Self {
            type_name: struct_ffi.name.to_string(),
            required_imports: struct_ffi.required_imports.clone(),
            consumer_init_args: expanded_fields.consumer_init_args,
            ffi_init_args: expanded_fields.ffi_init_args,
            consumer_getters: expanded_fields.consumer_getters,
            init_fn_name: struct_ffi.init_fn_name().to_string(),
            free_fn_name: struct_ffi.free_fn_name().to_string(),
            clone_fn_name: struct_ffi.clone_fn_name().to_string(),
            failable_init: false,
            forbid_memberwise_init: struct_ffi.forbid_memberwise_init,
        }
    }
}
