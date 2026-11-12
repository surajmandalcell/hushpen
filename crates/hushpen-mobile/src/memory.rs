#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
pub(crate) fn available() -> Option<u64> {
    unsafe extern "C" {
        fn os_proc_available_memory() -> usize;
    }
    // SAFETY: This iOS API takes no pointers and returns the calling app's current memory limit.
    Some(unsafe { os_proc_available_memory() } as u64)
}

#[cfg(target_os = "android")]
pub(crate) fn available() -> Option<u64> {
    Some(
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .as_deref()
            .and_then(mem_available)
            .unwrap_or(0),
    )
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub(crate) fn available() -> Option<u64> {
    None
}

#[cfg(any(target_os = "android", test))]
fn mem_available(text: &str) -> Option<u64> {
    let mut fields = text
        .lines()
        .find_map(|line| line.strip_prefix("MemAvailable:"))?
        .split_whitespace();
    let bytes = fields.next()?.parse::<u64>().ok()?.checked_mul(1024)?;
    (fields.next() == Some("kB") && fields.next().is_none()).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_memory_uses_available_bytes_and_rejects_unknown_or_bad_values() {
        assert_eq!(
            mem_available("MemTotal: 999 kB\nMemAvailable: 123 kB\n"),
            Some(123 * 1024)
        );
        for text in [
            "MemTotal: 999 kB",
            "MemAvailable: 10 MB",
            "MemAvailable: -1 kB",
            "MemAvailable: 18446744073709551615 kB",
        ] {
            assert_eq!(mem_available(text), None);
        }
    }
}
