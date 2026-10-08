//! Pure Hushpen logic. No IO, no native libraries.

pub mod catalog;
pub mod cleanup;
pub mod dictation;
pub mod error;
pub mod insert;
pub mod language;
pub mod permission;
pub mod protocol;
pub mod threads;
pub mod transcript;

/// Product version, shared by the app, the children, and the packagers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version shown to users and printed by `--version`. Debug builds add `-dev`;
/// release candidates set `HUSHPEN_BUILD_VERSION` at compile time.
pub const BUILD_VERSION: &str = match option_env!("HUSHPEN_BUILD_VERSION") {
    Some(version) => version,
    None if cfg!(debug_assertions) => concat!(env!("CARGO_PKG_VERSION"), "-dev"),
    None => VERSION,
};

#[cfg(test)]
mod tests {
    use super::{BUILD_VERSION, VERSION};

    #[test]
    fn version_is_the_release_version() {
        assert_eq!(VERSION, "2.0.0");
    }

    #[test]
    fn debug_builds_print_the_dev_version() {
        if option_env!("HUSHPEN_BUILD_VERSION").is_none() {
            let expected = if cfg!(debug_assertions) {
                "2.0.0-dev"
            } else {
                "2.0.0"
            };
            assert_eq!(BUILD_VERSION, expected);
        }
    }
}
