#[derive(Debug, Clone, ffi_common::derive::FFI)]
#[ffi(forbid_memberwise_init)]
pub struct SomeType {
    field: String,
}

#[ffi_common::derive::expose_impl(description("constructor"), generic(V = "String"))]
impl SomeType {
    pub fn special_init<V>(value: V) -> Self
    where
        String: From<V>,
    {
        SomeType {
            field: value.into(),
        }
    }
}

#[test]
fn test_generic() {
    unsafe {
        let cstring = ffi_common::core::ffi_string!("some test data");
        let some_type = constructor_some_type_ffi::constructor_some_type_ffi_special_init(cstring);
        assert_eq!(
            ffi_common::core::string::string_from_c(some_type_ffi::get_some_type_field(some_type)),
            "some test data"
        );
        some_type_ffi::some_type_rust_ffi_free(some_type);
    }
}
