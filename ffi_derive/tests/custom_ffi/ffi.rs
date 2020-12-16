use super::types::{FFIType, NonFFI};
use std::os::raw::c_char;

#[allow(box_pointers)]
#[no_mangle]
pub extern "C" fn ffi_type_init(value: *const c_char) -> *const FFIType {
    let string = ffi_common::string::string_from_c(value);
    let ffi_type = FFIType {
        non_ffi_field: NonFFI { value: string },
    };
    Box::into_raw(Box::new(ffi_type))
}

#[no_mangle]
pub extern "C" fn get_ffi_type_value(ptr: *const FFIType) -> *const c_char {
    unsafe { ffi_common::ffi_string!((&*ptr).non_ffi_field.value.clone()) }
}
