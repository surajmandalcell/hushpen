//! Where the app finds the worker binaries it starts.

use std::path::PathBuf;

/// A worker binary such as `hushpen-llm`, beside the app's resolved executable. Debug builds
/// also honor the `debug_override` variable, so a test build can point at a freshly built
/// worker.
pub(crate) fn worker_path(name: &str, debug_override: &str) -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os(debug_override).filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    #[cfg(not(debug_assertions))]
    let _ = debug_override;
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hushpen"));
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
