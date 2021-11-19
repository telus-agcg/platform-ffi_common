//!
//! Generates a wrapping function in the consumer's language.
//!

use super::TAB_SIZE;
use crate::{
    heck::MixedCase,
    items::fn_ffi::{FnFFI, FnReceiver},
    syn::Ident,
};

impl FnFFI {
    /// Generates a consumer function for calling the foreign function produced by
    /// `self.generate_ffi(...)`.
    ///
    pub(super) fn generate_consumer(
        &self,
        module_name: &Ident,
        module_docs: Option<&[syn::Attribute]>,
    ) -> String {
        // Include the keyword `static` if this function doesn't take a receiver.
        let static_keyword = if self.receiver == FnReceiver::None {
            "static "
        } else {
            ""
        };
        let (return_conversion, close_conversion, return_sig) =
            self.return_type.as_ref().map_or_else(
                || (String::new(), String::new(), String::new()),
                crate::type_ffi::TypeFFI::consumer_return_type_components,
            );
        let mut result = module_docs.map_or(String::default(), |docs| {
            crate::consumer::consumer_docs_from(docs, 1)
        });
        result.push_str(&crate::consumer::consumer_docs_from(&*self.doc_comments, 1));
        result.push_str(&format!(
"{spacer:l1$}{static_keyword}func {consumer_fn_name}({consumer_parameters}) {return_sig} {{
{spacer:l2$}{return_conversion}{ffi_fn_name}({ffi_parameters}){close_conversion}
{spacer:l1$}}}",
            spacer = " ",
            l1 = TAB_SIZE,
            l2 = TAB_SIZE * 2,
            static_keyword = static_keyword,
            consumer_fn_name = self.fn_name.to_string().to_mixed_case(),
            consumer_parameters = self.consumer_parameters(),
            return_sig = return_sig,
            return_conversion = return_conversion,
            ffi_fn_name = self.ffi_fn_name(module_name).to_string(),
            ffi_parameters = self.ffi_calling_arguments(),
            close_conversion = close_conversion,
        ));
        result
    }

    /// Generates the contents of a consumer extension for this function, extending the original
    /// type with the behaviors described by `self`. This is primarily for use with
    /// `ffi_derive::expose_fn`, where we want to generate an FFI and consumer for a standalone
    /// function.
    ///
    #[must_use]
    pub fn generate_consumer_extension(&self, consumer_type: &str, module_name: &Ident) -> String {
        // Include the keyword `static` if this function doesn't take a receiver.
        let static_keyword = if self.receiver == FnReceiver::None {
            "static "
        } else {
            ""
        };
        let (return_conversion, close_conversion, return_sig) =
            self.return_type.as_ref().map_or_else(
                || (String::new(), String::new(), String::new()),
                crate::type_ffi::TypeFFI::consumer_return_type_components,
            );

        let mut result = format!("extension {} {{", consumer_type);
        result.push('\n');
        result.push_str(&crate::consumer::consumer_docs_from(&*self.doc_comments, 1));
        result.push_str(&format!(
"{spacer:l1$}{static_keyword}func {consumer_fn_name}({consumer_parameters}) {return_sig} {{
{spacer:l2$}{return_conversion}{ffi_fn_name}({ffi_parameters}){close_conversion}
{spacer:l1$}}}",
                        spacer = " ",
                        l1 = TAB_SIZE,
                        l2 = TAB_SIZE * 2,
                        static_keyword = static_keyword,
                        consumer_fn_name = self.fn_name.to_string().to_mixed_case(),
                        consumer_parameters = self.consumer_parameters(),
                        return_sig = return_sig,
                        return_conversion = return_conversion,
                        ffi_fn_name = self.ffi_fn_name(module_name).to_string(),
                        ffi_parameters = self.ffi_calling_arguments(),
                        close_conversion = close_conversion,
                    ));
        result.push('\n');
        result.push('}');

        [super::header_and_imports(&[]), result].join("\n")
    }

    fn consumer_parameters(&self) -> String {
        self.parameters
            .iter()
            .map(|arg| {
                format!(
                    "{}: {}",
                    arg.name.to_string(),
                    arg.native_type_data.consumer_type(None)
                )
            })
            .collect::<Vec<String>>()
            .join(", ")
    }

    fn ffi_calling_arguments(&self) -> String {
        let mut parameters: Vec<String> = self
            .parameters
            .iter()
            .map(|arg| {
                let clone_or_borrow = if arg.native_type_data.argument_borrows_supported() {
                    "borrowReference"
                } else {
                    "clone"
                };
                format!("{}.{}()", arg.name.to_string(), clone_or_borrow)
            })
            .collect();
        if self.receiver != FnReceiver::None {
            let receiver_arg = "pointer".to_string();
            parameters.insert(0, receiver_arg);
        }
        parameters.join(", ")
    }
}
