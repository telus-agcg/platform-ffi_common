use ffi_common::derive::FFI;

#[derive(Debug, Clone, FFI)]
pub struct NumericData {
    data: u8,
}

#[derive(Debug, Clone, FFI)]
pub struct StringData {
    data: String,
}

#[derive(Debug, Clone, FFI)]
pub enum HasData {
    NumericData(NumericData),
    StringData(StringData),
    NoData,
}

#[derive(Debug, Clone, FFI)]
pub struct ContainsEnum {
    enum_data: HasData,
}

#[test]
fn test_associated_numeric() {
    unsafe {
        let data = numeric_data_ffi::numeric_data_rust_ffi_init(42) as *mut NumericData;
        let polymorphic = has_data_ffi::has_data_numeric_data_rust_ffi_init(data);
        let polymorphic_variant = has_data_ffi::get_has_data_variant(polymorphic);
        assert_eq!(polymorphic_variant, has_data_ffi::HasDataType::NumericData);
        let polymorphic_data = has_data_ffi::get_has_data_numeric_data_unnamed_field_0(polymorphic);
        assert_eq!(numeric_data_ffi::get_numeric_data_data(polymorphic_data), 42);
        has_data_ffi::rust_ffi_free_has_data(polymorphic);
    }
}

#[test]
fn test_associated_string() {
    use ffi_common::core::{ffi_string, string::string_from_c};
    unsafe {
        let test_string = "some test data";
        let string_data = string_data_ffi::string_data_rust_ffi_init(ffi_string!(test_string)) as *mut StringData;
        let polymorphic = has_data_ffi::has_data_string_data_rust_ffi_init(string_data);
        let polymorphic_variant = has_data_ffi::get_has_data_variant(polymorphic);
        assert_eq!(polymorphic_variant, has_data_ffi::HasDataType::StringData);

        let polymorphic_data = has_data_ffi::get_has_data_string_data_unnamed_field_0(polymorphic);
        assert_eq!(
            string_from_c(string_data_ffi::get_string_data_data(polymorphic_data)),
            test_string);
        has_data_ffi::rust_ffi_free_has_data(polymorphic);
    }
}

#[test]
fn test_nothing_associated() {
    unsafe {
        let polymorphic = has_data_ffi::has_data_no_data_rust_ffi_init();
        let polymorphic_variant = has_data_ffi::get_has_data_variant(polymorphic);
        assert_eq!(polymorphic_variant, has_data_ffi::HasDataType::NoData);
        has_data_ffi::rust_ffi_free_has_data(polymorphic);
    }
}

#[test]
fn test_struct_containing_associated_string() {
    use ffi_common::core::{ffi_string, string::string_from_c};
    unsafe {
        let test_string = "some test data";
        let string_data = string_data_ffi::string_data_rust_ffi_init(ffi_string!(test_string)) as *mut StringData;
        let polymorphic = has_data_ffi::has_data_string_data_rust_ffi_init(string_data);

        // Need to pass a mut reference here because the initializer takes ownership of this data.
        // Don't free `polymorphic` after this.
        let contains_enum = contains_enum_ffi::contains_enum_rust_ffi_init(polymorphic as *mut HasData);
        let polymorphic_from_struct = contains_enum_ffi::get_contains_enum_enum_data(contains_enum);

        // Safe to free `contains_enum` now; `polymorphic_from_struct` has the cloned data.
        contains_enum_ffi::contains_enum_rust_ffi_free(contains_enum);

        let polymorphic_variant = has_data_ffi::get_has_data_variant(polymorphic_from_struct);
        assert_eq!(polymorphic_variant, has_data_ffi::HasDataType::StringData);

        let polymorphic_data = has_data_ffi::get_has_data_string_data_unnamed_field_0(polymorphic_from_struct);
        // Safe to free `polymorphic_from_struct`, then access `polymorphic_data`.
        has_data_ffi::rust_ffi_free_has_data(polymorphic_from_struct);
        assert_eq!(
            string_from_c(string_data_ffi::get_string_data_data(polymorphic_data)),
            test_string);
    }
}