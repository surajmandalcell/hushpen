//! The microphone group on the Home view: test button, level meter, notice, and the microphone
//! list.

use super::{CaptureState, CaptureUse, METER_BARS, Mic};
use crate::hook;
use crate::theme::{self, Palette};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Window,
};
use hushpen_audio::{DEFAULT_ID, Selection};

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
        ui::meter_row(palette, "home", &meter, listening),
    ];
    if let Some(message) = notice {
        group_rows.push(ui::notice_row(
            palette,
            hook::id("home", "mic.notice"),
            message,
            Notice::Alert,
            None,
        ));
    }
    for (index, row) in rows.into_iter().enumerate() {
        group_rows.push(device_row(palette, mic, index, row, &row_focus[index]));
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
        move |_: &mut Window, cx: &mut App| {
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
    let detail = ui::secondary(palette)
        .id(hook::id("home", "mic.status"))
        .test_support()
        .child(detail);
    ui::icon_row(
        palette,
        IconName::Mic,
        ui::title(palette).child("Microphone"),
        Some(detail.into_any_element()),
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

fn device_row(
    palette: Palette,
    mic: &Entity<Mic>,
    index: usize,
    row: Row,
    focus: &FocusHandle,
) -> AnyElement {
    let id = row.id;
    let choose = {
        let mic = mic.clone();
        move |_: &mut Window, cx: &mut App| {
            mic.update(cx, |mic, cx| {
                let _ = mic.select(&id, cx);
            });
        }
    };
    let element_id = if index == 0 {
        hook::id("home", "mic.device.default")
    } else {
        hook::indexed("home", "mic.device", index)
    };
    ui::option_row(
        palette,
        element_id,
        row.name,
        row.detail,
        row.selected,
        focus,
        choose,
    )
}
