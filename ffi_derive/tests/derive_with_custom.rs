mod custom_ffi;

#[test]
fn test_custom_ffi() {
    let value = "meow";
    let ffi_type = custom_ffi::ffi::ffi_type_init(ffi_common::ffi_string!(value));
    let retrieved_value = custom_ffi::ffi::get_ffi_type_value(ffi_type);
    assert_eq!(value, ffi_common::string::string_from_c(retrieved_value));
    unsafe { custom_ffi::types::ffi_type_ffi::ffi_type_free(ffi_type) };
}
