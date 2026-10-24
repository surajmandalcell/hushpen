//! The onboarding page: a step header, one grouped panel for the step, and the Continue button.
//! There is no skip control and no way back: each step has to pass before the next one opens.

use super::{MicPhase, Mode, Onboarding};
use crate::hook;
use crate::models::ModelState;
use crate::models::panel::format_size;
use crate::theme::{self, BODY, Palette, SECONDARY, StyledType, TITLE, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Rgba,
    Role, StatefulInteractiveElement, Styled, Window, div, px, svg, transparent_black,
};
use hushpen_core::onboarding::{
    HINT_NO_SOUND, HINT_NO_SPEECH, HINT_TRANSCRIPT_FAILED, MicVerdict, NOTE_NO_MODEL, Row,
    RowState, Step,
};
use hushpen_core::permission::Permission;

const FIELD_MIN_HEIGHT: f32 = 84.0;

pub fn render(onboarding: &Entity<Onboarding>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (step, mode, can_continue, notice) = {
        let me = onboarding.read(cx);
        (
            me.step(),
            me.mode(),
            me.can_continue(cx),
            me.notice().map(str::to_owned),
        )
    };
    let continue_focus = onboarding.read(cx).continue_focus.clone();
    let body = match step {
        Step::Permissions => permissions(palette, onboarding, cx).into_any_element(),
        Step::MicTest => mic_test(palette, onboarding, cx).into_any_element(),
        Step::Model => model(palette, onboarding, cx).into_any_element(),
        Step::Practice => practice(palette, onboarding, cx).into_any_element(),
        Step::Updates => updates(palette, onboarding, cx).into_any_element(),
    };
    let last = step == Step::Updates && mode == Mode::Setup;
    let label = if last { "Finish" } else { "Continue" };
    let advance = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                // A refused Continue has nothing to say: the disabled button already did.
                let _ = me.advance(cx);
            });
        }
    };
    div()
        .id(hook::id("onboarding", "page"))
        .test_support()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .w(px(space::ONBOARDING_WIDTH))
        .max_w_full()
        .mx_auto()
        .pt(px(space::SM))
        .pb(px(space::XXL))
        .gap(px(space::LG))
        .child(header(palette, step, mode))
        .child(body)
        .children(notice.map(|message| {
            ui::group(
                hook::id("onboarding", "notice.group"),
                palette,
                vec![ui::notice_row(
                    palette,
                    hook::id("onboarding", "notice"),
                    message,
                    Notice::Alert,
                    None,
                )],
            )
        }))
        .child(div().flex().justify_end().child(ui::button(
            palette,
            hook::id("onboarding", "continue"),
            label,
            Kind::Default,
            Some(&continue_focus),
            can_continue,
            advance,
        )))
}

fn header(palette: Palette, step: Step, mode: Mode) -> impl IntoElement + use<> {
    let (eyebrow, title, detail) = if mode == Mode::Repair {
        (
            "Permissions changed".to_owned(),
            "Permissions".to_owned(),
            "Hushpen lost something it was allowed to use before. Allow it again to go on."
                .to_owned(),
        )
    } else {
        (
            format!("Step {} of {}", step.index() + 1, Step::ALL.len()),
            step.title().to_owned(),
            step_detail(step).to_owned(),
        )
    };
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(hook::id("onboarding", "progress"))
                .test_support()
                .aria_label(eyebrow.clone())
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .child(eyebrow),
        )
        .child(
            div()
                .id(hook::id("onboarding", "title"))
                .test_support()
                .aria_label(title.clone())
                .text_token(TITLE)
                .text_color(palette.label)
                .child(title),
        )
        .child(
            div()
                .id(hook::id("onboarding", "detail"))
                .test_support()
                .text_token(BODY)
                .text_color(palette.label_secondary)
                .child(detail),
        )
}

