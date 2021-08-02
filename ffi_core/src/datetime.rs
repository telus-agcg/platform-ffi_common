//!
//! FFI support for exposing time stamps.
//!

use crate::declare_opaque_type_ffi;
use chrono::NaiveDateTime;

/// Represents a UTC timestamp in a way that's safe to transfer across the FFI boundary.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeStamp {
    /// Seconds since the UNIX epoch time (January 1, 1970).
    pub secs: i64,
    /// Nanoseconds since the last whole second.
    pub nsecs: u32,
}

declare_opaque_type_ffi!(TimeStamp);

/// Initialize a Rust `chrono::NaiveDateTime` and return a raw pointer to it.
///
#[must_use]
#[allow(clippy::similar_names)]
#[no_mangle]
pub extern "C" fn time_stamp_init(secs: i64, nsecs: u32) -> *const TimeStamp {
    Box::into_raw(Box::new(TimeStamp { secs, nsecs }))
}

/// Retrieve the components of a `NaiveDateTime` as a `TimeStamp`.
///
#[must_use]
#[allow(clippy::missing_const_for_fn)]
#[no_mangle]
pub extern "C" fn get_time_stamp_secs(ptr: *const TimeStamp) -> i64 {
    let data = unsafe { &*ptr };
    data.secs
}

/// Retrieve the components of a `NaiveDateTime` as a `TimeStamp`.
///
#[must_use]
#[allow(clippy::missing_const_for_fn)]
#[no_mangle]
pub extern "C" fn get_time_stamp_nsecs(ptr: *const TimeStamp) -> u32 {
    let data = unsafe { &*ptr };
    data.nsecs
}

/// Return a `TimeStamp` to Rust to free.
///
#[no_mangle]
pub extern "C" fn time_stamp_free(ptr: *mut TimeStamp) {
    if !ptr.is_null() {
        let _ = unsafe { Box::from_raw(ptr) };
    }
}

// Conversion impls (we need to do some of these manually to convert `NaiveDateTime` to the FFI-safe
// `TimeStamp`, which can then be wrapped in the derived FFI types).

impl From<&NaiveDateTime> for TimeStamp {
    fn from(datetime: &NaiveDateTime) -> Self {
        Self {
            secs: datetime.timestamp(),
            nsecs: datetime.timestamp_subsec_nanos(),
        }
    }
}

impl From<&TimeStamp> for NaiveDateTime {
    fn from(timestamp: &TimeStamp) -> Self {
        Self::from_timestamp(timestamp.secs, timestamp.nsecs)
    }
}

// Collection conversion impls
impl From<&[NaiveDateTime]> for FFIArrayTimeStamp {
    fn from(slice: &[NaiveDateTime]) -> Self {
        let timestamps: Vec<TimeStamp> = slice.iter().map(|e| e.into()).collect();
        timestamps.as_slice().into()
    }
}

#[allow(clippy::use_self)]
impl From<FFIArrayTimeStamp> for Vec<NaiveDateTime> {
    fn from(array: FFIArrayTimeStamp) -> Self {
        let timestamps = Vec::<TimeStamp>::from(array);
        timestamps.iter().map(NaiveDateTime::from).collect()
    }
}

impl From<Option<&[NaiveDateTime]>> for FFIArrayTimeStamp {
    fn from(slice: Option<&[NaiveDateTime]>) -> Self {
        slice.map_or(
            Self {
                ptr: std::ptr::null(),
                len: 0,
                cap: 0,
            },
            |s| s.into(),
        )
    }
}

impl From<FFIArrayTimeStamp> for Option<Vec<NaiveDateTime>> {
    fn from(array: FFIArrayTimeStamp) -> Self {
        if array.ptr.is_null() {
            None
        } else {
            Some(Vec::from(array))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naive_date_time_to_time_stamp() {
        let secs: i64 = 1_599_868_112;
        let nsecs: u32 = 1_599_868;
        let datetime = NaiveDateTime::from_timestamp(secs, nsecs);
        let timestamp = TimeStamp::from(&datetime);
        assert_eq!(timestamp.secs, secs);
        assert_eq!(timestamp.nsecs, nsecs);
    }

    #[test]
    fn time_stamp_to_naive_date_time_() {
        let secs: i64 = 1_599_868_112;
        let nsecs: u32 = 1_599_868;
        let timestamp = TimeStamp { secs, nsecs };
        let datetime = NaiveDateTime::from(&timestamp);
        assert_eq!(datetime.timestamp(), secs);
        assert_eq!(datetime.timestamp_subsec_nanos(), nsecs);
    }

    #[test]
    fn naive_date_time_vec_to_time_stamp_array_and_back() {
        let date1 = NaiveDateTime::from_timestamp(1_599_868_112, 0);
        let date2 = NaiveDateTime::from_timestamp(653_010_512, 0);
        let input_date_vec = vec![date1, date2];
        let time_stamp_array = FFIArrayTimeStamp::from(&*input_date_vec);
        let date_vec_again = Vec::<NaiveDateTime>::from(time_stamp_array);
        assert_eq!(input_date_vec, date_vec_again);
    }
}
