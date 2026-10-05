//! Pure Hushpen logic. No IO, no native libraries.

/// Product version, shared by the app, the children, and the packagers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_the_release_version() {
        assert_eq!(VERSION, "2.0.0");
    }
}