fn step_detail(step: Step) -> &'static str {
    match step {
        Step::Permissions => {
            "Hushpen needs a few things from your system. Each line shows where it stands now."
        }
        Step::MicTest => {
            "Press Test, say a sentence, then press Stop. You will see the level move and then your words."
        }
        Step::Model => {
            "Hushpen turns speech into text on this computer. It needs one model file. The file is checked before it is used."
        }
        Step::Practice => {
            "Try your dictation shortcut here. The shortcuts turn on for other apps once this works."
        }
        Step::Updates => {
            "Choose if Hushpen may look for new versions. You can change this later in Settings."
        }
    }
}

fn permissions(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (rows, focus, relaunch_focus) = {
        let me = onboarding.read(cx);
        (
            me.rows(cx),
            me.permission_focus.clone(),
            me.relaunch_focus.clone(),
        )
    };
    let rows = rows
        .into_iter()
        .map(|row| {
            let key = row.key.key();
            let hint = row
                .opens
                .filter(|_| row.state != RowState::Ready)
                .and_then(|permission| onboarding.read(cx).hint(permission));
            let line = permission_row(palette, onboarding, row, &focus, &relaunch_focus);
            match hint {
                Some(text) => div()
                    .w_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(line)
                    .child(
                        ui::secondary(palette)
                            .id(hook::id("onboarding", &format!("permissions.hint.{key}")))
                            .test_support()
                            .px(px(size::ROW_PADDING))
                            .pb(px(space::SM))
                            .child(text),
                    )
                    .into_any_element(),
                None => line.into_any_element(),
            }
        })
        .collect();
    ui::group(hook::id("onboarding", "permissions"), palette, rows)
}

/// What the status mark of a row that needs the user says as a tooltip and to a screen reader.
const ATTENTION: &str = "Needs attention";

/// The status of a row: a word, or for a row that needs attention a warning icon with a
/// tooltip and no word. The hook tree and screen readers get the same text.
fn status(
    palette: Palette,
    id: gpui_kit::SharedString,
    text: &'static str,
    tone: Rgba,
    attention: bool,
) -> impl IntoElement + use<> {
    if attention {
        div()
            .id(id)
            .test_support()
            .role(Role::Image)
            .aria_label(text)
            .flex_none()
            .child(
                svg()
                    .path(IconName::CircleAlert.path())
                    .size(px(size::NAV_ICON))
                    .flex_none()
                    .text_color(tone),
            )
            .tooltip(move |window, cx| Tooltip::new(text).build(window, cx))
            .into_any_element()
    } else {
        ui::value(palette)
            .id(id)
            .test_support()
            .aria_label(text)
            .text_color(tone)
            .child(text)
            .into_any_element()
    }
}

