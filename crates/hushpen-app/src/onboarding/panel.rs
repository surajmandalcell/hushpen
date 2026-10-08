//! The onboarding page: a step header, one grouped panel for the step, and the Continue button.
//! There is no skip control and no way back: each step has to pass before the next one opens.

use super::{MicPhase, Mode, Onboarding};
use crate::hook;
use crate::mic::METER_BARS;
use crate::models::ModelState;
use crate::models::panel::format_size;
use crate::theme::{
    self, BODY_MD, BODY_SM, DISPLAY_MD, LABEL_CAPS, LABEL_MD, ROW_TITLE, StyledType, color, radius,
    size, space,
};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative, svg,
    transparent_black,
};
use hushpen_core::onboarding::{
    HINT_NO_SOUND, HINT_NO_SPEECH, HINT_TRANSCRIPT_FAILED, MicVerdict, NOTE_NO_MODEL, Row,
    RowState, Step,
};
use hushpen_core::permission::Permission;

const BAR_WIDTH: f32 = 4.0;
const BAR_GAP: f32 = 3.0;
const BAR_REST: f32 = 3.0;
const BAR_MAX: f32 = 28.0;
const FIELD_MIN_HEIGHT: f32 = 84.0;

pub fn render(onboarding: &Entity<Onboarding>, cx: &mut App) -> impl IntoElement + use<> {
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
        Step::Permissions => permissions(onboarding, cx).into_any_element(),
        Step::MicTest => mic_test(onboarding, cx).into_any_element(),
        Step::Model => model(onboarding, cx).into_any_element(),
        Step::Practice => practice(onboarding, cx).into_any_element(),
        Step::Updates => updates(onboarding, cx).into_any_element(),
    };
    let last = step == Step::Updates && mode == Mode::Setup;
    let label = if last { "Finish" } else { "Continue" };
    let advance = {
        let onboarding = onboarding.clone();
        move |cx: &mut App| {
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
        .w_full()
        .max_w(px(space::CONTENT_MAX_WIDTH))
        .mx_auto()
        .px(px(space::XL))
        .pb(px(space::XL))
        .gap(px(space::LG))
        .child(header(step, mode))
        .child(body)
        .children(notice.map(notice_row))
        .child(div().flex().justify_end().child(button(
            hook::id("onboarding", "continue"),
            label,
            &continue_focus,
            can_continue,
            advance,
        )))
}

fn header(step: Step, mode: Mode) -> impl IntoElement + use<> {
    let (eyebrow, title, detail) = if mode == Mode::Repair {
        (
            "PERMISSIONS CHANGED".to_owned(),
            "Permissions".to_owned(),
            "Hushpen lost something it was allowed to use before. Allow it again to go on."
                .to_owned(),
        )
    } else {
        (
            format!("STEP {} OF {}", step.index() + 1, Step::ALL.len()),
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
                .text_token(LABEL_CAPS)
                .text_color(theme::rgb_of(color::TEXT_SUBTLE))
                .child(eyebrow),
        )
        .child(
            div()
                .id(hook::id("onboarding", "title"))
                .test_support()
                .aria_label(title.clone())
                .text_token(DISPLAY_MD)
                .text_color(theme::rgb_of(color::TEXT_HEADING))
                .child(title),
        )
        .child(
            div()
                .id(hook::id("onboarding", "detail"))
                .test_support()
                .text_token(BODY_MD)
                .text_color(theme::rgb_of(color::TEXT_MUTED))
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

fn surface(id: SharedString) -> impl IntoElement + ParentElement + Styled {
    div()
        .id(id)
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
}

fn permissions(onboarding: &Entity<Onboarding>, cx: &mut App) -> impl IntoElement + use<> {
    let (rows, focus) = {
        let me = onboarding.read(cx);
        (me.rows(cx), me.permission_focus.clone())
    };
    let mut panel = surface(hook::id("onboarding", "permissions"));
    for (index, row) in rows.into_iter().enumerate() {
        panel = panel.child(permission_row(onboarding, index, row, &focus));
    }
    panel
}

fn permission_row(
    onboarding: &Entity<Onboarding>,
    index: usize,
    row: Row,
    focus: &[FocusHandle],
) -> impl IntoElement + use<> {
    let key = row.key.key();
    let (state_text, tone) = match row.state {
        RowState::Ready => ("Ready", color::FOCUS),
        RowState::Missing => ("Needs your help", color::STATUS_WARNING),
        RowState::Unavailable => ("Not available", color::TEXT_SUBTLE),
    };
    let icon = match row.key {
        hushpen_core::onboarding::RowKey::Microphone => IconName::Mic,
        hushpen_core::onboarding::RowKey::X11Session => IconName::Monitor,
        _ => IconName::Keyboard,
    };
    let open = row.opens.map(|permission| {
        let onboarding = onboarding.clone();
        (permission, move |cx: &mut App| {
            onboarding.update(cx, |me, cx| me.open_permission(permission, cx));
        })
    });
    let mut container = div()
        .id(hook::id("onboarding", &format!("permissions.row.{key}")))
        .test_support()
        .aria_label(format!("{}: {state_text}", row.title))
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD));
    if index > 0 {
        container = container
            .border_t_1()
            .border_color(theme::rgb_of(color::DIVIDER));
    }
    container
        .child(icon_tile(icon))
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
                        .id(hook::id("onboarding", &format!("permissions.detail.{key}")))
                        .test_support()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .child(row.detail),
                ),
        )
        .child(
            div()
                .id(hook::id("onboarding", &format!("permissions.state.{key}")))
                .test_support()
                .flex_none()
                .whitespace_nowrap()
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(tone))
                .child(state_text),
        )
        .children(open.map(|(permission, press)| {
            let slot = Permission::ALL
                .iter()
                .position(|candidate| *candidate == permission)
                .unwrap_or(0);
            button(
                hook::id("onboarding", &format!("permissions.open.{key}")),
                "Open Settings",
                &focus[slot],
                true,
                press,
            )
        }))
}

