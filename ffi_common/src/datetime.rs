//!
//! FFI support for exposing time stamps.
//!

use crate::{declare_value_type_array_struct, error};
use chrono::NaiveDateTime;
use paste::paste;

/// Represents a UTC timestamp in a way that's safe to transfer across the FFI boundary.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TimeStamp {
    /// Seconds since the UNIX epoch time (January 1, 1970).
    secs: i64,
    /// Nanoseconds since the last whole second.
    nsecs: u32,
}

impl Default for TimeStamp {
    fn default() -> Self {
        Self { secs: 0, nsecs: 0 }
    }
}

declare_value_type_array_struct!(TimeStamp);

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

impl From<TimeStamp> for NaiveDateTime {
    fn from(timestamp: TimeStamp) -> Self {
        Self::from_timestamp(timestamp.secs, timestamp.nsecs)
    }
}

impl From<&TimeStamp> for NaiveDateTime {
    fn from(timestamp: &TimeStamp) -> Self {
        Self::from_timestamp(timestamp.secs, timestamp.nsecs)
    }
}

// Option conversion impls
impl From<&Option<NaiveDateTime>> for OptionTimeStamp {
    fn from(opt: &Option<NaiveDateTime>) -> Self {
        let d = opt.map(|s| TimeStamp {
            secs: s.timestamp(),
            nsecs: s.timestamp_subsec_nanos(),
        });
        (&d).into()
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
impl From<&Vec<NaiveDateTime>> for FFIArrayTimeStamp {
    fn from(v: &Vec<NaiveDateTime>) -> Self {
        let timestamps: Vec<TimeStamp> = v.iter().map(|e| e.into()).collect();
        (&timestamps).into()
    }
}

#[allow(clippy::use_self)]
impl From<FFIArrayTimeStamp> for Vec<NaiveDateTime> {
    fn from(array: FFIArrayTimeStamp) -> Self {
        unsafe {
            let v = Vec::from_raw_parts(array.ptr as *mut TimeStamp, array.len, array.cap);
            v.iter().map(|e| e.into()).collect()
        }
    }
}
