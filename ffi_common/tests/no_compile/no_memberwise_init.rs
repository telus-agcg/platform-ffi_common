use ffi_common::derive::FFI;

#[derive(Debug, Clone, Copy, FFI)]
#[ffi(forbid_memberwise_init)]
pub struct NoInitializerAllowed {
    pub field1: u8,
}

fn test_foo() {
    // This should not compile because of the `forbid_memberwise_init` attribute.
    let _ = unsafe { no_initializer_allowed_ffi::no_initializer_allowed_rust_ffi_init(1) };
}

fn main() { }
