//! The microphone group on the Home view: test button, level meter, notice, and the microphone
//! list.

use super::{CaptureState, CaptureUse, METER_BARS, Mic};
use crate::hook;
use crate::theme::{self, BODY, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px, svg,
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

    let palette = theme::palette(cx);
    let mut group_rows: Vec<AnyElement> = vec![
        header(palette, mic, state, busy, &button_focus, default_name).into_any_element(),
        meter_row(palette, &meter, listening).into_any_element(),
    ];
    if let Some(message) = notice {
        group_rows.push(notice_row(palette, message).into_any_element());
    }
    for (index, row) in rows.into_iter().enumerate() {
        group_rows.push(
            device_row(palette, mic, index, row, row_focus[index].clone()).into_any_element(),
        );
    }
    ui::group(hook::id("home", "mic"), palette, group_rows)
}

fn header(
    palette: Palette,
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
    ui::row(palette)
        .child(ui::icon(palette, IconName::Mic))
        .child(
            ui::text_column()
                .child(ui::title(palette).child("Microphone"))
                .child(
                    ui::secondary(palette)
                        .id(hook::id("home", "mic.status"))
                        .test_support()
                        .child(detail),
                ),
        )
        .child(ui::button(
            palette,
            hook::id("home", "mic.record"),
            label,
            Kind::Plain,
            (!busy).then_some(focus),
            !busy,
            toggle,
        ))
}

fn meter_row(palette: Palette, levels: &[f32], listening: bool) -> impl IntoElement + use<> {
    let bar_color = if listening {
        palette.label_secondary
    } else {
        palette.label_tertiary
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
                .rounded(px(radius::BUTTON))
                .bg(bar_color)
        });
    div()
        .id(hook::id("home", "mic.meter"))
        .test_support()
        .flex()
        .items_end()
        .justify_center()
        .flex_none()
        .gap(px(BAR_GAP))
        .h(px(BAR_MAX + 2.0 * space::MD))
        .px(px(size::ROW_PADDING))
        .py(px(space::MD))
        .children(bars)
}

fn notice_row(palette: Palette, message: String) -> impl IntoElement {
    ui::row(palette)
        .id(hook::id("home", "mic.notice"))
        .test_support()
        .role(Role::Alert)
        .text_color(palette.label)
        .child(
            svg()
                .path(IconName::CircleAlert.path())
                .size(px(size::NAV_ICON))
                .flex_none()
                .text_color(palette.label_secondary),
        )
        .child(
            ui::title(palette)
                .flex_1()
                .min_w_0()
                .text_color(palette.label)
                .child(message),
        )
}

fn device_row(
    palette: Palette,
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
        .gap(px(ui::ROW_GAP))
        .w_full()
        .flex_none()
        .h(px(size::ROW_MIN_HEIGHT))
        .px(px(size::ROW_PADDING - size::FOCUS_RING))
        .border_2()
        .border_color(transparent_black())
        .cursor_pointer()
        .text_token(BODY)
        .text_color(palette.label)
        .hover(move |style| style.bg(palette.hover()))
        .focus_visible(move |style| style.border_color(palette.accent))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(
            svg()
                .path(IconName::Check.path())
                .size(px(size::NAV_ICON))
                .flex_none()
                .text_color(if row.selected {
                    palette.label
                } else {
                    gpui_kit::transparent_black().into()
                }),
        )
        .child(div().flex_none().child(row.name))
        .child(
            ui::secondary(palette)
                .flex_1()
                .min_w_0()
                .children(row.detail),
        )
}
