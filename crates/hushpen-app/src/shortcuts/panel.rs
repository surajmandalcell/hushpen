//! The rows of the Shortcuts section of Settings: a row for each shortcut with, lightest to
//! heaviest, a reset button, its recorder field, and its switch; the reason a shortcut was
//! refused or does not run; how to free a shortcut that something else runs; and a row that
//! puts all of them back to their defaults.

use super::Shortcuts;
use crate::hook;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement as _,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px, transparent_black,
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
    detail: String,
    shown: String,
    recording: bool,
    /// The key listener runs, so the shortcut can be recorded.
    enabled: bool,
    /// The user has the shortcut on.
    on: bool,
    is_default: bool,
    error: Option<String>,
    /// How to free a shortcut that something else runs, and whether its settings can be opened.
    steps: Option<(String, bool)>,
    focus: FocusHandle,
    reset_focus: FocusHandle,
    switch_focus: FocusHandle,
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
        Slot::ReadAloud => ("Read aloud", "Reads the selected text out loud."),
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
        let built_in = me.value(Slot::HandsFree).combo().is_none();
        let detail = match slot {
            Slot::Hold if !me.is_on(slot) && me.is_on(Slot::HandsFree) && built_in => {
                "Off. Hands-free still starts with this key.".to_owned()
            }
            _ => detail.to_owned(),
        };
        Row {
            slot,
            title,
            detail,
            shown,
            recording,
            enabled: me.why_off().is_none(),
            on: me.is_on(slot),
            is_default: me.is_default(slot),
            error: me.failure(slot).map(|refusal| refusal.message.clone()),
            steps: me.failure(slot).and_then(|refusal| {
                let steps = refusal.steps.clone()?;
                Some((steps, refusal.open.is_some()))
            }),
            focus: me.field_focus[index].clone(),
            reset_focus: me.slot_reset_focus[index].clone(),
            switch_focus: me.switch_focus[index].clone(),
        }
    };
    let (error, steps) = (row.error.clone(), row.steps.clone());
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
    if let Some((steps, can_open)) = steps {
        let open = can_open.then(|| {
            let shortcuts = shortcuts.clone();
            ui::button(
                palette,
                hook::id("settings", &format!("shortcut.{}.open", slot.key())),
                "Open Settings",
                Kind::Plain,
                None,
                true,
                move |_: &mut Window, cx: &mut App| {
                    if let Err(error) = shortcuts.read(cx).open_conflict_settings(slot) {
                        log::warn!("{error}");
                    }
                },
            )
            .into_any_element()
        });
        rows.push(ui::notice_row(
            palette,
            hook::id("settings", &format!("shortcut.{}.steps", slot.key())),
            steps,
            Notice::Info,
            open,
        ));
    }
    rows
}

/// The row that puts every shortcut back to its default and turns it on.
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
        .text_color(if !row.enabled || !row.on {
            palette.label_tertiary
        } else if row.recording {
            palette.label_secondary
        } else {
            palette.label
        });
    let usable = row.enabled && row.on;
    hook::mark_disabled(&format!("settings.shortcut.{key}"), !usable);
    if !row.recording && needs_tooltip(&row.shown) {
        field = ui::tooltip(field, row.shown.clone());
    }
    if usable {
        let on_click = move |_: &ClickEvent, _: &mut Window, cx: &mut App| toggle(cx);
        field = field
            .track_focus(&row.focus)
            .cursor_pointer()
            .hover(move |style| style.bg(palette.selection))
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_click(on_click);
    }
    let slot = row.slot;
    // A shortcut at its default has nothing to reset; the empty place keeps the fields lined up.
    let reset = if row.is_default {
        div()
            .flex_none()
            .size(px(size::BUTTON_HEIGHT))
            .into_any_element()
    } else {
        let shortcuts = shortcuts.clone();
        ui::icon_button(
            palette,
            hook::id("settings", &format!("shortcut.{key}.reset")),
            IconName::RotateCcw,
            format!("Reset {} to default", row.title),
            "Reset to default",
            &row.reset_focus,
            move |_: &mut Window, cx: &mut App| {
                shortcuts.update(cx, |shortcuts, cx| {
                    if let Err(error) = shortcuts.reset(slot, cx) {
                        log::debug!("the shortcut was not reset: {error}");
                    }
                });
            },
        )
        .into_any_element()
    };
    let switch = {
        let shortcuts = shortcuts.clone();
        let on = row.on;
        ui::switch(
            palette,
            hook::id("settings", &format!("shortcut.{key}.on")),
            row.title,
            on,
            true,
            &row.switch_focus,
            move |_: &mut Window, cx: &mut App| {
                shortcuts.update(cx, |shortcuts, cx| {
                    if let Err(error) = shortcuts.set_on(slot, !on, cx) {
                        log::debug!("the shortcut was not switched: {error}");
                    }
                });
            },
        )
    };
    ui::labeled_row(
        palette,
        hook::id("settings", &format!("shortcut.row.{key}")),
        row.title,
        Some(row.detail),
        div()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .child(reset)
            .child(field.child(div().min_w_0().truncate().child(row.shown)))
            .child(switch),
    )
}