fn permission_row(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    row: Row,
    focus: &[FocusHandle],
    relaunch_focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let key = row.key.key();
    let (state_text, tone) = match row.state {
        RowState::Ready => ("Ready", palette.success),
        RowState::Missing => (ATTENTION, palette.warning),
        RowState::Unavailable => ("Not available", palette.label_secondary),
    };
    let icon = match row.key {
        hushpen_core::onboarding::RowKey::Microphone => IconName::Mic,
        hushpen_core::onboarding::RowKey::X11Session => IconName::Monitor,
        _ => IconName::Keyboard,
    };
    let ready = row.state == RowState::Ready;
    let buttons = row.opens.map(|permission| {
        let allow = {
            let onboarding = onboarding.clone();
            move |_: &mut Window, cx: &mut App| {
                onboarding.update(cx, |me, cx| me.allow(permission, cx))
            }
        };
        let open = {
            let onboarding = onboarding.clone();
            move |_: &mut Window, cx: &mut App| {
                onboarding.update(cx, |me, cx| me.open_settings(permission, cx))
            }
        };
        (permission, allow, open)
    });
    let relaunch = row.relaunch.then(|| {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| onboarding.update(cx, |me, cx| me.relaunch(cx))
    });
    let detail = ui::secondary(palette)
        .id(hook::id("onboarding", &format!("permissions.detail.{key}")))
        .test_support()
        .child(row.detail);
    ui::icon_row(
        palette,
        icon,
        ui::title(palette).child(row.title),
        Some(detail.into_any_element()),
    )
    .id(hook::id("onboarding", &format!("permissions.row.{key}")))
    .test_support()
    .aria_label(format!("{}: {state_text}", row.title))
    .child(status(
        palette,
        hook::id("onboarding", &format!("permissions.state.{key}")),
        state_text,
        tone,
        row.state == RowState::Missing,
    ))
    .children(buttons.map(|(permission, allow, open)| {
        let slot = Permission::ALL
            .iter()
            .position(|candidate| *candidate == permission)
            .unwrap_or(0);
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(space::SM))
            // Allow asks the system, so it has nothing left to do once the row is Ready.
            .children((!ready).then(|| {
                ui::button(
                    palette,
                    hook::id("onboarding", &format!("permissions.allow.{key}")),
                    "Allow",
                    Kind::Default,
                    Some(&focus[slot * 2]),
                    true,
                    allow,
                )
            }))
            .child(ui::button(
                palette,
                hook::id("onboarding", &format!("permissions.open.{key}")),
                "Open Settings",
                Kind::Plain,
                Some(&focus[slot * 2 + 1]),
                true,
                open,
            ))
    }))
    .children(relaunch.map(|press| {
        ui::button(
            palette,
            hook::id("onboarding", "permissions.relaunch"),
            "Quit and Reopen",
            Kind::Default,
            Some(relaunch_focus),
            true,
            press,
        )
    }))
}

fn mic_test(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (test, meter, focus, models_ready, default_name) = {
        let me = onboarding.read(cx);
        let mic = me.mic.read(cx);
        (
            me.mic_test().clone(),
            mic.meter(),
            me.mic_focus.clone(),
            me.model_ready(cx),
            mic.default_name().map(str::to_owned),
        )
    };
    let listening = test.phase == MicPhase::Listening;
    let checking = test.phase == MicPhase::Checking;
    let label = if listening { "Stop" } else { "Test" };
    let status = match test.phase {
        MicPhase::Listening => "Listening".to_owned(),
        MicPhase::Checking => "Reading your words".to_owned(),
        MicPhase::Idle if test.passed => "Passed".to_owned(),
        MicPhase::Idle => default_name.map_or_else(
            || "Default microphone".to_owned(),
            |name| format!("Default: {name}"),
        ),
    };
    let toggle = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                // A refused start shows its reason in the notice row.
                let _ = me.toggle_mic_test(cx);
            });
        }
    };
    let (result_id, result_text, result_tone): (&str, String, Rgba) = match &test.verdict {
        Some(MicVerdict::Passed {
            transcript: Some(text),
        }) => ("transcript", text.clone(), palette.label),
        Some(MicVerdict::Passed { transcript: None }) => {
            ("note", NOTE_NO_MODEL.to_owned(), palette.label_secondary)
        }
        Some(MicVerdict::NoSound) => ("hint", HINT_NO_SOUND.to_owned(), palette.label),
        Some(MicVerdict::NoSpeech) => ("hint", HINT_NO_SPEECH.to_owned(), palette.label),
        Some(MicVerdict::TranscriptFailed) => {
            ("hint", HINT_TRANSCRIPT_FAILED.to_owned(), palette.label)
        }
        None if models_ready => (
            "idle",
            "Your words will show here.".to_owned(),
            palette.label_secondary,
        ),
        None => (
            "idle",
            "Words show here after the speech model is downloaded. The level works now.".to_owned(),
            palette.label_secondary,
        ),
    };
    let status = ui::secondary(palette)
        .id(hook::id("onboarding", "mic.status"))
        .test_support()
        .role(Role::Status)
        .aria_label(status.clone())
        .child(status);
    let rows: Vec<AnyElement> = vec![
        ui::icon_row(
            palette,
            IconName::Mic,
            ui::title(palette).child("Microphone"),
            Some(status.into_any_element()),
        )
        .child(ui::button(
            palette,
            hook::id("onboarding", "mic.test"),
            label,
            Kind::Plain,
            Some(&focus),
            !checking,
            toggle,
        ))
        .into_any_element(),
        ui::meter_row(palette, "onboarding", &meter, listening),
        div()
            .id(hook::id("onboarding", &format!("mic.{result_id}")))
            .test_support()
            .role(Role::Status)
            .aria_label(result_text.clone())
            .w_full()
            .flex_none()
            .px(px(size::ROW_PADDING))
            .py(px(space::MD))
            .text_token(BODY)
            .text_color(result_tone)
            .child(result_text)
            .into_any_element(),
    ];
    ui::group(hook::id("onboarding", "mic"), palette, rows)
}

