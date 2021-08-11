use crate::{
    impl_internals::fn_ffi::{FnReceiver, FnFFI},
    heck::MixedCase,
    syn::Ident
};

/// Generates a consumer function for calling the foreign function produced by
/// `self.generate_ffi(...)`.
///
pub(super) fn generate_consumer(fn_ffi: &FnFFI, module_name: &Ident) -> String {
    // Include the keyword `static` if this function doesn't take a receiver.
    let static_keyword = if fn_ffi.receiver == FnReceiver::None { "static" } else { "" };
    let (return_conversion, close_conversion, return_sig) =
        if let Some(return_type) = &fn_ffi.return_type {
            let ty = return_type.consumer_type(None);
            (
                if return_type.is_result {
                    "handle(result: ".to_string() 
                } else {
                    format!("{}.fromRust(", ty) 
                },
                format!(")"),
                if return_type.is_result {
                    format!("-> Result<{}, RustError>", ty)
                } else {
                    format!("-> {}", ty)
                },
            )
        } else {
            (String::new(), String::new(), String::new())
        };
    format!(
        r#"
{static_keyword} func {consumer_fn_name}({consumer_parameters}) {return_sig} {{
    {return_conversion}{ffi_fn_name}({ffi_parameters}){close_conversion}
}}
        "#,
        static_keyword = static_keyword,
        consumer_fn_name = fn_ffi.fn_name.to_string().to_mixed_case(),
        consumer_parameters = consumer_parameters(fn_ffi),
        return_sig = return_sig,
        return_conversion = return_conversion,
        ffi_fn_name = fn_ffi.ffi_fn_name(module_name).to_string(),
        ffi_parameters = ffi_calling_arguments(fn_ffi),
        close_conversion = close_conversion,
    )
}

pub fn generate_consumer_extension(fn_ffi: &FnFFI, header: &str, consumer_type: &str, module_name: &Ident, imports: Option<&str>) -> String {
    // Include the keyword `static` if this function doesn't take a receiver.
    let static_keyword = if fn_ffi.receiver == FnReceiver::None { "static" } else { "" };
    let (return_conversion, close_conversion, return_sig) =
        if let Some(return_type) = &fn_ffi.return_type {
            let ty = return_type.consumer_type(None);
            (
                if return_type.is_result {
                        "handle(result: ".to_string() 
                } else {
                    format!("{}.fromRust(", ty) 
                },
                format!(")"),
                if return_type.is_result {
                    format!("-> Result<{}, RustError>", ty)
                } else {
                    format!("-> {}", ty)
                },
            )
        } else {
            (String::new(), String::new(), String::new())
        };
    format!(
        r#"
{header}
{imports}

extension {consumer_type} {{
{static_keyword} func {consumer_fn_name}({consumer_parameters}) {return_sig} {{
    {return_conversion}{ffi_fn_name}({ffi_parameters}){close_conversion}
}}
}}
        "#,
        static_keyword = static_keyword,
        header = header,
        consumer_type = consumer_type,
        imports = imports.unwrap_or_default(),
        consumer_fn_name = fn_ffi.fn_name.to_string().to_mixed_case(),
        consumer_parameters = consumer_parameters(fn_ffi),
        return_sig = return_sig,
        return_conversion = return_conversion,
        ffi_fn_name = fn_ffi.ffi_fn_name(module_name).to_string(),
        ffi_parameters = ffi_calling_arguments(fn_ffi),
        close_conversion = close_conversion,
    )
}

fn consumer_parameters(fn_ffi: &FnFFI) -> String {
    fn_ffi.parameters
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

fn ffi_calling_arguments(fn_ffi: &FnFFI) -> String {
    let mut parameters: Vec<String> = fn_ffi
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
    if fn_ffi.receiver != FnReceiver::None {
        let receiver_arg = "pointer".to_string();
        parameters.insert(0, receiver_arg);
    }
    parameters.join(", ")
}
