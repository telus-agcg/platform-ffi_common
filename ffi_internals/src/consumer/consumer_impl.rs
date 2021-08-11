use crate::impl_internals::impl_ffi::ImplFFI;
use heck::CamelCase;

pub fn consumer_file_name(impl_ffi: &ImplFFI) -> String {
    format!("{}_{}.swift", impl_ffi.trait_name, impl_ffi.type_name)
}

/// Generates an implementation for the consumer's type so that they'll be able to call it like
/// `nativeTypeInstance.someMethod(with: params)`. Hardcoded to Swift for now like all the other
/// consumer output, until we bother templating for other languages.
///
/// Example output:
/// ```ignore
/// extension SelectedField {
///     func build_commodity_locations(plantings: [CLPlanting]) -> [CommodityLocation] {
///         [CommodityLocation].fromRust(build_commodity_locations(pointer, plantings.clone()))
///     }
/// }
/// ```
///
pub fn generate_consumer(impl_ffi: &ImplFFI, header: &str) -> String {
    let additional_imports: Vec<String> = impl_ffi
        .consumer_imports
        .iter()
        .map(|path| {
            let crate_name = path
                .segments
                .first()
                .unwrap()
                .ident
                .to_string()
                .to_camel_case();
            let type_name = path
                .segments
                .last()
                .unwrap()
                .ident
                .to_string()
                .to_camel_case();
            format!("import class {}.{}", crate_name, type_name)
        })
        .collect();
    format!(
        r#"
{header}
{common_framework}
{additional_imports}

public extension {native_type} {{
{functions}
}}
        "#,
        header = header,
        common_framework = option_env!("FFI_COMMON_FRAMEWORK")
            .map(|f| format!("import {}", f))
            .unwrap_or_default(),
        additional_imports = additional_imports.join("\n"),
        native_type = impl_ffi.type_name.to_string(),
        functions = impl_ffi
            .fns
            .iter()
            .map(|f| super::consumer_fn::generate_consumer(f, &impl_ffi.module_name()))
            .collect::<Vec<String>>()
            .join("\n"),
    )
}
