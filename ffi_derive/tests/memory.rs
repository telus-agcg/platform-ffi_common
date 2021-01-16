//!
//! Tests that cover memory behavior with FFI. In general, we want to pass an FFI type in to an ffi
//! init method, then make sure that the pointee is not the same afterward. It's not super easy to
//! make sense of, but afaik that's the closest we can get to asserting that the original values
//! were dealloc'd as expected; any other inspection of the pointer is UB (it could be a null
//! pointer, or it could (and frequently will be) reused to point to the result of another reference
//! type allocation.)
//!

use ffi_common;
use std::os::raw::c_char;
use uuid::Uuid;

#[derive(Debug, Clone, ffi_derive::FFI)]
pub struct UuidStruct {
    collection_of_ids: Vec<Uuid>,
}

use uuid_struct_ffi::*;

#[derive(Debug, Clone, ffi_derive::FFI)]
pub struct NestedStruct {
    collection_of_structs: Vec<UuidStruct>,
}

#[test]
fn check_uuid_vec_memory_after_free() {
    unsafe {
        let v = vec![Uuid::new_v4(), Uuid::new_v4()];
        let string_array = ffi_common::string::FFIArrayString::from(&*v);
        let unsafe_ptr = string_array.ptr as *mut *mut c_char;
        let original_pointee = *unsafe_ptr;

        assert_eq!(*unsafe_ptr, original_pointee);
        let uuid_struct = uuid_struct_init(string_array);
        // Flaky test. Nothing guarantees or requires that `unsafe_ptr`'s memory be immediately
        // changed just because the pointer has been reclaimed and dropped by the Rust allocator.
        assert_ne!(*unsafe_ptr, original_pointee);
        uuid_struct_free(uuid_struct);
    }
}

#[test]
fn check_struct_vec_memory_after_free() {
    unsafe {
        let ids1 = vec![Uuid::new_v4(), Uuid::new_v4()];
        let ids2 = vec![Uuid::new_v4(), Uuid::new_v4()];
        let inner_struct1 = *Box::from_raw(uuid_struct_init((&*ids1).into()) as *mut _);
        let inner_struct2 = *Box::from_raw(uuid_struct_init((&*ids2).into()) as *mut _);
        let v = vec![inner_struct1, inner_struct2];
        let inner_array = FFIArrayUuidStruct::from(&*v);
        let unsafe_ptr = inner_array.ptr;
        let original_value = *unsafe_ptr;

        assert_eq!(*unsafe_ptr, original_value);
        let outer_struct = nested_struct_ffi::nested_struct_init(inner_array);
        assert_ne!(*unsafe_ptr, original_value);
        nested_struct_ffi::nested_struct_free(outer_struct);
    }
}
