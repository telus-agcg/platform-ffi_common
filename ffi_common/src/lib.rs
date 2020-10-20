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

pub mod codegen_helpers;
#[macro_use]
pub mod error;
pub mod datetime;
pub mod macros;
pub mod string;

use paste::paste;

declare_value_type_ffi!(bool, u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_none() {
        let null_pointer = option_u8_init(false, 0);
        assert!(null_pointer.is_null());
    }

    #[test]
    fn test_ffi_some() {
        let u8_pointer = option_u8_init(true, 3);
        assert!(!u8_pointer.is_null());
        assert_eq!(unsafe { *Box::from_raw(u8_pointer as *mut u8) }, 3);
    }
}
