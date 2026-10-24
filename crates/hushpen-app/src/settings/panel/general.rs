//! General: launch at login and start hidden.

use super::Page;
use crate::settings::{Settings, slots};
use gpui_kit::{AnyElement, App};

pub fn launch_at_login(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).launch_at_login();
    page.toggle(
        "general.launchAtLogin",
        "Launch at login",
        "Hushpen starts when you log in.",
        on,
        slots::LOGIN,
        Settings::toggle_launch_at_login,
    )
}

pub fn start_hidden(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("startup.startHidden");
    page.toggle(
        "general.startHidden",
        "Start hidden",
        "A start at login shows only the tray icon.",
        on,
        slots::HIDDEN,
        Settings::toggle_start_hidden,
    )
}
