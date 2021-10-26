use crate::{
    consumer::{consumer_struct::ConsumerStruct, TAB_SIZE},
    heck::MixedCase,
    items::struct_ffi::custom,
    syn::Ident,
    type_ffi::TypeFFI,
};

struct InitArgs {
    consumer: String,
    ffi: String,
}

// This implements some additional consumer-related behavior for the type from
// `items::struct_ffi::custom` so that we can keep all of the consumer-related code isolated to the
// `ffi_internals::consumer` module.
impl custom::StructFFI<'_> {
    fn consumer_getters(&self) -> String {
        let type_prefix = format!("get_{}_", self.type_name);
        let failable_fns: Vec<&Ident> = self
            .custom_attributes
            .failable_fns
            .iter()
            .map(|x| crate::consumer::get_segment_ident(x.segments.last()))
            .collect();
        self.getters
            .iter()
            .enumerate()
            .fold(String::new(), |mut acc, (index, (getter_ident, getter_type))| {
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
"{spacer:l1$}{access_modifier} var {consumer_getter_name}: {consumer_type} {{
{spacer:l2$}{consumer_type}.fromRust({getter_ident}(pointer))
{spacer:l1$}}}",
                    spacer = " ",
                    l1 = TAB_SIZE,
                    l2 = TAB_SIZE * 2,
                    access_modifier = access_modifier,
                    consumer_getter_name = consumer_getter_name,
                    consumer_type = consumer_type,
                    getter_ident = getter_ident.to_string()
                ));
                // Push an extra line between var declarations.
                if index < self.getters.len() - 1 { acc.push_str("\n\n") }
                acc
            })
    }

    fn initialization_args(&self) -> InitArgs {
        let arg_count = self.init_args.len();
        let (consumer, ffi) = self.init_args.iter().enumerate().fold(
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
                    indent_level = TAB_SIZE * 2,
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
                    indent_level = TAB_SIZE * 3,
                ));
                acc
            },
        );

        InitArgs { consumer, ffi }
    }
}

impl From<&custom::StructFFI<'_>> for ConsumerStruct {
    /// Returns a `ConsumerStruct` for a type that defines its own custom FFI.
    ///
    fn from(inputs: &custom::StructFFI<'_>) -> Self {
        let init_args = inputs.initialization_args();
        let consumer_getters = inputs.consumer_getters();

        Self {
            type_name: inputs.type_name.to_string(),
            consumer_imports: inputs.consumer_imports.to_owned(),
            consumer_init_args: init_args.consumer,
            ffi_init_args: init_args.ffi,
            consumer_getters,
            init_fn_name: inputs.init_fn_name.to_string(),
            free_fn_name: inputs.free_fn_name.to_string(),
            clone_fn_name: inputs.clone_fn_name.to_string(),
            failable_init: inputs.custom_attributes.failable_init,
            forbid_memberwise_init: inputs.forbid_memberwise_init,
            docs: crate::consumer::consumer_docs_from(inputs.doc_comments, 0),
        }
    }
}
