//!
//! Module for generating code for the consumer side of the ffi.
//! 
//! Libraries that want to generate an interface for the FFI consumer (i.e., the language on the
//! other side of the boundary) must do the following:
//! 1. Add `ffi_common` to `[build-dependencies] in `Cargo.toml`.
//! 1. Set the environment variable `FFI_CONSUMER_ROOT_DIR` to the path you want the consumer files
//! written at (it doesn't have to exist as long as it's valid; we'll create any necessary
//! directories on the way). We'll write the consumer files for each crate to a subdirectory using
//! the crate's package name.
//! 1. If you need a common framework imported into the generated code (for example, you may want to
//! put the generated primitives and other FFI glue code in one framework, but put each crate's
//! generated consumer code in its own consumer framework to avoid having a single massive
//! interface), you can specify that common framework with the environment variable
//! `"FFI_COMMON_FRAMEWORK"`.
//! 1. Create a `build.rs` file at the root of the crate with the following:
//! ```ignore
//! fn main() {
//!     let consumer_out_dir = match option_env!("FFI_CONSUMER_ROOT_DIR") {
//!         Some(dir) => dir.to_string(),
//!         None => std::env::var("OUT_DIR").expect("OUT_DIR will always be set by cargo."),
//!     };
//!     ffi_common::consumer::write_consumer_foundation(&consumer_dir, "swift")
//!         .expect("Unable to write consumer files");
//! }
//! ```
//! 

#![allow(clippy::module_name_repetitions)]

use heck::CamelCase;

mod error;
mod primitives_conformance;

pub mod consumer_enum;
pub mod consumer_fn;
pub mod consumer_impl;
pub mod consumer_struct;
pub use error::Error;
use quote::spanned::Spanned;

/// A warning to add to the top of each file. Could add a date or customize the comment format if we
/// ever want to.
///
pub const HEADER: &str =
    "/* This was generated by the Rust `ffi_consumer` crate. Don't modify this manually. */";

/// Call this to write protocols and primitive conformance to those protocols to `consumer_dir`.
///
/// Note: If `consumer_dir` does not exist, it will be created (along with any missing parent
/// directories).
///
/// # Errors
///
/// Returns an error if we fail to read any of the supporting language files, or to write any of the
/// conformance files.
///
pub fn write_consumer_foundation(consumer_dir: &str, language: &str) -> Result<(), Error> {
    let consumer_dir = format!("{}/common", consumer_dir);
    let consumer_dir = super::create_consumer_dir(&consumer_dir)?;
    write_support_files(consumer_dir, language)?;
    write_primitive_conformances(consumer_dir)?;
    Ok(())
}

/// Reads the protocol file for `language` and writes it to `consumer_dir/FFIProtocols.language`.
///
/// This is a file in the consumer's language that contains any generic or non-type-specific
/// implementations needed for FFI support.
///
fn write_support_files(consumer_dir: &str, language: &str) -> Result<(), Error> {
    let crate_root = env!("CARGO_MANIFEST_DIR");
    let support_files = format!("{}/support/{}", crate_root, language);

    std::fs::read_dir(support_files)?
        .try_for_each(|entry| -> Result<(), Error> {
            let entry = entry?;
            let file_data: String = [HEADER, &std::fs::read_to_string(entry.path())?].join("\n\n");
            std::fs::write(
                format!("{}/{}", &consumer_dir, entry.file_name().into_string()?),
                file_data,
            )
            .map_err(Error::from)
        })
}

/// Write protocol conformance for all the supported primitive types to files in `consumer_dir`.
///
fn write_primitive_conformances(consumer_dir: &str) -> Result<(), std::io::Error> {
    [
        "bool", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64",
    ]
    .iter()
    .try_for_each(|native_type| {
        let consumer_type = crate::consumer_type_for(native_type, false);
        // Note: This is only accurate for Swift primitives, whose FFI and consumer types happen to
        // match. Don't assume consumer_type == ffi_type for non-primitive types, or for primitives
        // in other languages.
        let ffi_type = &consumer_type;
        let conformance_file: String = [
            HEADER,
            &primitives_conformance::generate(native_type, ffi_type, &consumer_type),
        ]
        .join("\n\n");
        std::fs::write(
            format!("{}/{}.swift", consumer_dir, consumer_type),
            conformance_file,
        )
    })
}

/// Turns a path segment into a camel cased string.
/// 
/// # Errors
/// 
/// Returns an error if `segment` is `None`.
/// 
fn get_segment_ident(segment: Option<&syn::PathSegment>) -> &syn::Ident {
    match segment {
        Some(segment) => &segment.ident,
        None => proc_macro_error::abort!(segment.__span(), "Missing path segment"),
    }
}

/// Turns a slice of paths into a vec of consumer import statements
/// 
/// # Errors
/// 
/// Returns an error if any element in `paths` has zero segments.
/// 
fn build_imports(paths: &[syn::Path]) -> Vec<String> {
    paths
        .iter()
        .map(|path| {
            let crate_name = get_segment_ident(path.segments.first()).to_string().to_camel_case();
            let type_name = get_segment_ident(path.segments.last()).to_string().to_camel_case();
            format!("import class {}.{}", crate_name, type_name)
        })
        .collect()
    // paths
    //     .iter()
    //     .try_fold(vec![], |mut acc, path| {
    //         let crate_name = get_segment_ident(path.segments.first())?.to_string().to_camel_case();
    //         let type_name = get_segment_ident(path.segments.last())?.to_string().to_camel_case();
    //         acc.push(format!("import class {}.{}", crate_name, type_name));
    //         Ok(acc)
    //     })
}
