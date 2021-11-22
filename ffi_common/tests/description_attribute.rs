#[derive(Debug, Clone, ffi_common::derive::FFI)]
pub struct TypeWithImpl {
    field: u8,
}

#[ffi_common::derive::expose_impl(description("testing_impl"))]
impl TypeWithImpl {
    pub fn field_doubled(&self) -> u8 {
        self.field * 2
    }
}

#[test]
fn test_description() {
    unsafe {
        let type_with_impl = type_with_impl_ffi::type_with_impl_rust_ffi_init(42);
        let doubled =
            testing_impl_type_with_impl_ffi::testing_impl_type_with_impl_ffi_field_doubled(
                type_with_impl,
            );
        assert_eq!(doubled, 84);
        type_with_impl_ffi::type_with_impl_rust_ffi_free(type_with_impl);
    }
}
