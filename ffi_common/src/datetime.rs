//!
//! FFI support for exposing time stamps.
//!

use crate::declare_value_type_ffi;
use chrono::NaiveDateTime;
use paste::paste;

/// Represents a UTC timestamp in a way that's safe to transfer across the FFI boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeStamp {
    /// Seconds since the UNIX epoch time (January 1, 1970).
    secs: i64,
    /// Nanoseconds since the last whole second.
    nsecs: u32,
}

declare_value_type_ffi!(TimeStamp);

// Conversion impls (we need to do some of these manually to convert `NaiveDateTime` to the FFI-safe
// `TimeStamp`, which can then be wrapped in the derived `OptionTimeStamp` and `FFIArrayTimeStamp`).

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

// Option conversion impls
impl From<Option<&NaiveDateTime>> for OptionTimeStamp {
    fn from(opt: Option<&NaiveDateTime>) -> Self {
        opt.map(|s| TimeStamp {
            secs: s.timestamp(),
            nsecs: s.timestamp_subsec_nanos(),
        })
        .as_ref()
        .into()
    }
}

impl From<OptionTimeStamp> for Option<NaiveDateTime> {
    fn from(opt: OptionTimeStamp) -> Self {
        if opt.has_value {
            Some(NaiveDateTime::from_timestamp(
                opt.value.secs,
                opt.value.nsecs,
            ))
        } else {
            None
        }
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
        unsafe {
            Vec::from_raw_parts(array.ptr as *mut TimeStamp, array.len, array.cap)
                .iter()
                .map(|e| e.into())
                .collect()
        }
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
