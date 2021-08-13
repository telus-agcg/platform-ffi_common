//!
//! This crate provides low level FFI functionality for primitives, error handling, and generating
//! FFI-safe structures for `Option` and `Vec` generics for simple struct and enum types.
//!

#![deny(unused_extern_crates, missing_docs)]
#![warn(
    box_pointers,
    clippy::all,
    clippy::correctness,
    clippy::nursery,
    clippy::pedantic,
    future_incompatible,
    missing_copy_implementations,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unused_qualifications,
    unused_results,
    variant_size_differences
)]
#![allow(box_pointers)]

pub use paste::paste;

#[macro_use]
pub mod error;
pub mod datetime;
#[macro_use]
pub mod macros;
pub mod string;

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
