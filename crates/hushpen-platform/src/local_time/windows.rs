use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows::Win32::System::Time::{
    DYNAMIC_TIME_ZONE_INFORMATION, FileTimeToSystemTime, GetDynamicTimeZoneInformation,
    GetTimeZoneInformationForYear, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
    TIME_ZONE_INFORMATION,
};

fn ticks(unix_seconds: i64) -> Option<u64> {
    u64::try_from(unix_seconds.checked_add(11_644_473_600)?)
        .ok()?
        .checked_mul(10_000_000)
}

fn value(time: FILETIME) -> i64 {
    ((u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)) as i64
}

pub(super) fn offset(unix_seconds: i64) -> Option<i64> {
    let ticks = ticks(unix_seconds)?;
    let utc_file = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    let mut local_file = FILETIME::default();
    let mut dynamic = DYNAMIC_TIME_ZONE_INFORMATION::default();
    let mut zone = TIME_ZONE_INFORMATION::default();
    // SAFETY: all pointers refer to initialized, live structures of the documented sizes.
    unsafe {
        FileTimeToSystemTime(&utc_file, &mut utc).ok()?;
        if GetDynamicTimeZoneInformation(&mut dynamic) == u32::MAX {
            return None;
        }
        GetTimeZoneInformationForYear(utc.wYear, Some(&dynamic), &mut zone).ok()?;
        SystemTimeToTzSpecificLocalTime(Some(&zone), &utc, &mut local).ok()?;
        SystemTimeToFileTime(&local, &mut local_file).ok()?;
    }
    Some((value(local_file) - value(utc_file)) / 10_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_epoch_and_invalid_dates_are_checked() {
        assert_eq!(ticks(0), Some(116_444_736_000_000_000));
        assert_eq!(ticks(-11_644_473_600), Some(0));
        assert_eq!(ticks(-11_644_473_601), None);
        assert_eq!(ticks(i64::MAX), None);
    }
}
