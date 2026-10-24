//! The project pages About opens, each named once. The update feed keeps its own address in the
//! app's `net/update.rs`.

/// The project website.
pub const SITE_URL: &str = "https://asrpro.mandalsuraj.com/";

/// The repository.
pub const REPO_URL: &str = "https://github.com/surajmandalcell/hushpen";

/// Where "Report an issue" goes.
pub const ISSUES_URL: &str = "https://github.com/surajmandalcell/hushpen/issues";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_urls_are_these_constants() {
        assert_eq!(SITE_URL, "https://asrpro.mandalsuraj.com/");
        assert_eq!(REPO_URL, "https://github.com/surajmandalcell/hushpen");
        assert_eq!(ISSUES_URL, format!("{REPO_URL}/issues"));
    }
}
