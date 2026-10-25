//! Storage: the data folder and its move, and how long dictation audio stays.

use super::Page;
use crate::mic;
use crate::settings::{MoveState, Settings, slots};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Window,
};

const RETENTION: [(&str, &str); 3] = [
    ("never", "Never"),
    ("30d", "30 days"),
    ("forever", "Keep forever"),
];

pub fn data_folder(page: &Page, cx: &mut App) -> AnyElement {
    let (folder, move_state, recording) = {
        let me = page.settings.read(cx);
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        (
            hushpen_core::paths::display(me.storage.data.root(), home.as_deref()),
            me.move_state.clone(),
            me.mic.read(cx).state() == mic::CaptureState::Listening,
        )
    };
    let (label, enabled) = match move_state {
        MoveState::Idle | MoveState::Failed(_) => ("Change…", !recording),
        MoveState::Choosing => ("Choosing…", false),
        MoveState::Restarting => ("Restarting…", false),
    };
    let settings = page.settings.clone();
    let change =
        move |_: &mut Window, cx: &mut App| settings.update(cx, |me, cx| me.change_data_folder(cx));
    let palette = page.palette;
    ui::row(palette)
        .id(super::settings_id("storage.folder.row"))
        .test_support()
        .child(
            ui::text_column()
                .child(ui::title(palette).child("Data folder"))
                .child(
                    ui::secondary(palette)
                        .id(super::settings_id("storage.dataFolder"))
                        .test_support()
                        .aria_label(folder.clone())
                        .child(folder),
                ),
        )
        .child(ui::button(
            palette,
            super::settings_id("storage.changeFolder"),
            label,
            Kind::Plain,
            Some(&page.focus[slots::CHANGE_FOLDER]),
            enabled,
            change,
        ))
        .into_any_element()
}

pub fn retention(page: &Page, cx: &mut App) -> AnyElement {
    let retention = page.settings.read(cx).stored_str("history.audioRetention");
    let current = RETENTION
        .iter()
        .find(|(key, _)| *key == retention)
        .map_or("30 days", |(_, label)| *label);
    let menu = page.picker_open("storage.retention", cx).then(|| {
        let choices = RETENTION
            .into_iter()
            .map(|(value, label)| (value.to_owned(), label.to_owned(), retention == value))
            .collect();
        page.options(
            "storage.retention",
            choices,
            &page.options,
            Settings::set_retention,
        )
    });
    page.menu(
        "storage.retention",
        "Keep dictation audio",
        "How long the audio of a finished dictation stays.",
        current.to_owned(),
        slots::RETENTION,
        menu,
    )
}
