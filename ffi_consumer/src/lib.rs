//!
//! # `ffi_consumer`
//!
//! A library for generating the FFI consumer for another Rust crate. `ffi_derive` produces a C
//! interface for your types, and `ffi_consumer` produces native types for the consumer, safely
//! wrapping that C interface.
//!
//! ## Usage
//!
//! Libraries that want to generate an interface for the FFI consumer (i.e., the language on the
//! other side of the boundary) must do the following:
//! 1. Add `ffi_consumer`, `ffi_common`, and `cbindgen` to `[build-dependencies] in `Cargo.toml`.
//! 2. Set the environment variables `FFI_CONSUMER_ROOT_DIR` and `FFI_HEADER_DIR` to the path you
//! want the consumer files written at (it doesn't have to exist as long as it's valid; we'll create
//! any necessary directories on the way).
//! 3. Create a `build.rs` file at the root of the crate with the following:
//! ```ignore
//! fn main() {
//!     let consumer_out_dir = match option_env!("FFI_CONSUMER_ROOT_DIR") {
//!         Some(dir) => dir.to_string(),
//!         None => std::env::var("OUT_DIR").expect("OUT_DIR will always be set by cargo."),
//!     };
//!     let consumer_dir = ffi_common::codegen_helpers::create_consumer_dir(&consumer_out_dir)
//!         .expect("Unable to create consumer dir");
//!     ffi_consumer::write_consumer_foundation(&consumer_dir, "swift")
//!         .expect("Unable to write consumer files");
//!
//!     let header_out_dir = match option_env!("FFI_HEADER_DIR") {
//!         Some(dir) => dir.to_string(),
//!         None => std::env::var("OUT_DIR").expect("OUT_DIR will always be set by cargo."),
//!     };
//!     let crate_dir = env!("CARGO_MANIFEST_DIR");
//!     cbindgen::generate(crate_dir)
//!         .expect("Unable to generate bindings")
//!         .write_to_file(format!("{}/agrian_types.h", header_out_dir));
//! }
//! ```
//!

#![warn(
    future_incompatible,
    missing_copy_implementations,
    nonstandard_style,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_qualifications,
    unused_results,
    variant_size_differences,
    clippy::all,
    clippy::complexity,
    clippy::correctness,
    clippy::pedantic,
    clippy::perf,
    clippy::nursery,
    clippy::style
)]
#![forbid(missing_docs, unused_extern_crates, unused_imports)]

mod error;
mod primitives_conformance;

pub mod consumer_struct;
pub use error::Error;

/// Call this to write protocols and primitive conformance to those protocols to `consumer_dir`.
///
/// Note: `consumer_dir` must exist.
///
/// # Errors
///
/// Returns an error if we fail to read any of the supporting language files, or to write any of the
/// conformance files.
///
pub fn write_consumer_foundation(consumer_dir: &str, language: &str) -> Result<(), Error> {
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
        .map(|entry| -> Result<(), Error> {
            let entry = entry?;
            let mut file_data = header();
            file_data.push_str(&std::fs::read_to_string(entry.path())?);
            std::fs::write(
                format!("{}/{}", &consumer_dir, entry.file_name().into_string()?),
                file_data,
            )
            .map_err(Error::from)
        })
        .collect()
}

/// Write protocol conformance for all the supported primitive types to files in `consumer_dir`.
///
fn write_primitive_conformances(consumer_dir: &str) -> Result<(), std::io::Error> {
    [
        "bool", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64",
    ]
    .iter()
    .map(|native_type| {
        let consumer_type = ffi_common::codegen_helpers::consumer_type_for(native_type, false);
        // Note: This is only accurate for Swift primitives, whose FFI and consumer types happen to
        // match. Don't assume consumer_type == ffi_type for non-primitive types, or for primitives
        // in other languages.
        let ffi_type = &consumer_type;
        let default_value = if native_type == &"bool" { "false" } else { "0" };
        let mut conformance_file = header();
        conformance_file.push_str(&primitives_conformance::generate(
            native_type,
            ffi_type,
            &consumer_type,
            default_value,
        ));
        std::fs::write(
            format!("{}/{}.swift", consumer_dir, consumer_type),
            conformance_file,
        )
    })
    .collect()
}

/// Returns a warning to add to the top of each file. Could add a date or customize the comment
/// format if we ever want to.
///
fn header() -> String {
    "/* This was generated by the Rust `ffi_consumer` crate. Don't modify this manually. */\n\n"
        .to_string()
}
