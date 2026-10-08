//! The Shortcuts section of Settings: one row for each shortcut with a recorder field, the
//! reason a shortcut was refused, and a button that puts all four back to their defaults.

use super::Shortcuts;
use crate::hook;
use crate::theme::{
    self, BODY_MD, BODY_SM, LABEL_MD, ROW_TITLE, StyledType, color, radius, size, space,
};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
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
            "Hold this key to speak an instruction for the selected text.",
        ),
    }
}

pub fn render(shortcuts: &Entity<Shortcuts>, cx: &mut App) -> impl IntoElement + use<> {
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
    let mut panel = div()
        .id(hook::id("settings", "shortcuts"))
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(header(shortcuts, &reset_focus));
    if let Some(message) = why_off {
        panel = panel.child(note_row(message));
    }
    for row in rows {
        panel = panel.child(shortcut_row(shortcuts, row));
    }
    panel
}

fn header(shortcuts: &Entity<Shortcuts>, reset_focus: &FocusHandle) -> impl IntoElement + use<> {
    let reset = {
        let shortcuts = shortcuts.clone();
        move |cx: &mut App| {
            shortcuts.update(cx, |shortcuts, cx| {
                if let Err(error) = shortcuts.reset_all(cx) {
                    log::warn!("the shortcuts could not be reset: {error}");
                }
            });
        }
    };
    div()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .p(px(space::LG))
        .child(icon_tile(IconName::Keyboard))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_token(ROW_TITLE)
                        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                        .child("Shortcuts"),
                )
                .child(
                    div()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .child("Click a shortcut, then press the keys. Esc cancels."),
                ),
        )
        .child(button(
            hook::id("settings", "shortcut.reset"),
            "Reset to default",
            reset_focus,
            reset,
        ))
}

fn note_row(message: String) -> impl IntoElement + use<> {
    div()
        .id(hook::id("settings", "shortcut.keys-off"))
        .test_support()
        .role(Role::Status)
        .aria_label(message.clone())
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::TEXT_MUTED))
        .child(
            svg()
                .path(IconName::Info.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::TEXT_MUTED)),
        )
        .child(div().flex_1().min_w_0().child(message))
}

fn shortcut_row(shortcuts: &Entity<Shortcuts>, row: Row) -> impl IntoElement + use<> {
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
        .h(px(32.0))
        .px(px(10.0 - size::FOCUS_RING))
        .rounded(px(radius::MD))
        .border_2()
        .border_color(theme::rgb_of(if row.recording {
            color::FOCUS
        } else {
            color::BORDER_CONTROL
        }))
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(if !row.enabled {
            color::TEXT_DISABLED
        } else if row.recording {
            color::TEXT_MUTED
        } else {
            color::TEXT_PRIMARY
        }));
    hook::mark_disabled(&format!("settings.shortcut.{key}"), !row.enabled);
    if row.enabled {
        let on_click = {
            let toggle = toggle.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| toggle(cx)
        };
        let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                toggle(cx);
                cx.stop_propagation();
            }
        };
        field = field
            .track_focus(&row.focus)
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key);
    }
    let field = field.child(div().truncate().child(row.shown));
    let mut block = div()
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .px(px(space::LG))
                .py(px(space::MD))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_token(ROW_TITLE)
                                .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                                .child(row.title),
                        )
                        .child(
                            div()
                                .text_token(BODY_SM)
                                .text_color(theme::rgb_of(color::TEXT_MUTED))
                                .child(row.detail),
                        ),
                )
                .child(field),
        );
    if let Some(message) = row.error {
        block = block.child(error_row(key, message));
    }
    block
}

fn error_row(key: &'static str, message: String) -> impl IntoElement + use<> {
    div()
        .id(hook::id("settings", &format!("shortcut.{key}.error")))
        .test_support()
        .role(Role::Alert)
        .aria_label(message.clone())
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .pb(px(space::MD))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::STATUS_ERROR))
        .child(
            svg()
                .path(IconName::CircleAlert.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::STATUS_ERROR)),
        )
        .child(div().flex_1().min_w_0().child(message))
}

fn button<F: Fn(&mut App) + Clone + 'static>(
    id: gpui_kit::SharedString,
    label: &'static str,
    focus: &FocusHandle,
    press: F,
) -> impl IntoElement + use<F> {
    let on_click = {
        let press = press.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| press(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            press(cx);
            cx.stop_propagation();
        }
    };
    div()
        .id(id)
        .test_support()
        .track_focus(focus)
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .h(px(32.0))
        .px(px(10.0 - size::FOCUS_RING))
        .rounded(px(radius::MD))
        .border_2()
        .border_color(transparent_black())
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
        .cursor_pointer()
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(label)
}

fn icon_tile(icon: IconName) -> impl IntoElement {
    div()
        .size(px(28.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .bg(theme::rgb_of(color::SURFACE_SOFT))
        .child(
            svg()
                .path(icon.path())
                .size(px(14.0))
                .text_color(theme::rgb_of(color::TEXT_BODY)),
        )
}
