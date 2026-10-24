//! Updates: the daily check, automatic install, and the status of the check with Check now.

use super::Page;
use crate::settings::{Settings, slots};
use gpui_kit::{AnyElement, App};

pub fn check(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("updates.check");
    page.toggle(
        "updates.check",
        "Check for updates",
        "Asks GitHub for a newer version at most once a day. Nothing identifies you.",
        on,
        slots::UPDATES_CHECK,
        Settings::toggle_update_check,
    )
}

pub fn auto_install(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("updates.autoInstall");
    page.toggle(
        "updates.autoInstall",
        "Install updates automatically",
        "Downloads a new version for you. It starts after you restart.",
        on,
        slots::UPDATES_AUTO,
        Settings::toggle_auto_install,
    )
}

/// The rows of the update check. Nothing before the updater is attached.
pub fn status(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    let Some(updater) = page.settings.read(cx).updater.clone() else {
        return Vec::new();
    };
    crate::updater::rows(
        &updater,
        &page.focus[slots::UPDATES_NOW],
        &page.focus[slots::UPDATES_NOTES],
        cx,
    )
}
