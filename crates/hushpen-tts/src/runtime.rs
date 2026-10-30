//! Where the native libraries are. The package puts sherpa-onnx and ONNX Runtime beside the
//! worker on Linux and in the bundle's `Contents/Frameworks` on macOS. sherpa-onnx links ONNX
//! Runtime; the Chatterbox backend opens the same ONNX Runtime library itself.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub const LIBRARY: &str = "libsherpa-onnx-c-api.dylib";
#[cfg(target_os = "windows")]
pub const LIBRARY: &str = "sherpa-onnx-c-api.dll";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LIBRARY: &str = "libsherpa-onnx-c-api.so";

#[cfg(target_os = "macos")]
pub const ONNXRUNTIME: &str = "libonnxruntime.dylib";
#[cfg(target_os = "windows")]
pub const ONNXRUNTIME: &str = "onnxruntime.dll";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const ONNXRUNTIME: &str = "libonnxruntime.so";

/// Debug builds look in this folder first, so a test can use a downloaded copy.
pub const DIR_ENV: &str = "HUSHPEN_TTS_RUNTIME_DIR";

/// The sherpa-onnx library the worker opens.
pub fn library_path() -> PathBuf {
    path_of(LIBRARY)
}

/// The ONNX Runtime library the Chatterbox backend opens.
pub fn onnxruntime_path() -> PathBuf {
    path_of(ONNXRUNTIME)
}

/// The first candidate for `name` that exists, else the last one (the load error then names
/// the place the package should have filled).
fn path_of(name: &str) -> PathBuf {
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
    let candidates = candidates(dir, override_dir, name);
    candidates
        .iter()
        .find(|path| path.is_file())
        .or(candidates.last())
        .cloned()
        .unwrap_or_else(|| PathBuf::from(name))
}

fn candidates(exe_dir: &Path, override_dir: Option<PathBuf>, name: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = override_dir.map(|dir| dir.join(name)).into_iter().collect();
    if cfg!(target_os = "macos") {
        paths.push(exe_dir.join("../Frameworks").join(name));
    }
    paths.push(exe_dir.join(name));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_comes_first_and_the_worker_folder_last() {
        for name in [LIBRARY, ONNXRUNTIME] {
            let paths = candidates(Path::new("/app/bin"), Some(PathBuf::from("/tmp/rt")), name);
            assert_eq!(paths.first(), Some(&Path::new("/tmp/rt").join(name)));
            assert_eq!(paths.last(), Some(&Path::new("/app/bin").join(name)));
            let bundle = Path::new("/app/bin/../Frameworks").join(name);
            assert_eq!(paths.contains(&bundle), cfg!(target_os = "macos"));
        }
    }
}
