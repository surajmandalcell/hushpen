//! The macOS menu bar: Hushpen (About, Settings, Hide, Quit) and Edit (Undo, Cut, Copy, Paste,
//! Select All). Other systems have no application menu bar, so [`install`] does nothing there.

use crate::actions::{HideHushpen, QuitHushpen, ShowAbout, ShowSettings};
#[cfg(target_os = "macos")]
use gpui_kit::KeyBinding;
use gpui_kit::component::input::{Copy, Cut, Paste, SelectAll, Undo};
use gpui_kit::{App, Menu, MenuItem, OsAction};

pub fn menus() -> Vec<Menu> {
    vec![
        Menu::new("Hushpen").items([
            MenuItem::action("About Hushpen", ShowAbout),
            MenuItem::separator(),
            MenuItem::action("Settings…", ShowSettings),
            MenuItem::separator(),
            MenuItem::action("Hide Hushpen", HideHushpen),
            MenuItem::separator(),
            MenuItem::action("Quit Hushpen", QuitHushpen),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
        ]),
    ]
}

/// Puts the menus in the menu bar, with the keys that macOS users expect for them.
pub fn install(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        cx.bind_keys([
            KeyBinding::new("cmd-q", QuitHushpen, None),
            KeyBinding::new("cmd-,", ShowSettings, None),
            KeyBinding::new("cmd-h", HideHushpen, None),
        ]);
        cx.set_menus(menus());
    }
    #[cfg(not(target_os = "macos"))]
    let _ = cx;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(menu: &Menu) -> Vec<String> {
        menu.items
            .iter()
            .filter_map(|item| match item {
                MenuItem::Action { name, .. } => Some(name.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_hushpen_menu_has_about_settings_hide_and_quit() {
        let menus = menus();
        assert_eq!(menus[0].name, "Hushpen");
        assert_eq!(
            titles(&menus[0]),
            ["About Hushpen", "Settings…", "Hide Hushpen", "Quit Hushpen"]
        );
    }

    #[test]
    fn the_edit_menu_has_the_five_text_commands() {
        let menus = menus();
        assert_eq!(menus[1].name, "Edit");
        assert_eq!(
            titles(&menus[1]),
            ["Undo", "Cut", "Copy", "Paste", "Select All"]
        );
    }
}
