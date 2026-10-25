//! Flow bar: whether it shows, whether it stays between dictations, and where it sits.

use super::Page;
use crate::settings::{Settings, slots};
use gpui_kit::{AnyElement, App};

const POSITIONS: [(&str, &str); 2] = [
    ("bottom", "Bottom of the screen"),
    ("top", "Top of the screen"),
];

pub fn show(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("overlay.enabled");
    page.toggle(
        "flowbar.enabled",
        "Show the flow bar",
        "The floating bar while you dictate.",
        on,
        slots::FLOWBAR,
        Settings::toggle_flow_bar,
    )
}

pub fn idle(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("overlay.idleVisible");
    page.toggle(
        "flowbar.idle",
        "Show when idle",
        "The bar stays on screen between dictations.",
        on,
        slots::FLOWBAR_IDLE,
        Settings::toggle_flow_bar_idle,
    )
}

pub fn position(page: &Page, cx: &mut App) -> AnyElement {
    let position = page.settings.read(cx).stored_str("overlay.position");
    let label = |value: &str| {
        POSITIONS
            .iter()
            .find(|(key, _)| *key == value)
            .map_or(POSITIONS[0].1, |(_, label)| *label)
    };
    let menu = page.picker_open("flowbar.position", cx).then(|| {
        let choices = POSITIONS
            .into_iter()
            .map(|(value, text)| (value.to_owned(), text.to_owned(), position == value))
            .collect();
        page.options(
            "flowbar.position",
            choices,
            &page.options,
            Settings::set_position,
        )
    });
    page.menu(
        "flowbar.position",
        "Position",
        "Where the bar sits on the screen.",
        label(&position).to_owned(),
        slots::POSITION,
        menu,
    )
}
