//!
//! Common FFI behaviors related to managing strings for language interop.
//!

#![allow(clippy::module_name_repetitions)]

use crate::error;
use std::{
    ffi::{CStr, CString},
    mem::ManuallyDrop,
    os::raw::c_char,
};
use uuid::Uuid;

/// An FFI-safe representation of a collection of string data. Use to communicate a `Vec<String>`,
/// `Vec<UUID::Uuid>`, etc. across the FFI boundary.
///
/// This can also express an `Option<Vec<String>>` with a null pointer and a len and capacity of 0.
/// ```
/// FFIArrayString {
///     ptr: std::ptr::null(),
///     len: 0,
///     cap: 0,
/// }
/// ```
///
/// FFI consumers should therefore make sure that the pointer is not null (although our generated
/// code should be able to preserve optionality across the FFI boundary, so it will only have to
/// check in places where null is really possible.)
///
/// # Safety
///
/// This will need to be brought back into rust ownership in two ways; first, the vec needs to
/// be reclaimed with `Vec::from_raw_parts`; second, each `CString` element of the vec will need
/// to be reclaimed with `CString::from_raw`. Pass this `FFIArrayString` to
/// `free_ffi_array_string` when you're done with it (i.e., when you've copied it into native
/// memory, displayed it, whatever you're doing on the other side of the FFI boundary) so we can
/// take care of those steps.
///
/// # Performance
///
/// Note that creating this struct requires creating new `CString`s from the original vec, which
/// means:
/// 1. There's a cost to getting an array of strings (just like getting a single string, we have to
/// allocate a `CString` for each element in the original collection).
/// 2. The lifetime of this struct is unrelated to the lifetime of whatever may hold the array in
/// Rust. That's why this struct has to be returned to Rust to clean up, and why it can outlive the
/// object it came from. This lends some flexibility for optimizing large arrays; clients are free
/// to hold on to this struct indefinitely, reading from it as needed instead of copying the array
/// contents into native memory up front.
///
#[repr(C)]
#[derive(Clone, Debug)]
pub struct FFIArrayString {
    #[doc = "Pointer to the first element in the array."]
    pub ptr: *const *const c_char,
    #[doc = "The length of (i.e. the number of elements in) this array."]
    pub len: usize,
    #[doc = "The capacity with which this array was allocated."]
    pub cap: usize,
}

/// Initialize an array of strings from across the FFI boundary. This will copy the provided data
/// into Rust memory.
///
/// # Safety
///
/// The pointer you send must point to the first element of an array whose elements are each
/// pointers to the first character of a null-terminated array of utf8-encoded characters.
///
/// If `ptr` is a null pointer, this will create an `FFIArrayString` with a length and capacity of
/// `0`, and a null pointer; this expresses the `None` variant of an `Option<Vec<T: ToString>>`.
///
/// **Important: do not pass a null pointer if the field that this array will be used with is not
/// an `Option`.**
///
/// This is the only way to safely construct an `FFIArrayString` from the non-Rust side of the FFI
/// boundary. We assume that all instances of `FFIArrayString` are allocated by Rust, as this allows
/// us to greatly simplify memory management.
///
#[must_use]
#[no_mangle]
pub unsafe extern "C" fn ffi_array_string_init(
    ptr: *const *const c_char,
    len: isize,
) -> FFIArrayString {
    if ptr.is_null() {
        FFIArrayString {
            ptr: std::ptr::null(),
            len: 0,
            cap: 0,
        }
    } else {
        let mut v = vec![];
        for i in 0..len {
            let x = *ptr.offset(i);
            let c = CStr::from_ptr(x).to_str().unwrap().to_string();
            v.push(c);
        }
        (&v).into()
    }
}

impl<T: ToString> From<&Vec<T>> for FFIArrayString {
    /// Convenience for converting any string-like vec into an `FFIArrayString`.
    ///
    fn from(v_s: &Vec<T>) -> Self {
        let v: ManuallyDrop<Vec<*const c_char>> = ManuallyDrop::new(
            v_s.iter()
                .map(|s| {
                    let c_string = try_or_set_error!(CString::new(s.to_string()));
                    let c: *const c_char = c_string.into_raw();
                    c
                })
                .collect(),
        );
        let len = v.len();
        let ptr = v.as_ptr();
        let cap = v.capacity();

        Self { ptr, len, cap }
    }
}

