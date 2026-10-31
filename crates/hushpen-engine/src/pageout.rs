//! `madvise(MADV_PAGEOUT)` on the memory a model load mapped (see `hushpen_core::idle`).
//!
//! Only Linux has the call. Elsewhere [`track`] finds nothing and [`page_out`] advises nothing.

use hushpen_core::idle;
use std::ops::Range;
use std::path::Path;

fn mappings() -> Vec<idle::Mapping> {
    std::fs::read_to_string("/proc/self/maps")
        .map(|text| idle::parse_maps(&text))
        .unwrap_or_default()
}

/// Runs `load` and returns its value with the ranges it mapped for the model file `path`.
pub fn track<T>(path: &Path, load: impl FnOnce() -> T) -> (T, Vec<Range<usize>>) {
    if !cfg!(target_os = "linux") {
        return (load(), Vec::new());
    }
    let before = mappings();
    let value = load();
    let after = mappings();
    let file = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let len = std::fs::metadata(&file).map_or(0, |meta| meta.len());
    let ranges = idle::model_ranges(&before, &after, &file.to_string_lossy(), len);
    (value, ranges)
}

/// Asks the kernel to reclaim `ranges` now and returns the bytes advised.
#[cfg(target_os = "linux")]
pub fn page_out(ranges: &[Range<usize>]) -> std::io::Result<u64> {
    use std::ffi::{c_int, c_void};
    const MADV_PAGEOUT: c_int = 21;
    unsafe extern "C" {
        fn madvise(addr: *mut c_void, len: usize, advice: c_int) -> c_int;
    }
    let mut advised = 0;
    for range in ranges {
        // SAFETY: MADV_PAGEOUT only reclaims pages and keeps their contents (madvise(2)), and
        // `track` took each page-aligned range from this process's own mappings.
        if unsafe { madvise(range.start as *mut c_void, range.len(), MADV_PAGEOUT) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        advised += range.len() as u64;
    }
    Ok(advised)
}

#[cfg(not(target_os = "linux"))]
pub fn page_out(_ranges: &[Range<usize>]) -> std::io::Result<u64> {
    Ok(0)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn resident_kb() -> u64 {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        status
            .lines()
            .find_map(|line| line.strip_prefix("RssAnon:"))
            .and_then(|rest| rest.trim().trim_end_matches(" kB").trim().parse().ok())
            .unwrap()
    }

    #[test]
    fn a_large_buffer_is_found_and_its_pages_out_keep_their_contents() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let len = 64 << 20;
        file.as_file().set_len(len as u64).unwrap();
        let (buffer, ranges) = track(file.path(), || {
            let mut buffer = vec![0_u8; len];
            for (index, byte) in buffer.iter_mut().enumerate() {
                *byte = (index % 251) as u8;
            }
            buffer
        });
        assert!(!ranges.is_empty(), "the 64 MiB buffer is a new mapping");
        let start = buffer.as_ptr() as usize;
        // Other tests run in parallel and may map memory too, so look for the range that holds it.
        assert!(
            ranges
                .iter()
                .any(|r| r.start <= start && r.end >= start + len)
        );
        let before = resident_kb();
        let advised = page_out(&ranges).expect("MADV_PAGEOUT needs Linux 5.4");
        assert!(advised >= (len as u64) / 2);
        let after = resident_kb();
        eprintln!("RssAnon {before} kB -> {after} kB after paging out {advised} bytes");
        assert!(
            buffer
                .iter()
                .enumerate()
                .all(|(i, b)| *b == (i % 251) as u8)
        );
    }
}
