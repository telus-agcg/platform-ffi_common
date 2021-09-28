//! # `ffi_common`
//!
//! Crate for common Rust FFI behaviors, including error, string, and primitive handling in `core`,
//! generating an ffi with `derive`, and generating consumer types around that FFI with `consumer`.
//!

#![warn(
    clippy::all,
    clippy::correctness,
    clippy::nursery,
    clippy::pedantic,
    future_incompatible,
    missing_copy_implementations,
    nonstandard_style,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_qualifications,
    unused_results,
    variant_size_differences
)]
#![forbid(missing_docs, unused_extern_crates, unused_imports)]

pub use ffi_core as core;
pub use ffi_derive as derive;
pub use ffi_internals::consumer;
