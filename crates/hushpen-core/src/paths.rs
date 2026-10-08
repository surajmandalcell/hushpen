//! How a path is shown to the user: under the home folder as `~/...`, outside
//! it in full. The views truncate what is still too long.

use std::path::Path;

/// The path as the user reads it. A path directly under home becomes
/// `~/name`; home itself becomes `~`. Anything else is shown in full.
pub fn display(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home
        && let Ok(rest) = path.strip_prefix(home)
    {
        return match rest.as_os_str().is_empty() {
            true => "~".to_owned(),
            false => format!("~/{}", rest.display()),
        };
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/ada")
    }

    #[test]
    fn a_path_under_home_shows_with_a_tilde() {
        let path = home().join("hp-data");
        assert_eq!(display(&path, Some(&home())), "~/hp-data");
        let deep = home().join("one/two/three");
        assert_eq!(display(&deep, Some(&home())), "~/one/two/three");
    }

    #[test]
    fn home_itself_is_the_tilde() {
        assert_eq!(display(&home(), Some(&home())), "~");
    }

    #[test]
    fn a_path_outside_home_shows_in_full() {
        let path = PathBuf::from("/data");
        assert_eq!(display(&path, Some(&home())), "/data");
        // A sibling that shares a prefix is not under home.
        let sibling = PathBuf::from("/home/ada-other/x");
        assert_eq!(display(&sibling, Some(&home())), "/home/ada-other/x");
    }

    #[test]
    fn without_a_known_home_everything_shows_in_full() {
        let path = home().join("hp-data");
        assert_eq!(display(&path, None), "/home/ada/hp-data");
    }

    #[test]
    fn a_relative_home_does_not_swallow_absolute_paths() {
        let path = PathBuf::from("data");
        assert_eq!(display(&path, Some(&home())), "data");
    }
}
