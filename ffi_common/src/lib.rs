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

#[macro_use]
pub mod error;
pub mod datetime;
pub mod macros;
pub mod string;

use paste::paste;

declare_value_type_array_struct!(bool, u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);
