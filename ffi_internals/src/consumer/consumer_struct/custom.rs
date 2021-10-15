use crate::{
    consumer::{consumer_struct::ConsumerStruct, TAB_SIZE},
    heck::MixedCase,
    parsing::CustomAttributes,
    syn::{Ident, Path, Type},
    type_ffi::TypeFFI,
};

/// Representes the inputs for building a customm consumer struct.
///
pub struct ConsumerStructInputs<'a> {
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
    /// If true, do not generate a memberwise initializer for this type. Some types only allow
    /// construction via specific APIs that implemenat additional checks; in those cases, a
    /// generated memberwise init bypasses those restrictions.
    ///
    pub forbid_memberwise_init: bool,
}

struct InitArgs {
    consumer: String,
    ffi: String,
}

impl ConsumerStructInputs<'_> {
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
                    l1 = TAB_SIZE,
                    l2 = TAB_SIZE * 2,
                    access_modifier = access_modifier,
                    consumer_getter_name = consumer_getter_name,
                    consumer_type = consumer_type,
                    getter_ident = getter_ident.to_string()
                ));
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

impl From<ConsumerStructInputs<'_>> for ConsumerStruct {
    /// Returns a `ConsumerStruct` for a type that defines its own custom FFI.
    ///
    fn from(inputs: ConsumerStructInputs<'_>) -> Self {
        let init_args = inputs.initialization_args();
        let consumer_getters = inputs.consumer_getters();

        Self {
            type_name: inputs.type_name,
            required_imports: inputs.required_imports.to_owned(),
            consumer_init_args: init_args.consumer,
            ffi_init_args: init_args.ffi,
            consumer_getters,
            init_fn_name: inputs.init_fn_name,
            free_fn_name: inputs.free_fn_name,
            clone_fn_name: inputs.clone_fn_name,
            failable_init: inputs.custom_attributes.failable_init,
            forbid_memberwise_init: inputs.forbid_memberwise_init,
        }
    }
}