fn mic_test(onboarding: &Entity<Onboarding>, cx: &mut App) -> impl IntoElement + use<> {
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
        move |cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                // A refused start shows its reason in the notice row.
                let _ = me.toggle_mic_test(cx);
            });
        }
    };
    let (result_id, result_text, result_tone) = match &test.verdict {
        Some(MicVerdict::Passed {
            transcript: Some(text),
        }) => ("transcript", text.clone(), color::TEXT_PRIMARY),
        Some(MicVerdict::Passed { transcript: None }) => {
            ("note", NOTE_NO_MODEL.to_owned(), color::TEXT_MUTED)
        }
        Some(MicVerdict::NoSound) => ("hint", HINT_NO_SOUND.to_owned(), color::STATUS_ERROR),
        Some(MicVerdict::NoSpeech) => ("hint", HINT_NO_SPEECH.to_owned(), color::STATUS_ERROR),
        Some(MicVerdict::TranscriptFailed) => (
            "hint",
            HINT_TRANSCRIPT_FAILED.to_owned(),
            color::STATUS_ERROR,
        ),
        None if models_ready => (
            "idle",
            "Your words will show here.".to_owned(),
            color::TEXT_SUBTLE,
        ),
        None => (
            "idle",
            "Words show here after the speech model is downloaded. The level works now.".to_owned(),
            color::TEXT_SUBTLE,
        ),
    };
    surface(hook::id("onboarding", "mic"))
        .child(
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
                                .id(hook::id("onboarding", "mic.status"))
                                .test_support()
                                .role(Role::Status)
                                .aria_label(status.clone())
                                .text_token(BODY_SM)
                                .text_color(theme::rgb_of(color::TEXT_MUTED))
                                .truncate()
                                .child(status),
                        ),
                )
                .child(button(
                    hook::id("onboarding", "mic.test"),
                    label,
                    &focus,
                    !checking,
                    toggle,
                )),
        )
        .child(meter_row(&meter, listening))
        .child(
            div()
                .id(hook::id("onboarding", &format!("mic.{result_id}")))
                .test_support()
                .role(Role::Status)
                .aria_label(result_text.clone())
                .px(px(space::LG))
                .py(px(space::MD))
                .border_t_1()
                .border_color(theme::rgb_of(color::DIVIDER))
                .text_token(BODY_MD)
                .text_color(theme::rgb_of(result_tone))
                .child(result_text),
        )
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
                .id(hook::indexed("onboarding", "mic.bar", index))
                .test_support()
                .flex_none()
                .w(px(BAR_WIDTH))
                .h(px(BAR_REST + level.clamp(0.0, 1.0) * (BAR_MAX - BAR_REST)))
                .rounded(px(radius::XS))
                .bg(theme::rgb_of(bar_color))
        });
    div()
        .id(hook::id("onboarding", "mic.meter"))
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

