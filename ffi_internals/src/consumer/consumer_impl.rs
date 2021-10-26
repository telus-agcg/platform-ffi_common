//!
//! Generates a wrapping implementation in the consumer's language.
//!

use crate::items::impl_ffi::ImplFFI;

impl ImplFFI {
    /// Generates an appropriate consumer file name for this impl (by joining the trait and type
    /// names).
    ///
    #[must_use]
    pub fn consumer_file_name(&self) -> String {
        format!("{}_{}.swift", self.impl_description, self.type_name)
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
    /// # Errors
    ///
    /// Returns an error if `self.consumer_imports` contains invalid paths (i.e. paths with zero
    /// segments).
    ///
    #[must_use]
    fn generate_consumer(&self) -> String {
        // If there's exactly one function in this impl, we want to push the impl docs down into the
        // function so that they're move visible in the consumer API. This is useful for
        // implementing traits like `PartialEq`, where the main API is `eq`, and any notes on the
        // consumer implementation ought to be exposed there instead of on the extension.
        let (module_docs_for_fn, mut result) = if self.fns.len() == 1 {
            (Some(&*self.doc_comments), String::default())
        } else {
            (None, crate::consumer::consumer_docs_from(&*self.doc_comments, 0))
        };
        result.push_str(&format!(
"public extension {native_type} {{
{functions}
}}",
            native_type = self.type_name.to_string(),
            functions = self
                .fns
                .iter()
                .map(|f| f.generate_consumer(&self.module_name(), module_docs_for_fn))
                .collect::<Vec<String>>()
                .join("\n\n"),
        ));
        result
    }
}

impl From<&ImplFFI> for String {
    fn from(impl_ffi: &ImplFFI) -> Self {
        [
            super::header_and_imports(&*impl_ffi.consumer_imports),
            impl_ffi.generate_consumer(),
        ]
        .join("\n")
    }
}
