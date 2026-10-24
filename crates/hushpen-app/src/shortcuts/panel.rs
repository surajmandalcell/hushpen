//! The rows of the Shortcuts section of Settings: a row for each shortcut with a recorder field
//! on its right, the reason a shortcut was refused, and a row that puts all four back to their
//! defaults.

use super::Shortcuts;
use crate::hook;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    AnyElement, App, ClickEvent, Entity, FocusHandle, InteractiveElement, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div, px, transparent_black,
};
use hushpen_core::shortcut::Slot;

/// A binding this long may be cut by the field, so its whole text shows as a tooltip.
const TOOLTIP_FROM_CHARS: usize = 24;

pub(super) fn needs_tooltip(shown: &str) -> bool {
    shown.chars().count() > TOOLTIP_FROM_CHARS
}

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
            "Command mode",
            "Hold to speak an instruction for the selected text.",
        ),
    }
}

/// The notice that the shortcuts cannot work here, such as on Wayland, or nothing.
pub fn keys_off_row(shortcuts: &Entity<Shortcuts>, cx: &mut App) -> Option<AnyElement> {
    let palette = theme::palette(cx);
    let message = shortcuts.read(cx).why_off()?.to_owned();
    Some(ui::notice_row(
        palette,
        hook::id("settings", "shortcut.keys-off"),
        message,
        Notice::Info,
        None,
    ))
}

/// The row of one shortcut with its recorder field, and the reason under it when the last
/// shortcut recorded for it was refused.
pub fn slot_rows(shortcuts: &Entity<Shortcuts>, slot: Slot, cx: &mut App) -> Vec<AnyElement> {
    let palette = theme::palette(cx);
    let row = {
        let me = shortcuts.read(cx);
        let (title, detail) = text_of(slot);
        let recording = me.recording() == Some(slot);
        let shown = if recording {
            me.progress()
                .unwrap_or_else(|| "Press the shortcut".to_owned())
        } else {
            me.display(slot)
        };
        let index = Slot::ALL.iter().position(|each| *each == slot).unwrap_or(0);
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
    };
    let error = row.error.clone();
    let mut rows = vec![shortcut_row(palette, shortcuts, row)];
    if let Some(message) = error {
        rows.push(ui::notice_row(
            palette,
            hook::id("settings", &format!("shortcut.{}.error", slot.key())),
            message,
            Notice::Alert,
            None,
        ));
    }
    rows
}

/// The row that puts all four shortcuts back to their defaults.
pub fn reset_row(shortcuts: &Entity<Shortcuts>, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let reset_focus = shortcuts.read(cx).reset_focus.clone();
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
            Some(&reset_focus),
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
        .w(px(size::FIELD_WIDTH))
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
    if !row.recording && needs_tooltip(&row.shown) {
        let full = row.shown.clone();
        field = field.tooltip(move |window, cx| Tooltip::new(full.clone()).build(window, cx));
    }
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
        field.child(div().min_w_0().truncate().child(row.shown)),
    )
}
