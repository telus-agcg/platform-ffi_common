use ffi_derive::FFI;

/// This type is never exposed to FFI for reasons.
///
#[derive(Clone, Debug)]
pub struct NonFFI {
    pub value: String,
}

/// This type is exposed to FFI, but it provides its own initializer and getters in
/// `tests/custom_ffi/ffi.rs`.
///
#[derive(Clone, Debug, FFI)]
#[ffi(custom = "tests/custom_ffi/ffi.rs")]
pub struct FFIType {
    pub non_ffi_field: NonFFI,
}
