//! The Shortcuts section of Settings: one group with a row for each shortcut and a recorder
//! field on its right, the reason a shortcut was refused, and a row that puts all four back
//! to their defaults.

use super::Shortcuts;
use crate::hook;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px, transparent_black,
};
use hushpen_core::shortcut::Slot;

const FIELD_MIN_WIDTH: f32 = 128.0;
const FIELD_MAX_WIDTH: f32 = 236.0;

struct Row {
    slot: Slot,
    title: &'static str,
    detail: &'static str,
    shown: String,
    recording: bool,
    enabled: bool,
    error: Option<String>,
    focus: FocusHandle,
}

fn text_of(slot: Slot) -> (&'static str, &'static str) {
    match slot {
        Slot::Hold => ("Hold to dictate", "Hold this key while you speak."),
        Slot::HandsFree => ("Hands-free", "Starts a session. Use it again to stop."),
        Slot::PasteLast => ("Paste last transcript", "Pastes your last dictation again."),
        Slot::Command => (
            "Command Mode",
            "Hold to speak an instruction for the selected text.",
        ),
    }
}

pub fn render(shortcuts: &Entity<Shortcuts>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (rows, why_off, reset_focus) = {
        let me = shortcuts.read(cx);
        let rows: Vec<Row> = Slot::ALL
            .into_iter()
            .enumerate()
            .map(|(index, slot)| {
                let (title, detail) = text_of(slot);
                let recording = me.recording() == Some(slot);
                let shown = if recording {
                    me.progress()
                        .unwrap_or_else(|| "Press the shortcut".to_owned())
                } else {
                    me.display(slot)
                };
                Row {
                    slot,
                    title,
                    detail,
                    shown,
                    recording,
                    enabled: me.why_off().is_none(),
                    error: me.failure(slot).map(|refusal| refusal.message.clone()),
                    focus: me.field_focus[index].clone(),
                }
            })
            .collect();
        (
            rows,
            me.why_off().map(str::to_owned),
            me.reset_focus.clone(),
        )
    };
    let mut group_rows: Vec<AnyElement> = Vec::new();
    if let Some(message) = why_off {
        group_rows.push(ui::note_row(
            palette,
            hook::id("settings", "shortcut.keys-off"),
            message,
            false,
        ));
    }
    for row in rows {
        let key = row.slot.key();
        let error = row.error.clone();
        group_rows.push(shortcut_row(palette, shortcuts, row));
        if let Some(message) = error {
            group_rows.push(ui::note_row(
                palette,
                hook::id("settings", &format!("shortcut.{key}.error")),
                message,
                true,
            ));
        }
    }
    group_rows.push(reset_row(palette, shortcuts, &reset_focus));
    ui::section(
        palette,
        ui::plain_group(hook::id("settings", "shortcuts"), palette, group_rows),
        Some((
            hook::id("settings", "shortcuts.help"),
            "Click a shortcut, then press the keys. Esc cancels.".to_owned(),
        )),
    )
}

fn reset_row(
    palette: Palette,
    shortcuts: &Entity<Shortcuts>,
    reset_focus: &FocusHandle,
) -> AnyElement {
    let reset = {
        let shortcuts = shortcuts.clone();
        move |_: &mut Window, cx: &mut App| {
            shortcuts.update(cx, |shortcuts, cx| {
                if let Err(error) = shortcuts.reset_all(cx) {
                    log::warn!("the shortcuts could not be reset: {error}");
                }
            });
        }
    };
    ui::labeled_row(
        palette,
        hook::id("settings", "shortcut.reset.row"),
        "Default shortcuts",
        None,
        ui::button(
            palette,
            hook::id("settings", "shortcut.reset"),
            "Reset to default",
            Kind::Plain,
            Some(reset_focus),
            true,
            reset,
        ),
    )
}

fn shortcut_row(palette: Palette, shortcuts: &Entity<Shortcuts>, row: Row) -> AnyElement {
    let key = row.slot.key();
    let toggle = {
        let shortcuts = shortcuts.clone();
        let slot = row.slot;
        move |cx: &mut App| {
            shortcuts.update(cx, |shortcuts, cx| {
                if shortcuts.recording() == Some(slot) {
                    shortcuts.cancel_recording(cx);
                } else if let Err(error) = shortcuts.start_recording(slot, cx) {
                    log::debug!("the recorder did not open: {error}");
                }
            });
        }
    };
    let mut field = div()
        .id(hook::id("settings", &format!("shortcut.{key}")))
        .test_support()
        .role(Role::Button)
        .aria_label(row.shown.clone())
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(FIELD_MIN_WIDTH))
        .max_w(px(FIELD_MAX_WIDTH))
        .h(px(size::BUTTON_HEIGHT))
        .px(px(size::BUTTON_PADDING - size::FOCUS_RING))
        .rounded(px(radius::BUTTON))
        .border_2()
        .border_color(if row.recording {
            palette.accent
        } else {
            transparent_black().into()
        })
        .bg(palette.control)
        .text_token(BUTTON)
        .text_color(if !row.enabled {
            palette.label_tertiary
        } else if row.recording {
            palette.label_secondary
        } else {
            palette.label
        });
    hook::mark_disabled(&format!("settings.shortcut.{key}"), !row.enabled);
    if row.enabled {
        let on_click = move |_: &ClickEvent, _: &mut Window, cx: &mut App| toggle(cx);
        field = field
            .track_focus(&row.focus)
            .cursor_pointer()
            .hover(move |style| style.bg(palette.selection))
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_click(on_click);
    }
    ui::labeled_row(
        palette,
        hook::id("settings", &format!("shortcut.row.{key}")),
        row.title,
        Some(row.detail.to_owned()),
        field.child(div().truncate().child(row.shown)),
    )
}
