//! The microphone panel on the Home view: test button, level meter, notice,
//! and the microphone list.

use super::{CaptureState, CaptureUse, METER_BARS, Mic};
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
use hushpen_audio::{DEFAULT_ID, Selection};

const BAR_WIDTH: f32 = 4.0;
const BAR_GAP: f32 = 3.0;
const BAR_REST: f32 = 3.0;
const BAR_MAX: f32 = 28.0;

struct Row {
    id: String,
    name: String,
    detail: Option<String>,
    selected: bool,
}

pub fn render(mic: &Entity<Mic>, cx: &mut App) -> impl IntoElement + use<> {
    let (state, busy, meter, notice, rows, button_focus, row_focus, default_name) = {
        let mic = mic.read(cx);
        let default_selected = !matches!(mic.selection(), Selection::Device(_));
        let rows: Vec<Row> = std::iter::once(Row {
            id: DEFAULT_ID.to_string(),
            name: "Default".into(),
            detail: mic.default_name().map(str::to_string),
            selected: default_selected,
        })
        .chain(
            mic.devices()
                .iter()
                .take(super::MAX_DEVICE_ROWS)
                .map(|device| Row {
                    id: device.id.clone(),
                    name: device.name.clone(),
                    detail: None,
                    selected: matches!(mic.selection(), Selection::Device(id) if *id == device.id),
                }),
        )
        .collect();
        // While a dictation owns the stream the panel shows a busy note and
        // rests the meter: the levels on screen belong to a test only.
        let busy = mic.state() == CaptureState::Listening
            && mic.session_use() == Some(CaptureUse::Dictation);
        let meter = if busy {
            vec![0.0; METER_BARS]
        } else {
            mic.meter()
        };
        (
            mic.state(),
            busy,
            meter,
            mic.notice().map(|notice| notice.message.clone()),
            rows,
            mic.button_focus.clone(),
            mic.row_focus.clone(),
            mic.default_name().map(str::to_string),
        )
    };
    let listening = state == CaptureState::Listening && !busy;

    let mut panel = div()
        .id(hook::id("home", "mic"))
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(header(mic, state, busy, &button_focus, default_name))
        .child(meter_row(&meter, listening));
    if let Some(message) = notice {
        panel = panel.child(notice_row(message));
    }
    for (index, row) in rows.into_iter().enumerate() {
        panel = panel.child(device_row(mic, index, row, row_focus[index].clone()));
    }
    panel
}

fn header(
    mic: &Entity<Mic>,
    state: CaptureState,
    busy: bool,
    focus: &FocusHandle,
    default_name: Option<String>,
) -> impl IntoElement + use<> {
    let toggle = {
        let mic = mic.clone();
        move |cx: &mut App| {
            mic.update(cx, |mic, cx| {
                if mic.state() == CaptureState::Listening {
                    let _ = mic.stop(cfg!(feature = "test-automation"), cx);
                } else {
                    let _ = mic.start(CaptureUse::Test, cx);
                }
            });
        }
    };
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
    let listening = state == CaptureState::Listening && !busy;
    let label = if listening {
        "Stop test"
    } else {
        "Test microphone"
    };
    let detail = if busy {
        "Mic in use by dictation".to_string()
    } else if listening {
        "Listening".to_string()
    } else {
        default_name.map_or_else(
            || "Default microphone".to_string(),
            |name| format!("Default: {name}"),
        )
    };
    div()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .p(px(space::LG))
        .child(icon_tile(IconName::Mic))
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
                        .child("Microphone"),
                )
                .child(
                    div()
                        .id(hook::id("home", "mic.status"))
                        .test_support()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .truncate()
                        .child(detail),
                ),
        )
        .child({
            let id = hook::id("home", "mic.record");
            hook::mark_disabled(&id, busy);
            let mut button = div()
                .id(id)
                .test_support()
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
                .child(label);
            if busy {
                button = button.text_color(theme::rgb_of(color::TEXT_DISABLED));
            } else {
                button = button
                    .track_focus(focus)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
                    .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
                    .on_click(on_click)
                    .on_key_down(on_key);
            }
            button
        })
}

fn meter_row(levels: &[f32], listening: bool) -> impl IntoElement + use<> {
    let bar_color = if listening {
        color::ACCENT_BLUE
    } else {
        color::BORDER_CONTROL
    };
    let bars = levels
        .iter()
        .take(METER_BARS)
        .enumerate()
        .map(|(index, level)| {
            div()
                .id(hook::indexed("home", "mic.bar", index))
                .test_support()
                .flex_none()
                .w(px(BAR_WIDTH))
                .h(px(BAR_REST + level.clamp(0.0, 1.0) * (BAR_MAX - BAR_REST)))
                .rounded(px(radius::XS))
                .bg(theme::rgb_of(bar_color))
        });
    div()
        .id(hook::id("home", "mic.meter"))
        .test_support()
        .flex()
        .items_end()
        .justify_center()
        .gap(px(BAR_GAP))
        .h(px(BAR_MAX + 2.0 * space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .children(bars)
}

fn notice_row(message: String) -> impl IntoElement {
    div()
        .id(hook::id("home", "mic.notice"))
        .test_support()
        .role(Role::Alert)
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
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

fn device_row(
    mic: &Entity<Mic>,
    index: usize,
    row: Row,
    focus: FocusHandle,
) -> impl IntoElement + use<> {
    let id = row.id.clone();
    let choose = {
        let mic = mic.clone();
        move |cx: &mut App| {
            let id = id.clone();
            mic.update(cx, |mic, cx| {
                let _ = mic.select(&id, cx);
            });
        }
    };
    let on_click = {
        let choose = choose.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| choose(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            choose(cx);
            cx.stop_propagation();
        }
    };
    let element_id = if index == 0 {
        hook::id("home", "mic.device.default")
    } else {
        hook::indexed("home", "mic.device", index)
    };
    div()
        .id(element_id)
        .test_support()
        .track_focus(&focus)
        .role(Role::RadioButton)
        .aria_label(row.name.clone())
        .flex()
        .items_center()
        .gap(px(space::SM))
        .h(px(size::NAV_ITEM_HEIGHT + space::SM))
        .px(px(space::LG - size::FOCUS_RING))
        .border_2()
        .border_color(transparent_black())
        .cursor_pointer()
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::TEXT_BODY))
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(div().flex_none().child(row.name))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(color::TEXT_MUTED))
                .children(row.detail),
        )
        .child(
            svg()
                .path(IconName::Check.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(if row.selected {
                    color::FOCUS
                } else {
                    color::SURFACE
                })),
        )
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
