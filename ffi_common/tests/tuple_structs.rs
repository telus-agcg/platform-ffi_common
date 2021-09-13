#[derive(Debug, Clone, ffi_common::derive::FFI)]
pub struct SomeType {
    bar: u8,
}

#[derive(Debug, Clone, ffi_common::derive::FFI)]
pub struct Wrapper(SomeType, SomeType);

#[test]
fn access_wrapper_ffo() {
    use crate::{some_type_ffi, wrapper_ffi};
    let value1 = 42;
    let value2 = 99;
    unsafe {
        let input1 = some_type_ffi::some_type_init(value1) as *mut SomeType;
        let input2 = some_type_ffi::some_type_init(value2) as *mut SomeType;
        let wrapper = wrapper_ffi::wrapper_init(input1, input2);
        let output1 = wrapper_ffi::get_wrapper_unnamed_field_0(wrapper);
        let output2 = wrapper_ffi::get_wrapper_unnamed_field_1(wrapper);
        assert_eq!(value1, (*output1).bar);
        assert_eq!(value2, (*output2).bar);
    }
}
