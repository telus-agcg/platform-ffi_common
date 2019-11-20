//! Common FFI behaviors related to managing memory for language interop.
//!

use crate::error;
use std::{ffi::CString, os::raw::c_char};

/// Free a string that was created in Rust.
///
/// Some Rust FFI functions return a `*const c_char`. The data these point
/// to should be copied into client-owned memory, after which the pointer
/// should be passed to `free_rust_string` so that Rust can safely free it.
///
/// You *must not* use the pointer after passing it to `free_rust_string`.
///
#[no_mangle]
pub extern "C" fn free_rust_string(string: *const c_char) {
    error::clear_last_err_msg();
    unsafe {
        if string.is_null() {
            return;
        }
        let _ = CString::from_raw(string as *mut c_char);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn can_free_string() {
        error::set_last_err_msg("testy test test".to_string());
        let error = error::get_last_err_msg();
        let error_bytes = unsafe { CStr::from_ptr(error).to_bytes() };
        assert!(!error_bytes.is_empty());
        free_rust_string(error);
        let error_bytes_after_free = unsafe { CStr::from_ptr(error).to_bytes() };
        assert!(error_bytes_after_free.is_empty());
    }
}
