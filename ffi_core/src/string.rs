//!
//! Common FFI behaviors related to managing strings for language interop.
//!

#![allow(clippy::module_name_repetitions)]

use std::{
    ffi::{CStr, CString},
    mem::ManuallyDrop,
    os::raw::c_char,
};
use uuid::Uuid;

/// An FFI-safe representation of a collection of string data. Use to communicate a `Vec<String>`,
/// `Vec<uuid::Uuid>`, etc. across the FFI boundary.
///
/// This can also express an `Option<Vec<String>>` with a null pointer and a len and capacity of 0.
/// ```
/// use ffi_core::string::FFIArrayString;
/// FFIArrayString {
///     ptr: std::ptr::null(),
///     len: 0,
///     cap: 0,
/// };
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
#[allow(missing_copy_implementations)]
#[derive(Debug)]
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
/// # Panics
/// 
/// This will panic if, for any element in `ptr`, we cannot convert a `CStr` to a `str`.
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
        v.as_slice().into()
    }
}

impl<T: ToString> From<&[T]> for FFIArrayString {
    /// Convenience for converting any string-like vec into an `FFIArrayString`.
    ///
    fn from(slice: &[T]) -> Self {
        let v: ManuallyDrop<Vec<*const c_char>> = ManuallyDrop::new(
            slice
                .iter()
                .map(|s| {
                    let c_string = try_or_set_error!(
                        CString::new(s.to_string()).map(std::ffi::CString::into_raw)
                    );
                    c_string
                })
                .collect(),
        );
        let len = v.len();
        let ptr = v.as_ptr();
        let cap = v.capacity();

        Self { ptr, len, cap }
    }
}

impl<T: ToString> From<Option<&[T]>> for FFIArrayString {
    /// Convenience for converting any string-like vec into an `FFIArrayString`.
    ///
    fn from(opt: Option<&[T]>) -> Self {
        opt.map_or(
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
            // Create a vec from the data in the array, but don't let Rust drop it. That will happen
            // when the consumer tells us they're done with the array by calling
            // `free_ffi_array_string`. Clone it into one that we can use safely.
            Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap)
                .into_iter()
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
            Some(Vec::from(array))
        }
    }
}

#[allow(clippy::use_self)]
impl From<FFIArrayString> for Vec<Uuid> {
    fn from(array: FFIArrayString) -> Self {
        unsafe {
            Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap)
                .into_iter()
                .map(|s| {
                    Uuid::parse_str(CString::from_raw(s as *mut c_char).to_str().unwrap()).unwrap()
                })
                .collect()
        }
    }
}

/// This is a gross type, but we have to support it because a) some services (notably core's
/// associations type) return optional collections, and b) other services actually distinguish
/// between `None` and `[]` to mean nothing vs everything. Avoid Option<Vec<T>> if you can, but
/// it's sometimes required to describe service resources in `agrian_types`.
///
impl From<FFIArrayString> for Option<Vec<Uuid>> {
    fn from(array: FFIArrayString) -> Self {
        if array.ptr.is_null() {
            None
        } else {
            Some(Vec::from(array))
        }
    }
}

/// Pass an `FFIArrayString` to this method to allow Rust to reclaim ownership of the object so that
/// it can be safely deallocated.
///
/// # Safety
///
/// We're assuming that the memory in the `FFIArrayString` you give us was allocated by Rust in the
/// process of creating an `FFIArrayString` initialized natively in Rust (either internally
/// or through the provided FFI constructor `ffi_array_*_init`). If you do something bizarre (like
/// initializing an `FFIArrayString` on the other side of the FFI boundary), this will have
/// undefined behavior. Don't do that.
///
/// You **must not** access `array` after passing it to `ffi_array_string_free`.
///
/// It is safe to call this method with an `array` whose `ptr` is null; we won't double-free or free
/// unallocated memory if, for example, you pass an array that represents the `None` variant of an
/// `Option<Vec<T>>`.
///
#[no_mangle]
pub unsafe extern "C" fn ffi_array_string_free(array: FFIArrayString) {
    if array.ptr.is_null() {
        return;
    }
    let v = Vec::from_raw_parts(array.ptr as *mut *const c_char, array.len, array.cap);
    for s in v {
        free_rust_string(s);
    }
}

/// Converts a string to a raw, unowned pointer.
///
/// If there's a previous error, it will be cleared when calling this. If an error occurs, this will
/// return std::ptr::null(), and you can check the error with `error::get_last_err_msg()`.
///
#[macro_export]
macro_rules! ffi_string {
    ($string:expr) => {{
        $crate::error::clear_last_err_msg();
        let c_string = $crate::try_or_set_error!(
            std::ffi::CString::new($string).map(std::ffi::CString::into_raw)
        );
        c_string
    }};
}

/// Converts an FFI string (a `*const c_char`) to a `Uuid`.
/// 
/// # Safety
/// 
/// `ptr` is unchecked and will be dereferenced, so it must not be null.
/// 
/// # Panics
/// 
/// This will panic if we cannot parse the string at `ptr` as a `Uuid`.
///
#[must_use]
pub unsafe fn uuid_from_c(ptr: *const c_char) -> Uuid {
    Uuid::parse_str(&CStr::from_ptr(ptr).to_string_lossy()).unwrap()
}

/// Converts an FFI string (a `*const c_char`) to a `String`.
/// 
/// # Safety
/// 
/// `ptr` is unchecked and will be dereferenced, so it must not be null.
/// 
#[must_use]
pub unsafe fn string_from_c(ptr: *const c_char) -> String {
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

/// Free a string that was created in Rust.
///
/// Some Rust FFI functions return a `*const c_char`. The data these point
/// to should be copied into client-owned memory, after which the pointer
/// should be passed to `free_rust_string` so that Rust can safely free it.
///
/// # Safety
/// 
/// We assume that the memory behind `string` was allocated by Rust. Don't call this with an object
/// created on the other side of the FFI boundary; that is undefined behavior.
/// 
/// You **must not** access `string` after passing it to this method.
/// 
/// It's safe to call this with a null pointer.
///
#[no_mangle]
pub unsafe extern "C" fn free_rust_string(string: *const c_char) {
    if string.is_null() {
        return;
    }
    drop(CString::from_raw(string as *mut c_char));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn can_free_string() {
        unsafe {
            crate::error::set_last_err_msg("testy test test");
            let error = crate::error::get_last_err_msg();
            let error_bytes = CStr::from_ptr(error).to_bytes();
            assert!(!error_bytes.is_empty());
            let original_pointee = *error;
            assert_eq!(*error, original_pointee);
            free_rust_string(error);
            assert_ne!(*error, original_pointee);
        }
    }

    #[test]
    fn string_array_move_semantics() {
        let v = vec!["one", "two"];
        let string_array = FFIArrayString::from(&*v);
        let v2: Vec<String> = string_array.into();
        // Both v and v2 are safe to access; v's data was cloned into `string_array` so that it can
        // be transferred across the FFI boundary regardless of the lifetime of v. v2 is created by
        // taking ownership of `string_array` (giving us use of that data passed from the FFI
        // consumer, and simultaneously reclaiming the memory occupied by the FFI type).
        assert_eq!(v, v2);
    }
}
