//! The offset of the time zone the user set in the system.

// One plain C call on memory this module owns.
#![allow(unsafe_code)]

/// Seconds to add to a Unix time to get the local wall-clock time at that moment, so daylight
/// saving of that date counts. It is 0 when the system does not say.
pub fn utc_offset_seconds(unix_seconds: i64) -> i64 {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        local::offset(unix_seconds)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = unix_seconds;
        0
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod local {
    use std::ffi::{c_char, c_int, c_long};

    /// `struct tm` with the `tm_gmtoff` and `tm_zone` members that macOS and glibc both add
    /// after the nine fields of the C standard. Both lay it out the same on 64-bit targets.
    #[repr(C)]
    struct Tm {
        sec: c_int,
        min: c_int,
        hour: c_int,
        mday: c_int,
        mon: c_int,
        year: c_int,
        wday: c_int,
        yday: c_int,
        isdst: c_int,
        gmtoff: c_long,
        zone: *const c_char,
    }

    const _: () = assert!(size_of::<c_long>() == 8 && size_of::<usize>() == 8);

    unsafe extern "C" {
        /// `time_t` is 64 bits on every target the app ships for.
        fn localtime_r(time: *const i64, out: *mut Tm) -> *mut Tm;
    }

    pub(super) fn offset(unix_seconds: i64) -> i64 {
        let mut tm = Tm {
            sec: 0,
            min: 0,
            hour: 0,
            mday: 0,
            mon: 0,
            year: 0,
            wday: 0,
            yday: 0,
            isdst: 0,
            gmtoff: 0,
            zone: std::ptr::null(),
        };
        // SAFETY: both pointers refer to live locals; `localtime_r` writes only into `tm` and
        // returns null on failure.
        let done = unsafe { localtime_r(&unix_seconds, &mut tm) };
        if done.is_null() { 0 } else { tm.gmtoff }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offset_is_a_real_time_zone_offset() {
        for moment in [0, 1_748_769_300, 1_791_203_696] {
            let offset = utc_offset_seconds(moment);
            assert!((-14 * 3600..=14 * 3600).contains(&offset), "{offset}");
            assert_eq!(offset % 900, 0, "{offset}");
        }
    }
}
