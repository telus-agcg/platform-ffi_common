#[derive(Debug, Clone, ffi_common::derive::FFI)]
pub struct Foo {
    data: u8,
}

#[derive(Debug, Clone, ffi_common::derive::FFI)]
pub struct Bar {
    data: String,
}

#[derive(Debug, Clone, ffi_common::derive::FFI)]
pub enum HasStuff {
    FooStuff(Foo),
    BarStuff(Bar),
}

#[derive(Debug, Clone, ffi_common::derive::FFI)]
pub struct ContainsEnum {
    stuff: HasStuff,
}

#[test]
fn test() {
    unsafe {
        let data = foo_ffi::foo_rust_ffi_init(42) as *mut Foo;
        let polymorphic = has_stuff_ffi::has_stuff_foo_stuff_rust_ffi_init(data);
        let polymorphic_variant = has_stuff_ffi::get_has_stuff_variant(polymorphic);
        assert_eq!(polymorphic_variant, has_stuff_ffi::HasStuffType::FooStuff);
        let polymorphic_data = has_stuff_ffi::get_has_stuff_foo_stuff_unnamed_field_0(polymorphic);
        assert_eq!(foo_ffi::get_foo_data(polymorphic_data), 42);
        has_stuff_ffi::rust_ffi_free_has_stuff(polymorphic);
    }
}
