//!
//! Contains representations of and FFI implementations for the different kinds of structs we
//! support. `custom` for our purposes refers to a struct that derives an FFI but supplies the
//! `ffi(custom = "crate/relative/path")` attribute to reference a file with a handwritten FFI for
//! the type. We still generate some supporting wrappers and conveniences, as well as the wrapping
//! consumer, but we expect the file specified in the `custom` attributes to do the heavy lifting
//! (initializer and getter fns).
//!

pub mod custom;
pub mod standard;
