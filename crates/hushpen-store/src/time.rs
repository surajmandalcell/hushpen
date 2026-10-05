//! Wall-clock helpers. ISO 8601 UTC without a date-time dependency.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// `2026-10-05T12:00:00Z`
pub fn iso_seconds(unix_ms: u64) -> String {
    let (date, seconds_of_day, _) = split(unix_ms);
    format!("{date}T{}Z", clock(seconds_of_day))
}

/// `2026-10-05T12:00:00.123Z`
pub fn iso_millis(unix_ms: u64) -> String {
    let (date, seconds_of_day, millis) = split(unix_ms);
    format!("{date}T{}.{millis:03}Z", clock(seconds_of_day))
}

fn clock(seconds_of_day: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds_of_day / 3600,
        seconds_of_day % 3600 / 60,
        seconds_of_day % 60
    )
}

fn split(unix_ms: u64) -> (String, u64, u64) {
    let days = (unix_ms / 86_400_000) as i64;
    let seconds_of_day = unix_ms / 1000 % 86_400;
    let (year, month, day) = civil_from_days(days);
    (
        format!("{year:04}-{month:02}-{day:02}"),
        seconds_of_day,
        unix_ms % 1000,
    )
}

/// Howard Hinnant's days-to-civil algorithm, proleptic Gregorian.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch() {
        assert_eq!(iso_seconds(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn a_known_instant_with_milliseconds() {
        // 2026-10-05T12:34:56.789Z
        assert_eq!(iso_millis(1_791_203_696_789), "2026-10-05T12:34:56.789Z");
    }

    #[test]
    fn leap_day() {
        // 2024-02-29T23:59:59Z
        assert_eq!(iso_seconds(1_709_251_199_000), "2024-02-29T23:59:59Z");
    }
}