fn model(onboarding: &Entity<Onboarding>, cx: &mut App) -> impl IntoElement + use<> {
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
    let (status, tone): (String, u32) = match state {
        ModelState::NotDownloaded => ("Not downloaded".into(), color::TEXT_SUBTLE),
        ModelState::Downloading { .. } => (
            state
                .percent()
                .map_or_else(|| "Starting".into(), |percent| format!("{percent}%")),
            color::TEXT_BODY,
        ),
        ModelState::Verifying => ("Checking".into(), color::TEXT_MUTED),
        ModelState::Ready => ("Ready".into(), color::FOCUS),
        ModelState::Failed => ("Failed verification".into(), color::STATUS_ERROR),
    };
    let fraction = state
        .percent()
        .map_or(0.0, |percent| percent as f32 / 100.0);
    let downloading = matches!(state, ModelState::Downloading { .. });
    let press = {
        let onboarding = onboarding.clone();
        move |cx: &mut App| {
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
    let mut panel = surface(hook::id("onboarding", "model"));
    if let Some(message) = notice {
        panel = panel.child(
            div()
                .id(hook::id("onboarding", "model.notice"))
                .test_support()
                .role(Role::Alert)
                .px(px(space::LG))
                .py(px(space::MD))
                .border_b_1()
                .border_color(theme::rgb_of(color::DIVIDER))
                .text_token(BODY_MD)
                .text_color(theme::rgb_of(color::STATUS_ERROR))
                .child(message),
        );
    }
    panel
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .p(px(space::LG))
                .child(icon_tile(IconName::Cpu))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .id(hook::id("onboarding", "model.name"))
                                .test_support()
                                .text_token(ROW_TITLE)
                                .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                                .truncate()
                                .child(name),
                        )
                        .child(
                            div()
                                .text_token(BODY_SM)
                                .text_color(theme::rgb_of(color::TEXT_MUTED))
                                .child(format!("{} · recommended", format_size(bytes))),
                        ),
                )
                .child(
                    div()
                        .id(hook::id("onboarding", "model.status"))
                        .test_support()
                        .role(Role::Status)
                        .aria_label(status.clone())
                        .flex_none()
                        .whitespace_nowrap()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(tone))
                        .child(status),
                )
                .children(action.map(|(label, id)| {
                    button(
                        hook::id("onboarding", &format!("model.{id}")),
                        label,
                        &focus,
                        true,
                        press,
                    )
                })),
        )
        .child(
            div()
                .id(hook::id("onboarding", "model.progress"))
                .test_support()
                .role(Role::ProgressIndicator)
                .h(px(4.0))
                .mx(px(space::LG))
                .mb(px(space::LG))
                .rounded(px(radius::FULL))
                .bg(theme::rgb_of(color::SURFACE_CONTROL))
                .child(
                    div()
                        .h_full()
                        .w(relative(if state == ModelState::Ready {
                            1.0
                        } else {
                            fraction
                        }))
                        .rounded(px(radius::FULL))
                        .bg(theme::rgb_of(color::ACCENT_BLUE)),
                ),
        )
}

