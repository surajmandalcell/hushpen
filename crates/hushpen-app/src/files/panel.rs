//! The Manual view: Transcribe or Read aloud, the two tabs after Home in the title bar.

use super::{Files, Mode};
use crate::hook;
use crate::{import, read_aloud};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div};

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
