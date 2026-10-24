//! The microphone group on the Home view: one row with the microphone picker and the test
//! button, the open picker list, the level meter while a test runs, and the notice.

use super::{CaptureState, CaptureUse, Mic};
use crate::hook;
use crate::theme::{self, Palette, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Styled,
    Window, div, px,
};
use hushpen_audio::DEFAULT_ID;

struct Snapshot {
    /// A test is listening, so the meter shows.
    testing: bool,
    /// A dictation owns the stream, so the test button rests.
    busy: bool,
    meter: Vec<f32>,
    notice: Option<String>,
    current: String,
    choices: Vec<(String, String, bool)>,
    picker_open: bool,
    button_focus: FocusHandle,
    picker_focus: FocusHandle,
    row_focus: Vec<FocusHandle>,
}

pub fn render(mic: &Entity<Mic>, cx: &mut App) -> impl IntoElement + use<> {
    let view = {
        let mic = mic.read(cx);
        let listening = mic.state() == CaptureState::Listening;
        let busy = listening && mic.session_use() == Some(CaptureUse::Dictation);
        Snapshot {
            testing: listening && !busy,
            busy,
            meter: mic.meter(),
            notice: mic.notice().map(|notice| notice.message.clone()),
            current: mic.saved_label(),
            choices: mic.choices(),
            picker_open: mic.picker_open,
            button_focus: mic.button_focus.clone(),
            picker_focus: mic.picker_focus.clone(),
            row_focus: mic.row_focus.clone(),
        }
    };

    let palette = theme::palette(cx);
    let mut rows = vec![header(palette, mic, &view).into_any_element()];
    if view.picker_open {
        rows.push(option_list(palette, mic, &view));
    }
    if view.testing {
        rows.push(ui::meter_row(palette, "home", &view.meter, true));
    }
    if let Some(message) = view.notice {
        rows.push(ui::notice_row(
            palette,
            hook::id("home", "mic.notice"),
            message,
            Notice::Alert,
            None,
        ));
    }
    ui::group(hook::id("home", "mic"), palette, rows)
}

fn header(palette: Palette, mic: &Entity<Mic>, view: &Snapshot) -> impl IntoElement + use<> {
    let test = {
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
    let toggle = {
        let mic = mic.clone();
        move |_: &mut Window, cx: &mut App| {
            mic.update(cx, |mic, cx| {
                mic.picker_open = !mic.picker_open;
                cx.notify();
            });
        }
    };
    let detail = if view.busy {
        "Mic in use by dictation"
    } else if view.testing {
        "Listening"
    } else {
        "Dictation listens on this microphone"
    };
    let detail = ui::secondary(palette)
        .id(hook::id("home", "mic.status"))
        .test_support()
        .child(detail);
    let label = if view.testing {
        "Stop test"
    } else {
        "Test microphone"
    };
    ui::icon_row(
        palette,
        IconName::Mic,
        ui::title(palette).child("Microphone"),
        Some(detail.into_any_element()),
    )
    .child(
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .child(ui::menu_button(
                palette,
                hook::id("home", "mic.picker"),
                "Microphone",
                view.current.clone(),
                true,
                &view.picker_focus,
                toggle,
            ))
            .child(ui::button(
                palette,
                hook::id("home", "mic.record"),
                label,
                Kind::Plain,
                (!view.busy).then_some(&view.button_focus),
                !view.busy,
                test,
            )),
    )
}

/// The open picker: Default under `home.mic.device.default`, then each microphone under
/// `home.mic.device.<n>` from 1.
fn option_list(palette: Palette, mic: &Entity<Mic>, view: &Snapshot) -> AnyElement {
    let rows = view.choices.iter().zip(&view.row_focus).enumerate().map(
        |(index, ((id, label, chosen), focus))| {
            let choose = {
                let (mic, id) = (mic.clone(), id.clone());
                move |_: &mut Window, cx: &mut App| {
                    mic.update(cx, |mic, cx| {
                        let _ = mic.select(&id, cx);
                    });
                }
            };
            let element_id = if id == DEFAULT_ID {
                hook::id("home", "mic.device.default")
            } else {
                hook::indexed("home", "mic.device", index)
            };
            ui::option_row(
                palette,
                element_id,
                label.clone(),
                None,
                *chosen,
                focus,
                choose,
            )
        },
    );
    let close = mic.clone();
    ui::option_list(hook::id("home", "mic.list"), rows.collect(), move |cx| {
        close.update(cx, |mic, cx| {
            mic.picker_open = false;
            cx.notify();
        })
    })
}
