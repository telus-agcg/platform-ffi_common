//!
//! Contains representations of and FFI implementations for the different kinds of enums we
//! support. `complex` for our purposes refers to any enum that *isn't* `repr(C)`, since we have to
//! wrap those like we do structs (instead of just exposing them directly with some helpers, which
//! is what we do for `repr(C)` enums).
//!

pub mod complex;
pub mod reprc;
