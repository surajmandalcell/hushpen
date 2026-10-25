//! The microphone group on the Home view: one row with the microphone pop-up and the test
//! button, the level meter while a test runs, and the notice.

use super::{CaptureState, CaptureUse, Mic};
use crate::hook;
use crate::theme::{self, Palette, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Styled, Window, div,
    px,
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
            .child(ui::pop_up(
                palette,
                ui::PopUp {
                    id: hook::id("home", "mic.picker"),
                    label: "Microphone",
                    current: view.current.clone(),
                    enabled: true,
                    focus: &view.picker_focus,
                    menu: view.picker_open.then(|| menu(mic, view)),
                },
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

/// The open menu: Default under `home.mic.device.default`, then each microphone under
/// `home.mic.device.<n>` from 1.
fn menu(mic: &Entity<Mic>, view: &Snapshot) -> ui::Menu {
    let close = mic.clone();
    let menu = ui::Menu::new(hook::id("home", "mic.list"), move |cx| {
        close.update(cx, |mic, cx| {
            mic.picker_open = false;
            cx.notify();
        })
    });
    view.choices.iter().zip(&view.row_focus).enumerate().fold(
        menu,
        |menu, (index, ((id, label, chosen), focus))| {
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
            menu.option(element_id, label.clone(), None, *chosen, focus, choose)
        },
    )
}
