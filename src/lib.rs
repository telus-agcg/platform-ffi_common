//! # `ffi_common`
//!
//! Crate for common FFI behaviors needed by other Rust crates that provide a C interface.
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

pub mod error;
pub mod memory;
