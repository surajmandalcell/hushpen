//! Where the sherpa-onnx library is. The package puts it beside the worker on Linux and in the
//! bundle's `Contents/Frameworks` on macOS, next to the ONNX Runtime library it links.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub const LIBRARY: &str = "libsherpa-onnx-c-api.dylib";
#[cfg(target_os = "windows")]
pub const LIBRARY: &str = "sherpa-onnx-c-api.dll";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LIBRARY: &str = "libsherpa-onnx-c-api.so";

/// Debug builds look in this folder first, so a test can use a downloaded copy.
pub const DIR_ENV: &str = "HUSHPEN_TTS_RUNTIME_DIR";

/// The library the worker opens: the first candidate that exists, else the last one (the
/// load error then names the place the package should have filled).
pub fn library_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hushpen-tts"));
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let dir = exe.parent().unwrap_or(Path::new("."));
    let override_dir = if cfg!(debug_assertions) {
        std::env::var_os(DIR_ENV)
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
    } else {
        None
    };
    let candidates = candidates(dir, override_dir);
    candidates
        .iter()
        .find(|path| path.is_file())
        .or(candidates.last())
        .cloned()
        .unwrap_or_else(|| PathBuf::from(LIBRARY))
}

fn candidates(exe_dir: &Path, override_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = override_dir
        .map(|dir| dir.join(LIBRARY))
        .into_iter()
        .collect();
    if cfg!(target_os = "macos") {
        paths.push(exe_dir.join("../Frameworks").join(LIBRARY));
    }
    paths.push(exe_dir.join(LIBRARY));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_comes_first_and_the_worker_folder_last() {
        let paths = candidates(Path::new("/app/bin"), Some(PathBuf::from("/tmp/rt")));
        assert_eq!(paths.first(), Some(&Path::new("/tmp/rt").join(LIBRARY)));
        assert_eq!(paths.last(), Some(&Path::new("/app/bin").join(LIBRARY)));
        let bundle = Path::new("/app/bin/../Frameworks").join(LIBRARY);
        assert_eq!(paths.contains(&bundle), cfg!(target_os = "macos"));
    }
}
