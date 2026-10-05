//! The seven main views, in sidebar order.

use crate::theme::color;
use gpui_kit::assets::IconName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Home,
    History,
    Dictionary,
    Import,
    Models,
    Settings,
    About,
}

impl View {
    pub const ALL: [View; 7] = [
        View::Home,
        View::History,
        View::Dictionary,
        View::Import,
        View::Models,
        View::Settings,
        View::About,
    ];

    /// Name shown in the sidebar and as the view title.
    pub fn title(self) -> &'static str {
        match self {
            View::Home => "Home",
            View::History => "History",
            View::Dictionary => "Dictionary",
            View::Import => "Import",
            View::Models => "Models",
            View::Settings => "Settings",
            View::About => "About",
        }
    }

    /// Lower-case name used in test hook ids.
    pub fn key(self) -> &'static str {
        match self {
            View::Home => "home",
            View::History => "history",
            View::Dictionary => "dictionary",
            View::Import => "import",
            View::Models => "models",
            View::Settings => "settings",
            View::About => "about",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            View::Home => IconName::House,
            View::History => IconName::Clock,
            View::Dictionary => IconName::BookA,
            View::Import => IconName::Import,
            View::Models => IconName::Cpu,
            View::Settings => IconName::Settings,
            View::About => IconName::Info,
        }
    }

    /// Icon tile color. Only Home and History keep a section accent.
    pub fn tile_color(self) -> u32 {
        match self {
            View::Home => color::ACCENT_ORANGE,
            View::History => color::ACCENT_PURPLE,
            _ => color::BORDER_SIDEBAR,
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
                "Dictionary",
                "Import",
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
            assert_eq!(*key, view.title().to_lowercase());
        }
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 7);
    }

    #[test]
    fn only_home_and_history_keep_an_accent() {
        for view in View::ALL {
            let accent = matches!(view, View::Home | View::History);
            assert_eq!(
                view.tile_color() != color::BORDER_SIDEBAR,
                accent,
                "{view:?}"
            );
        }
    }

    #[test]
    fn index_matches_position() {
        for (i, view) in View::ALL.iter().enumerate() {
            assert_eq!(view.index(), i);
        }
    }
}
