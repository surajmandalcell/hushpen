//! The seven main views, in sidebar order.

use gpui_kit::assets::IconName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Home,
    History,
    /// Audio files to transcribe and text to read aloud, each started by hand.
    Manual,
    Dictionary,
    Models,
    Settings,
    About,
}

impl View {
    pub const ALL: [View; 7] = [
        View::Home,
        View::History,
        View::Manual,
        View::Dictionary,
        View::Models,
        View::Settings,
        View::About,
    ];

    /// Name shown in the sidebar and as the view title.
    pub fn title(self) -> &'static str {
        match self {
            View::Home => "Home",
            View::History => "History",
            View::Manual => "Manual",
            View::Dictionary => "Dictionary",
            View::Models => "Models",
            View::Settings => "Settings",
            View::About => "About",
        }
    }

    /// Lower-case name used in test hook ids and in the saved last view. Manual keeps `files`,
    /// its name before the rename, so hook scripts, pixel baselines, and saved settings still
    /// find it.
    pub fn key(self) -> &'static str {
        match self {
            View::Home => "home",
            View::History => "history",
            View::Manual => "files",
            View::Dictionary => "dictionary",
            View::Models => "models",
            View::Settings => "settings",
            View::About => "about",
        }
    }

    /// The view of a key, or of `manual`, the name Manual shows.
    pub fn from_key(key: &str) -> Option<View> {
        if key == "manual" {
            return Some(View::Manual);
        }
        Self::ALL.into_iter().find(|view| view.key() == key)
    }

    pub fn icon(self) -> IconName {
        match self {
            View::Home => IconName::House,
            View::History => IconName::Clock,
            View::Manual => IconName::Files,
            View::Dictionary => IconName::BookA,
            View::Models => IconName::Cpu,
            View::Settings => IconName::Settings,
            View::About => IconName::Info,
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|view| *view == self).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_order_is_home_to_about() {
        let titles: Vec<_> = View::ALL.iter().map(|view| view.title()).collect();
        assert_eq!(
            titles,
            [
                "Home",
                "History",
                "Manual",
                "Dictionary",
                "Models",
                "Settings",
                "About"
            ]
        );
    }

    #[test]
    fn hook_keys_are_unique_lower_case_names() {
        let mut keys: Vec<_> = View::ALL.iter().map(|view| view.key()).collect();
        for (view, key) in View::ALL.iter().zip(&keys) {
            let name = match view {
                View::Manual => "files",
                _ => view.title(),
            };
            assert_eq!(*key, name.to_lowercase());
        }
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 7);
    }

    #[test]
    fn from_key_finds_every_view_and_nothing_else() {
        for view in View::ALL {
            assert_eq!(View::from_key(view.key()), Some(view));
        }
        assert_eq!(View::from_key("manual"), Some(View::Manual));
        assert_eq!(View::from_key("Home"), None);
        assert_eq!(View::from_key(""), None);
        assert_eq!(View::from_key("nowhere"), None);
    }

    #[test]
    fn index_matches_position() {
        for (i, view) in View::ALL.iter().enumerate() {
            assert_eq!(view.index(), i);
        }
    }
}
