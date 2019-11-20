//! Thread local error storage for FFI.
//!
//! Errors may occur when a foreign interface function is called. Since we can't return a Rust
//! `Result` type across language boundaries, FFI crates need to provide a way for clients to
//! retrieve errors from the library. This module provides a native interface for setting and
//! clearing the most recent error that occurred in the current thread, and an FFI for retrieving
//! that error as a "string" (`*const c_char`).
//!

use std::{cell::RefCell, ffi::CString, os::raw::c_char};

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = RefCell::new(None);
}

/// Set the stored error message.
///
/// Errors that occur during an FFI function (either from normal library code execution or from
/// FFI-specific code) should cause the function to return something that indicates to the client
/// that an error occurred, and to log a description of that error here.
///
pub fn set_last_err_msg(msg: String) {
    LAST_ERROR.with(|last_error| {
        *last_error.borrow_mut() = Some(msg);
    });
}

/// Clear any stored error message.
///
/// In general, this should be used at the start of an FFI function to ensure that clients don't
/// end up retrieving earlier errors if the function fails to set a new error that occurs, or a
/// client requests errors unnecessarily.
///
pub fn clear_last_err_msg() {
    LAST_ERROR.with(|last_error| {
        *last_error.borrow_mut() = None;
    });
}

/// Get the last error message stored by the library.
///
/// Note that as with all other references to string data originating in Rust, clients *must* call
/// `free_rust_string` with this pointer once its data has been copied into client-owned memory.
///
#[no_mangle]
pub extern "C" fn get_last_err_msg() -> *const c_char {
    let mut msg: Option<String> = None;
    LAST_ERROR.with(|last_error| {
        msg = last_error.borrow().clone();
    });
    match msg {
        Some(str) => match CString::new(str) {
            Ok(ffi_string) => ffi_string.into_raw(),
            Err(why) => {
                set_last_err_msg(why.to_string());
                std::ptr::null()
            }
        },
        None => std::ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn can_get_error() {
        let error = "dummy error";
        set_last_err_msg(error.to_string());
        let result = get_last_err_msg();
        let result_c: &CStr = unsafe { CStr::from_ptr(result) };
        let returned_error = result_c.to_str().expect("Failed to get str from CStr");
        assert_eq!(error, returned_error);
    }

    #[test]
    fn can_clear_error() {
        let error = "dummy error".to_string();
        set_last_err_msg(error);
        clear_last_err_msg();
        assert_eq!(std::ptr::null(), get_last_err_msg());
    }
}
