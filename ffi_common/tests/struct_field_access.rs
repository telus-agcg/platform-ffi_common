//!
//! Tests that cover accessing fields of various types through the derived FFI getters.
//!

use chrono::NaiveDateTime;
use ffi_common::{
    core::{datetime::FFIArrayTimeStamp, string::FFIArrayString, *},
    derive,
};
use std::{convert::TryInto, ffi::CStr};
use uuid::Uuid;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, derive::FFI)]
pub enum TestEnum {
    Variant1,
    Variant2,
}

impl Default for TestEnum {
    fn default() -> Self {
        Self::Variant1
    }
}

use crate::test_enum_ffi::FFIArrayTestEnum;

#[derive(Debug, Clone, derive::FFI)]
pub struct TestStruct {
    string: String,
    i32_collection: Vec<i32>,
    #[ffi(raw)]
    enum_variant: TestEnum,
    f64_thing: f64,
    collection_of_strings: Vec<String>,
    collection_of_ids: Vec<Uuid>,
    #[ffi(raw)]
    collection_of_variants: Vec<TestEnum>,
    collection_of_dates: Option<Vec<NaiveDateTime>>,
    collection_of_structs: Vec<TestStruct>,
}

#[test]
fn test_struct_ffi() {
    use test_struct_ffi::*;

    unsafe {
        let input_text = "some text";
        let input_i32_vec = vec![1, 2, 3, 4];
        let variant = TestEnum::Variant1;
        let double = 42.42;
        let input_string_vec = vec!["foo".to_string(), "bar".to_string()];
        let input_uuid_vec = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let input_variant_vec = vec![TestEnum::Variant1, TestEnum::Variant1, TestEnum::Variant2];
        let input_date_vec = Some(vec![
            NaiveDateTime::from_timestamp(1599868112, 0),
            NaiveDateTime::from_timestamp(653010512, 0),
        ]);

        // Convert to the FFI types that the FFI consumer will be passing in.
        let ffi_string = ffi_string!(input_text);
        let ffi_i32_array = ffi_array_i32_init(
            input_i32_vec.as_ptr(),
            input_i32_vec.len().try_into().unwrap(),
        );
        let ffi_string_array = FFIArrayString::from(&*input_string_vec);
        let ffi_uuid_array = FFIArrayString::from(&*input_uuid_vec);
        let ffi_variant_array = test_enum_ffi::ffi_array_TestEnum_init(
            input_variant_vec.as_ptr(),
            input_variant_vec.len().try_into().unwrap(),
        );
        let ffi_date_array = FFIArrayTimeStamp::from(input_date_vec.as_deref());

        // Initialize the test instance through the ffi init, getting back a pointer to it.
        let test_struct = test_struct_ffi::test_struct_rust_ffi_init(
            ffi_string,
            ffi_i32_array,
            variant,
            double,
            ffi_string_array,
            ffi_uuid_array,
            ffi_variant_array,
            ffi_date_array,
            ffi_array_TestStruct_init(std::ptr::null(), 0),
        );

        // Read and check every field.
        assert_eq!(
            input_text,
            CStr::from_ptr(get_test_struct_string(test_struct)).to_string_lossy()
        );
        assert_eq!(
            input_i32_vec,
            Vec::from(get_test_struct_i32_collection(test_struct))
        );
        assert_eq!(variant, get_test_struct_enum_variant(test_struct));
        approx::assert_relative_eq!(double, get_test_struct_f64_thing(test_struct));
        assert_eq!(
            input_string_vec,
            Vec::<String>::from(get_test_struct_collection_of_strings(test_struct))
        );
        assert_eq!(
            input_uuid_vec,
            Vec::from(get_test_struct_collection_of_ids(test_struct))
        );
        assert_eq!(
            input_variant_vec,
            Vec::from(get_test_struct_collection_of_variants(test_struct))
        );
        assert_eq!(
            input_date_vec,
            Some(Vec::from(get_optional_test_struct_collection_of_dates(
                test_struct
            )))
        );

        // Free the thing.
        test_struct_rust_ffi_free(test_struct);
    }
}
