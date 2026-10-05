//! Finds the repo root while xtask runs. The path is never baked in at
//! compile time: a shared `CARGO_TARGET_DIR` keeps one xtask binary alive
//! across checkouts, and a baked-in path would point at a removed one.

use std::path::{Path, PathBuf};

/// Walks up from `start` to the first folder whose `Cargo.toml` declares a
/// `[workspace]`.
pub fn find_from(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|dir| {
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
        manifest
            .lines()
            .any(|line| line.trim() == "[workspace]")
            .then(|| dir.to_path_buf())
    })
}

pub fn repo_root() -> Result<PathBuf, String> {
    let here = std::env::current_dir()
        .map_err(|error| format!("could not read the working folder: {error}"))?;
    find_from(&here).ok_or_else(|| {
        format!(
            "no workspace Cargo.toml above {}; run xtask inside the repo",
            here.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-root-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_the_workspace_from_a_nested_member_folder() {
        let root = scratch("nested");
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"a\"]\n").unwrap();
        let deep = root.join("crates/a/src");
        fs::create_dir_all(&deep).unwrap();
        fs::write(
            root.join("crates/a/Cargo.toml"),
            "[package]\nname = \"a\"\n",
        )
        .unwrap();
        assert_eq!(find_from(&deep), Some(root.clone()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn skips_a_member_manifest_without_a_workspace_table() {
        let root = scratch("member");
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        let member = root.join("m");
        fs::create_dir_all(&member).unwrap();
        fs::write(member.join("Cargo.toml"), "[package]\nname = \"m\"\n").unwrap();
        assert_eq!(find_from(&member), Some(root.clone()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_nothing_outside_a_workspace() {
        let root = scratch("none");
        assert_eq!(find_from(&root), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_running_checkout_is_found_without_a_compile_time_path() {
        let found = repo_root().unwrap();
        assert!(found.join("xtask/Cargo.toml").is_file());
    }
}