fn model(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (info, state, focus, notice) = {
        let me = onboarding.read(cx);
        (
            me.model_info(cx),
            me.model_state(cx),
            me.download_focus.clone(),
            me.model_notice(cx),
        )
    };
    let (name, bytes) = info.unwrap_or_else(|| ("Speech model".to_owned(), 0));
    let state = state.unwrap_or(ModelState::NotDownloaded);
    let (status, tone): (String, Rgba) = match state {
        ModelState::NotDownloaded => ("Not downloaded".into(), palette.label_secondary),
        ModelState::Downloading { .. } => (
            state
                .percent()
                .map_or_else(|| "Starting".into(), |percent| format!("{percent}%")),
            palette.label,
        ),
        ModelState::Verifying => ("Checking".into(), palette.label_secondary),
        ModelState::Ready => ("Ready".into(), palette.success),
        ModelState::Failed => ("Failed verification".into(), palette.error),
    };
    let fraction = state
        .percent()
        .map_or(0.0, |percent| percent as f32 / 100.0);
    let downloading = matches!(state, ModelState::Downloading { .. });
    let press = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                // A refused action shows its reason in the notice row.
                let _ = if downloading {
                    me.cancel_model(cx)
                } else {
                    me.download_model(cx)
                };
            });
        }
    };
    let action = match state {
        ModelState::Ready | ModelState::Verifying => None,
        _ if downloading => Some(("Cancel", "cancel")),
        _ => Some(("Download", "download")),
    };
    let mut rows: Vec<AnyElement> = Vec::new();
    if let Some(message) = notice {
        rows.push(ui::notice_row(
            palette,
            hook::id("onboarding", "model.notice"),
            message,
            Notice::Alert,
            None,
        ));
    }
    let size_line = ui::secondary(palette).child(format!("{} · Recommended", format_size(bytes)));
    rows.push(
        ui::icon_row(
            palette,
            IconName::Cpu,
            ui::title(palette)
                .id(hook::id("onboarding", "model.name"))
                .test_support()
                .child(name),
            Some(size_line.into_any_element()),
        )
        .child(
            ui::value(palette)
                .id(hook::id("onboarding", "model.status"))
                .test_support()
                .role(Role::Status)
                .aria_label(status.clone())
                .text_color(tone)
                .child(status),
        )
        .children(action.map(|(label, id)| {
            ui::button(
                palette,
                hook::id("onboarding", &format!("model.{id}")),
                label,
                Kind::Plain,
                Some(&focus),
                true,
                press,
            )
        }))
        .into_any_element(),
    );
    rows.push(
        div()
            .w_full()
            .flex_none()
            .px(px(size::ROW_PADDING))
            .py(px(space::MD))
            .child(
                ui::track(
                    palette,
                    if state == ModelState::Ready {
                        1.0
                    } else {
                        fraction
                    },
                )
                .id(hook::id("onboarding", "model.progress"))
                .test_support()
                .role(Role::ProgressIndicator),
            )
            .into_any_element(),
    );
    ui::group(hook::id("onboarding", "model"), palette, rows)
}

