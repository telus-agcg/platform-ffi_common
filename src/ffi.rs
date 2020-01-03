//!
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

/// Get the last error message stored by the library.
///
/// Note that as with all other references to string data originating in Rust, clients *must* call
/// `free_rust_string` with this pointer once its data has been copied into client-owned memory.
///
#[must_use]
#[no_mangle]
pub extern "C" fn get_last_err_msg() -> *const c_char {
    let mut msg: Option<String> = None;
    error::LAST_ERROR.with(|last_error| {
        msg = last_error.borrow().clone();
    });
    match msg {
        Some(string) => try_or_set_error!(CString::new(string)).into_raw(),
        None => std::ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn can_free_string() {
        error::set_last_err_msg("testy test test");
        let error = get_last_err_msg();
        let error_bytes = unsafe { CStr::from_ptr(error).to_bytes() };
        assert!(!error_bytes.is_empty());
        free_rust_string(error);
        let error_bytes_after_free = unsafe { CStr::from_ptr(error).to_bytes() };
        assert!(error_bytes_after_free.is_empty());
    }
}
