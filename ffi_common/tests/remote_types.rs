use chrono::{DateTime, Utc};
use ffi_common::core::ffi_string;
use ffi_derive::FFI;
use std::str::FromStr;

#[derive(Debug, Clone, FFI)]
pub struct DateTimeWrapper {
    pub datetime: String,
}

impl From<DateTime<Utc>> for DateTimeWrapper {
    fn from(datetime: DateTime<Utc>) -> Self {
        Self {
            datetime: datetime.to_string(),
        }
    }
}

impl From<DateTimeWrapper> for DateTime<Utc> {
    fn from(wrapper: DateTimeWrapper) -> Self {
        DateTime::<Utc>::from_str(&wrapper.datetime).unwrap()
    }
}

#[derive(Debug, Clone, FFI)]
pub struct StructWithRemoteTypeFields {
    pub primitive: u8,
    #[ffi(expose_as = "DateTimeWrapper")]
    pub remote: DateTime<Utc>,
}

#[test]
fn test_remote_wrapper() {
    unsafe {
        let wrapper = date_time_wrapper_ffi::date_time_wrapper_rust_ffi_init(ffi_string!(
            "2020-10-27T16:30:52Z"
        ));
        // `wrapper` is consumed here; note that even with wrapped remote types, arguments passed to an
        // FFI initializer are considered `move`s.
        let struct_with_remote =
            struct_with_remote_type_fields_ffi::struct_with_remote_type_fields_rust_ffi_init(
                42,
                wrapper as *mut DateTimeWrapper,
            );
        assert_eq!(
            (*struct_with_remote).remote.to_string(),
            "2020-10-27 16:30:52 UTC"
        );
        struct_with_remote_type_fields_ffi::struct_with_remote_type_fields_rust_ffi_free(
            struct_with_remote,
        );
    }
}
