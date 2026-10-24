//! The microphone step: the test button, the level meter, and what the engine heard.

use crate::onboarding::{MicPhase, Onboarding};

use crate::hook;
use crate::theme::{BODY, Palette, StyledType, size, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Rgba, Role,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use hushpen_core::onboarding::{
    HINT_NO_SOUND, HINT_NO_SPEECH, HINT_TRANSCRIPT_FAILED, MicVerdict, NOTE_NO_MODEL,
};

pub(super) fn view(
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