fn practice(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (text, hold, keys_ready, listening, busy, can_record, field_focus, record_focus) = {
        let me = onboarding.read(cx);
        (
            me.practice_text().map(str::to_owned),
            me.hold_key(cx),
            me.keys_ready(cx),
            me.practice_listening(cx),
            me.practice_busy(cx),
            me.can_practice_record(cx),
            me.practice_focus.clone(),
            me.record_focus.clone(),
        )
    };
    let passed = text.is_some();
    let instruction = if keys_ready {
        format!("Hold {hold}, say a sentence, then let go.")
    } else {
        "The hold key is not available on this system. Use the Record button instead.".to_owned()
    };
    let status = if passed {
        "That worked. The shortcuts are on now."
    } else if listening {
        "Listening"
    } else if busy {
        "Reading your words"
    } else if !can_record {
        "Download the speech model first."
    } else {
        "Waiting for your voice"
    };
    let toggle = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                let _ = me.toggle_practice_record(cx);
            });
        }
    };
    let shown = text.clone().unwrap_or_default();
    let status = ui::secondary(palette)
        .id(hook::id("onboarding", "practice.status"))
        .test_support()
        .role(Role::Status)
        .aria_label(status)
        .child(status);
    let rows: Vec<AnyElement> = vec![
        ui::icon_row(
            palette,
            IconName::Keyboard,
            ui::title(palette)
                .id(hook::id("onboarding", "practice.instruction"))
                .test_support()
                .child(instruction),
            Some(status.into_any_element()),
        )
        .child(ui::button(
            palette,
            hook::id("onboarding", "practice.record"),
            if listening { "Stop" } else { "Record" },
            Kind::Plain,
            Some(&record_focus),
            can_record || listening,
            toggle,
        ))
        .into_any_element(),
        div()
            .w_full()
            .flex_none()
            .p(px(size::ROW_PADDING))
            .child(
                div()
                    .id(hook::id("onboarding", "practice.field"))
                    .test_support()
                    .track_focus(&field_focus)
                    .role(Role::Paragraph)
                    .aria_label(if shown.is_empty() {
                        "Practice field, empty".to_owned()
                    } else {
                        shown.clone()
                    })
                    .w_full()
                    .min_h(px(FIELD_MIN_HEIGHT))
                    .p(px(space::SM - size::FOCUS_RING))
                    .rounded(px(radius::BUTTON))
                    .border_2()
                    .border_color(transparent_black())
                    .bg(palette.control)
                    .text_token(BODY)
                    .text_color(if passed {
                        palette.label
                    } else {
                        palette.label_secondary
                    })
                    .focus_visible(move |style| style.border_color(palette.accent))
                    .child(if passed {
                        shown
                    } else {
                        "Your words will show here.".to_owned()
                    }),
            )
            .into_any_element(),
    ];
    ui::group(hook::id("onboarding", "practice"), palette, rows)
}

fn updates(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (on, focus) = {
        let me = onboarding.read(cx);
        (me.updates_on(), me.updates_focus.clone())
    };
    let flip = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                let on = !me.updates_on();
                me.set_updates(on, cx);
            });
        }
    };
    let detail =
        ui::secondary(palette).child("Off unless you turn it on. Hushpen never updates by itself.");
    let rows: Vec<AnyElement> = vec![
        ui::icon_row(
            palette,
            IconName::Download,
            ui::title(palette).child("Check for updates"),
            Some(detail.into_any_element()),
        )
        .child(ui::switch(
            palette,
            hook::id("onboarding", "updates.toggle"),
            "Check for updates",
            on,
            true,
            &focus,
            flip,
        ))
        .into_any_element(),
    ];
    ui::group(hook::id("onboarding", "updates"), palette, rows)
}
