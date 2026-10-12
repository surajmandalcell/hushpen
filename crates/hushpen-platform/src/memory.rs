//! How much memory the machine has, to pick the AI cleanup model.

/// Total physical memory in bytes, or `None` when the system does not say.
pub fn total_ram_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        Some(objc2_foundation::NSProcessInfo::processInfo().physicalMemory())
    }
    #[cfg(not(target_os = "macos"))]
    {
        parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
    }
}

/// The `MemTotal` line of `/proc/meminfo`, which counts kibibytes.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kib: u64 = line
        .trim_start_matches("MemTotal:")
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    kib.checked_mul(1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_total_is_read_in_bytes() {
        let text = "MemTotal:       16384000 kB\nMemFree:         1000 kB\n";
        assert_eq!(parse_meminfo(text), Some(16_384_000 * 1024));
    }

    #[test]
    fn a_file_with_no_mem_total_gives_none() {
        assert_eq!(parse_meminfo("MemFree: 5 kB\n"), None);
        assert_eq!(parse_meminfo("MemTotal: lots kB\n"), None);
        assert_eq!(parse_meminfo(""), None);
    }

    #[test]
    fn this_machine_reports_some_memory() {
        assert!(total_ram_bytes().is_some_and(|bytes| bytes > 256 << 20));
    }
}