impl<T: ToString> From<&Option<Vec<T>>> for FFIArrayString {
    /// Convenience for converting any string-like vec into an `FFIArrayString`.
    ///
    fn from(opt: &Option<Vec<T>>) -> Self {
        opt.as_ref().map_or(
            Self {
                ptr: std::ptr::null(),
                len: 0,
                cap: 0,
            },
            |v| v.into(),
        )
    }
}

#[allow(clippy::use_self)]
impl From<FFIArrayString> for Vec<String> {
    fn from(array: FFIArrayString) -> Self {
        unsafe {
            let v = Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap);
            v.into_iter()
                .map(|s| {
                    CString::from_raw(s as *mut c_char)
                        .to_str()
                        .unwrap()
                        .to_string()
                })
                .collect()
        }
    }
}

impl From<FFIArrayString> for Option<Vec<String>> {
    fn from(array: FFIArrayString) -> Self {
        if array.ptr.is_null() {
            None
        } else {
            unsafe {
                let v = Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap);
                Some(
                    v.into_iter()
                        .map(|s| {
                            CString::from_raw(s as *mut c_char)
                                .to_str()
                                .unwrap()
                                .to_string()
                        })
                        .collect(),
                )
            }
        }
    }
}

#[allow(clippy::use_self)]
impl From<FFIArrayString> for Vec<Uuid> {
    fn from(array: FFIArrayString) -> Self {
        unsafe {
            let v = Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap);
            v.into_iter()
                .map(|s| {
                    Uuid::parse_str(CString::from_raw(s as *mut c_char).to_str().unwrap()).unwrap()
                })
                .collect()
        }
    }
}

impl From<FFIArrayString> for Option<Vec<Uuid>> {
    fn from(array: FFIArrayString) -> Self {
        if array.ptr.is_null() {
            None
        } else {
            unsafe {
                let v = Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap);
                Some(
                    v.into_iter()
                        .map(|s| {
                            Uuid::parse_str(CString::from_raw(s as *mut c_char).to_str().unwrap())
                                .unwrap()
                        })
                        .collect(),
                )
            }
        }
    }
}

impl Drop for FFIArrayString {
    fn drop(&mut self) {
        println!("> Dropping string array: {:?}", self);
    }
}

/// Pass an `FFIArrayString` to this method to allow Rust to reclaim ownership of the object so that
/// it can be safely deallocated.
///
/// # Safety
///
/// We're assuming that the memory in the `FFIArrayString` you give us was allocated by Rust in the
/// process of creating an `FFIArrayString` initialized natively in Rust. If you do something
/// bizarre (like initializing an `FFIArrayString` on the other side of the FFI boundary), this will
/// have undefined behavior. Don't do that.
///
/// You **must not** access `array` after passing it to `free_ffi_array_string`.
///
/// Null pointers will be a no-op.
///
#[no_mangle]
pub extern "C" fn free_ffi_array_string(array: FFIArrayString) {
    error::clear_last_err_msg();
    if array.ptr.is_null() {
        return;
    }
    unsafe {
        let v = Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap);
        for s in v {
            free_rust_string(s);
        }
    }
}

/// Free a string that was created in Rust.
///
/// Some Rust FFI functions return a `*const c_char`. The data these point
/// to should be copied into client-owned memory, after which the pointer
/// should be passed to `free_rust_string` so that Rust can safely free it.
///
/// # Safety
///
/// You **must not** use the pointer after passing it to `free_rust_string`.
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
        error::set_last_err_msg("testy test test");
        let error = error::get_last_err_msg();
        let error_bytes = unsafe { CStr::from_ptr(error).to_bytes() };
        assert!(!error_bytes.is_empty());
        free_rust_string(error);
        let error_bytes_after_free = unsafe { CStr::from_ptr(error).to_bytes() };
        assert!(error_bytes_after_free.is_empty());
    }
}
