//! Where the data folder is. `HUSHPEN_DATA_DIR` wins; otherwise the per-OS
//! default (`~/Library/Application Support/Hushpen` on macOS,
//! `${XDG_DATA_HOME:-~/.local/share}/hushpen` on Linux).

use directories::BaseDirs;
use std::ffi::OsString;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    resolve(
        std::env::var_os("HUSHPEN_DATA_DIR"),
        BaseDirs::new().map(|dirs| dirs.data_dir().to_path_buf()),
    )
}

fn resolve(override_dir: Option<OsString>, data_home: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = override_dir.filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    let name = if cfg!(target_os = "macos") {
        "Hushpen"
    } else {
        "hushpen"
    };
    data_home.unwrap_or_else(|| PathBuf::from(".")).join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_wins_over_the_default() {
        let dir = resolve(Some("/data".into()), Some("/home/u/.local/share".into()));
        assert_eq!(dir, PathBuf::from("/data"));
    }

    #[test]
    fn an_empty_override_is_ignored() {
        let dir = resolve(Some("".into()), Some("/home/u/.local/share".into()));
        assert!(dir.starts_with("/home/u/.local/share"));
    }

    #[test]
    fn the_default_lives_under_the_os_data_home() {
        let dir = resolve(None, Some("/base".into()));
        let expected = if cfg!(target_os = "macos") {
            "/base/Hushpen"
        } else {
            "/base/hushpen"
        };
        assert_eq!(dir, PathBuf::from(expected));
    }
}