fn practice(onboarding: &Entity<Onboarding>, cx: &mut App) -> impl IntoElement + use<> {
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
        move |cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                let _ = me.toggle_practice_record(cx);
            });
        }
    };
    let shown = text.clone().unwrap_or_default();
    surface(hook::id("onboarding", "practice"))
        .child(
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
                                .id(hook::id("onboarding", "practice.instruction"))
                                .test_support()
                                .text_token(ROW_TITLE)
                                .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                                .child(instruction),
                        )
                        .child(
                            div()
                                .id(hook::id("onboarding", "practice.status"))
                                .test_support()
                                .role(Role::Status)
                                .aria_label(status)
                                .text_token(BODY_SM)
                                .text_color(theme::rgb_of(color::TEXT_MUTED))
                                .child(status),
                        ),
                )
                .child(button(
                    hook::id("onboarding", "practice.record"),
                    if listening { "Stop" } else { "Record" },
                    &record_focus,
                    can_record || listening,
                    toggle,
                )),
        )
        .child(
            div().px(px(space::LG)).pb(px(space::LG)).child(
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
                    .p(px(space::MD - size::FOCUS_RING))
                    .rounded(px(radius::MD))
                    .border_2()
                    .border_color(transparent_black())
                    .bg(theme::rgb_of(color::SURFACE_CONTROL))
                    .text_token(BODY_MD)
                    .text_color(theme::rgb_of(if passed {
                        color::TEXT_PRIMARY
                    } else {
                        color::TEXT_SUBTLE
                    }))
                    .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
                    .child(if passed {
                        shown
                    } else {
                        "Your words will show here.".to_owned()
                    }),
            ),
        )
}

fn updates(onboarding: &Entity<Onboarding>, cx: &mut App) -> impl IntoElement + use<> {
    let (on, focus) = {
        let me = onboarding.read(cx);
        (me.updates_on(), me.updates_focus.clone())
    };
    let flip = {
        let onboarding = onboarding.clone();
        move |cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                let on = !me.updates_on();
                me.set_updates(on, cx);
            });
        }
    };
    let on_click = {
        let flip = flip.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| flip(cx)
    };
    let knob = div()
        .size(px(14.0))
        .rounded(px(radius::FULL))
        .bg(theme::rgb_of(color::TEXT_PRIMARY));
    let track = div()
        .w(px(36.0))
        .h(px(20.0))
        .flex_none()
        .flex()
        .items_center()
        .px(px(3.0))
        .rounded(px(radius::FULL))
        .border_1()
        .border_color(theme::rgb_of(color::BORDER_CONTROL))
        .bg(theme::rgb_of(if on {
            color::ACCENT_BLUE
        } else {
            color::SURFACE_CONTROL
        }));
    let track = if on {
        track.justify_end().child(knob)
    } else {
        track.justify_start().child(knob)
    };
    surface(hook::id("onboarding", "updates")).child(
        div()
            .flex()
            .items_center()
            .gap(px(space::MD))
            .p(px(space::LG))
            .child(icon_tile(IconName::Download))
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
                            .child("Check for updates"),
                    )
                    .child(
                        div()
                            .text_token(BODY_SM)
                            .text_color(theme::rgb_of(color::TEXT_MUTED))
                            .child("Off unless you turn it on. Hushpen never updates by itself."),
                    ),
            )
            .child(
                div()
                    .id(hook::id("onboarding", "updates.toggle"))
                    .test_support()
                    .track_focus(&focus)
                    .role(Role::Switch)
                    .aria_label(if on {
                        "Check for updates, on"
                    } else {
                        "Check for updates, off"
                    })
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(space::SM))
                    .h(px(32.0))
                    .px(px(space::SM - size::FOCUS_RING))
                    .rounded(px(radius::MD))
                    .border_2()
                    .border_color(transparent_black())
                    .text_token(LABEL_MD)
                    .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                    .cursor_pointer()
                    .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
                    .on_click(on_click)
                    .child(track)
                    .child(if on { "On" } else { "Off" }),
            ),
    )
}

fn notice_row(message: String) -> impl IntoElement {
    div()
        .id(hook::id("onboarding", "notice"))
        .test_support()
        .role(Role::Alert)
        .flex()
        .items_center()
        .gap(px(space::SM))
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
    id: SharedString,
    label: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    press: F,
) -> impl IntoElement + use<F> {
    hook::mark_disabled(&id, !enabled);
    // A disabled button keeps its focus handle. When the step changes or the button turns off
    // under the user's finger, the focus stays in the page instead of falling out of the window.
    let mut element = div()
        .id(id)
        .test_support()
        .track_focus(focus)
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
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
        .text_color(theme::rgb_of(if enabled {
            color::TEXT_PRIMARY
        } else {
            color::TEXT_DISABLED
        }));
    if enabled {
        let on_click = {
            let press = press.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| press(cx)
        };
        element = element
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .on_click(on_click);
    }
    element.child(label)
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
