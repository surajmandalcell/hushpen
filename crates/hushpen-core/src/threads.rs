//! How many threads the speech engine uses.

use serde_json::Value;

const AUTO_MIN: usize = 2;
const AUTO_MAX: usize = 8;

/// `auto`: leave one CPU to the app, never fewer than 2 or more than 8 threads.
pub fn auto_threads(cpus: usize) -> usize {
    cpus.saturating_sub(1).clamp(AUTO_MIN, AUTO_MAX)
}

/// Reads the `engine.threads` setting: the text `auto` or a whole number. Anything else counts
/// as `auto`, because the settings store already refuses bad values.
pub fn resolve_threads(setting: Option<&Value>, cpus: usize) -> usize {
    match setting.and_then(Value::as_u64) {
        Some(count) if count >= 1 => usize::try_from(count).unwrap_or(AUTO_MAX),
        _ => auto_threads(cpus),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn auto_keeps_one_cpu_free_within_2_and_8() {
        assert_eq!(auto_threads(1), 2);
        assert_eq!(auto_threads(2), 2);
        assert_eq!(auto_threads(4), 3);
        assert_eq!(auto_threads(9), 8);
        assert_eq!(auto_threads(64), 8);
    }

    #[test]
    fn a_number_wins_over_auto() {
        assert_eq!(resolve_threads(Some(&json!(3)), 2), 3);
        assert_eq!(resolve_threads(Some(&json!("auto")), 2), 2);
        assert_eq!(resolve_threads(None, 6), 5);
        assert_eq!(resolve_threads(Some(&json!(0)), 6), 5);
    }
}
