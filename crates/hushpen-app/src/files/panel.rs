//! The Manual view: the mode switch at the top, then Transcribe or Read aloud under it.

use super::{Files, Mode};
use crate::hook;
use crate::theme::{self, space};
use crate::ui;
use crate::{import, read_aloud};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div, px};

pub fn render(files: &Entity<Files>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (mode, import, read, focus) = {
        let view = files.read(cx);
        (
            view.mode(),
            view.import().clone(),
            view.read_aloud().cloned(),
            view.mode_focus.clone(),
        )
    };
    let choices: Vec<_> = Mode::ALL
        .into_iter()
        .filter(|mode| *mode == Mode::Transcribe || read.is_some())
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
    let body = match (mode, read) {
        (Mode::ReadAloud, Some(read)) => read_aloud::panel::render(&read, cx).into_any_element(),
        _ => import::panel::render(&import, cx).into_any_element(),
    };
    div()
        .id(hook::id("files", "view"))
        .test_support()
        .flex()
        .flex_col()
        .gap(px(space::XL))
        .child(div().flex().justify_center().child(ui::segmented(
            palette,
            hook::id("files", "mode"),
            "Mode",
            &choices,
            theme::size::SEGMENT_WIDTH,
            mode.key(),
            &focus,
            switch,
        )))
        .child(body)
}
