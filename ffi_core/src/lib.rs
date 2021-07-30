pub use paste::paste;

#[macro_use]
pub mod error;
pub mod datetime;
#[macro_use]
pub mod macros;
pub mod string;

declare_value_type_ffi!(bool, u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_none() {
        let null_pointer = option_u8_init(false, 0);
        assert!(null_pointer.is_null());
    }

    #[test]
    fn test_ffi_some() {
        let u8_pointer = option_u8_init(true, 3);
        assert!(!u8_pointer.is_null());
        assert_eq!(unsafe { *Box::from_raw(u8_pointer as *mut u8) }, 3);
    }
}
