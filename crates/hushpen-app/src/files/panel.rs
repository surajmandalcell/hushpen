//! The Manual view: Transcribe or Read aloud. The mode switch sits in the title bar.

use super::{Files, Mode};
use crate::hook;
use crate::theme;
use crate::ui;
use crate::{import, read_aloud};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div};

/// The Transcribe | Read aloud control, for the middle of the title bar.
pub fn mode_switch(files: &Entity<Files>, cx: &App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (mode, has_read, focus) = {
        let view = files.read(cx);
        (
            view.mode(),
            view.read_aloud().is_some(),
            view.mode_focus.clone(),
        )
    };
    let choices: Vec<_> = Mode::ALL
        .into_iter()
        .filter(|mode| *mode == Mode::Transcribe || has_read)
        .map(|mode| (mode.key(), mode.label()))
        .collect();
    let switch = {
        let files = files.clone();
        move |key: &'static str, _: &mut gpui_kit::Window, cx: &mut App| {
            if let Some(mode) = Mode::from_key(key) {
                // A refusal is a settings file that cannot be written; the view stays.
                let _ = files.update(cx, |files, cx| files.set_mode(mode, cx));
            }
        }
    };
    ui::segmented(
        palette,
        hook::id("files", "mode"),
        "Mode",
        &choices,
        theme::size::SEGMENT_WIDTH,
        mode.key(),
        &focus,
        switch,
    )
}

pub fn render(files: &Entity<Files>, cx: &mut App) -> impl IntoElement + use<> {
    let (mode, import, read) = {
        let view = files.read(cx);
        (
            view.mode(),
            view.import().clone(),
            view.read_aloud().cloned(),
        )
    };
    let body = match (mode, read) {
        (Mode::ReadAloud, Some(read)) => read_aloud::panel::render(&read, cx).into_any_element(),
        _ => import::panel::render(&import, cx).into_any_element(),
    };
    div()
        .id(hook::id("files", "view"))
        .test_support()
        .flex()
        .flex_col()
        .child(body)
}
