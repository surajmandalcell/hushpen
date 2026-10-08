//! The one place that names the project on the web. About and the updater feed
//! build on these; nothing else in the app hard-codes a GitHub URL.

/// The repository. External links open only this URL and pages below it.
pub const REPO_URL: &str = "https://github.com/surajmandalcell/asrpro";

/// Where "Report issue" goes.
pub const ISSUES_URL: &str = "https://github.com/surajmandalcell/asrpro/issues";

/// True when `url` is the repository or a page below it. About refuses to open
/// anything else.
pub fn is_repo_page(url: &str) -> bool {
    url == REPO_URL || url.starts_with(&format!("{REPO_URL}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repo_url_is_the_one_constant() {
        assert_eq!(REPO_URL, "https://github.com/surajmandalcell/asrpro");
        assert_eq!(ISSUES_URL, format!("{REPO_URL}/issues"));
    }

    #[test]
    fn repo_pages_are_the_repo_and_below_it() {
        assert!(is_repo_page(REPO_URL));
        assert!(is_repo_page(ISSUES_URL));
        assert!(is_repo_page(&format!("{REPO_URL}/releases")));
        assert!(!is_repo_page("https://github.com/surajmandalcell"));
        assert!(!is_repo_page(
            "https://github.com/surajmandalcell/asrpro.evil"
        ));
        assert!(!is_repo_page("http://github.com/surajmandalcell/asrpro"));
    }
}
