//! Updates: the daily check, automatic install, the status of the check with Check now, and
//! Share anonymous usage.

use super::Page;
use crate::settings::{Settings, slots};
use crate::ui;
use gpui_kit::assets::IconName;
use gpui_kit::{AnyElement, App, IntoElement, ParentElement, Styled, Window};

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

/// The usage switch and the row that opens "See what's sent".
pub fn share_usage(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    let on = page
        .settings
        .read(cx)
        .stored_bool(crate::updater::SHARE_USAGE);
    let settings = page.settings.clone();
    let open = move |window: &mut Window, cx: &mut App| {
        settings.update(cx, |me, cx| me.open_usage(window, cx));
    };
    let palette = page.palette;
    vec![
        page.toggle(
            "updates.shareUsage",
            "Share anonymous usage",
            "Once a day: how often you used Hushpen and which models.",
            on,
            slots::SHARE_USAGE,
            Settings::toggle_share_usage,
        ),
        ui::press_row(
            palette,
            super::settings_id("updates.usage.open"),
            "See what's sent",
            &page.focus[slots::USAGE_SENT],
            true,
            open,
        )
        .child(
            ui::title(palette)
                .flex_1()
                .min_w_0()
                .child("See what's sent"),
        )
        .child(ui::glyph(palette, IconName::ChevronRight))
        .into_any_element(),
    ]
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
