use super::*;
use crate::controller::testkit::*;
use crate::engine_host::LoadProblem;
use gpui_kit::TestAppContext;
use hushpen_audio::CaptureError;
use hushpen_core::dictation::AppEvent;
use hushpen_engine::{Failure, JobOutcome};
use std::path::PathBuf;

fn act<T>(
    cx: &mut TestAppContext,
    rig: &Rig,
    run: impl FnOnce(&mut Dictation, &mut Context<Dictation>) -> T,
) -> T {
    rig.dictation.update(cx, run)
}

fn phase(cx: &mut TestAppContext, rig: &Rig) -> Phase {
    cx.update(|cx| rig.dictation.read(cx).phase(cx))
}

fn state(cx: &mut TestAppContext, rig: &Rig) -> Value {
    cx.update(|cx| rig.dictation.read(cx).state_json(cx))
}

/// One full run from the record button: start, stop, and let the engine answer.
fn dictate(cx: &mut TestAppContext, rig: &Rig, outcome: JobOutcome) {
    say(rig, outcome);
    act(cx, rig, |me, cx| me.toggle(cx)).unwrap();
    act(cx, rig, |me, cx| me.toggle(cx)).unwrap();
    settle(cx, rig);
}

#[gpui_kit::test]
fn a_dictation_goes_from_listening_to_transcribing_to_done_and_copies_the_words(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("the quick brown fox", "en"));

    act(cx, &rig, |me, cx| me.toggle(cx)).unwrap();
    assert_eq!(phase(cx, &rig), Phase::Listening);
    act(cx, &rig, |me, cx| me.toggle(cx)).unwrap();
    assert_eq!(phase(cx, &rig), Phase::Transcribing);
    settle(cx, &rig);

    assert_eq!(phase(cx, &rig), Phase::Done);
    let state = state(cx, &rig);
    assert_eq!(state["transcript"], "the quick brown fox");
    assert_eq!(state["language"], "en");
    assert_eq!(clipboard(cx).as_deref(), Some("the quick brown fox"));
}

#[gpui_kit::test]
fn blank_markers_never_reach_the_transcript_or_the_clipboard(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    dictate(
        cx,
        &rig,
        outcome_text("[BLANK_AUDIO] hello there (silence) [MUSIC]", "en"),
    );
    assert_eq!(state(cx, &rig)["transcript"], "hello there");
    assert_eq!(clipboard(cx).as_deref(), Some("hello there"));
}

#[gpui_kit::test]
fn silence_gives_no_speech_and_leaves_the_clipboard_and_old_text_alone(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    dictate(cx, &rig, outcome_text("first words", "en"));
    put_on_clipboard(cx, "OLD");

    dictate(cx, &rig, outcome_text("[BLANK_AUDIO]", "en"));

    assert_eq!(phase(cx, &rig), Phase::NoSpeech);
    let state = state(cx, &rig);
    assert_eq!(state["transcript"], "");
    assert_eq!(state["last_result"], "no-speech");
    assert_eq!(state["notice"]["code"], "ENGINE_NO_SPEECH");
    assert_eq!(clipboard(cx).as_deref(), Some("OLD"));
}

#[gpui_kit::test]
fn an_engine_no_speech_error_is_the_same_state(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    dictate(
        cx,
        &rig,
        JobOutcome::Failed(Failure::new("ENGINE_NO_SPEECH", "no audio")),
    );
    assert_eq!(phase(cx, &rig), Phase::NoSpeech);
    assert_eq!(state(cx, &rig)["notice"]["code"], "ENGINE_NO_SPEECH");
}

#[gpui_kit::test]
fn a_long_dictation_keeps_every_word(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let words: Vec<String> = (0..160).map(|n| format!("word{n}")).collect();
    let text = format!("{} the final word is lighthouse", words.join(" "));
    dictate(cx, &rig, outcome_text(&text, "en"));
    assert_eq!(state(cx, &rig)["transcript"], text.as_str());
    assert_eq!(clipboard(cx), Some(text));
}

#[gpui_kit::test]
fn a_failed_job_clears_old_words_keeps_the_recording_and_the_next_run_works(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    dictate(cx, &rig, outcome_text("earlier words", "en"));

    dictate(
        cx,
        &rig,
        JobOutcome::Failed(Failure::new("ENGINE_CRASHED", "the engine died")),
    );
    assert_eq!(phase(cx, &rig), Phase::Failed);
    let failed = state(cx, &rig);
    assert_eq!(failed["transcript"], "");
    assert_eq!(failed["notice"]["code"], "ENGINE_CRASHED");
    let wav = PathBuf::from(failed["session"].as_str().unwrap());
    assert!(wav.exists(), "the recording of a failed job is kept");

    dictate(cx, &rig, outcome_text("later words", "en"));
    assert_eq!(phase(cx, &rig), Phase::Done);
    assert_eq!(state(cx, &rig)["transcript"], "later words");
}

#[gpui_kit::test]
fn the_language_setting_goes_to_the_engine_and_auto_means_detect(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    dictate(cx, &rig, outcome_text("hello", "en"));
    act(cx, &rig, |me, cx| me.set_language("es", cx)).unwrap();
    dictate(cx, &rig, outcome_text("hola", "es"));

    let specs = rig.specs.lock().unwrap();
    assert_eq!(specs[0].language, None);
    assert_eq!(specs[1].language.as_deref(), Some("es"));
    drop(specs);
    assert_eq!(state(cx, &rig)["language_name"], "Spanish");
}

#[gpui_kit::test]
fn the_picker_lists_auto_and_the_whisper_languages_and_saves_the_choice(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let codes = rig.dictation.read_with(cx, |me, _| me.picker_codes());
    assert_eq!(codes[0], "auto");
    assert!(codes.len() >= 91);
    for wanted in ["en", "es", "de", "ja"] {
        assert!(codes.contains(&wanted), "{wanted}");
    }

    act(cx, &rig, |me, cx| me.set_language("es", cx)).unwrap();
    assert_eq!(
        rig.storage.settings.get("dictation.language"),
        Some(json!("es"))
    );
    assert_eq!(
        rig.storage.settings.get("dictation.recentLanguages"),
        Some(json!(["es"]))
    );
    let codes = rig.dictation.read_with(cx, |me, _| me.picker_codes());
    assert_eq!(&codes[..2], ["auto", "es"]);
    assert!(act(cx, &rig, |me, cx| me.set_language("xx-invalid", cx)).is_err());
}

#[gpui_kit::test]
fn an_english_only_model_turns_the_picker_off_with_a_reason(cx: &mut TestAppContext) {
    let rig = rig(cx, &["tiny.en"], "tiny.en");
    let state = state(cx, &rig);
    assert_eq!(state["picker"]["enabled"], false);
    assert!(
        state["picker"]["reason"]
            .as_str()
            .unwrap()
            .contains("English only")
    );
    assert!(act(cx, &rig, |me, cx| me.toggle_picker(cx)).is_err());
    assert!(act(cx, &rig, |me, cx| me.set_language("es", cx)).is_err());
    assert_eq!(
        rig.storage.settings.get("dictation.language"),
        Some(json!("auto"))
    );
}

#[gpui_kit::test]
fn with_no_model_home_cannot_start_and_points_to_models(cx: &mut TestAppContext) {
    let rig = rig(cx, &[], "");
    let state = state(cx, &rig);
    assert_eq!(state["can_record"], false);
    assert_eq!(state["blocker"]["code"], "ENGINE_NO_MODEL");
    assert_eq!(state["blocker"]["models_link"], true);

    assert!(act(cx, &rig, |me, cx| me.toggle(cx)).is_err());
    assert_eq!(phase(cx, &rig), Phase::Idle);
    assert!(!rig.storage.data.sessions_dir().exists());
}

#[gpui_kit::test]
fn an_empty_model_id_uses_the_catalog_default_once_it_is_verified(cx: &mut TestAppContext) {
    let missing = rig(cx, &["tiny.en"], "");
    assert_eq!(state(cx, &missing)["model"], "base");
    assert_eq!(state(cx, &missing)["can_record"], false);

    let present = rig(cx, &["base"], "");
    assert_eq!(state(cx, &present)["can_record"], true);
    assert_eq!(
        present.models.read_with(cx, |models, _| models.effective()),
        "base"
    );
}

#[gpui_kit::test]
fn a_cpu_without_the_needed_instructions_blocks_recording_with_a_clear_message(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    *rig.problem.borrow_mut() = Some(LoadProblem {
        model: "base".into(),
        code: "ENGINE_LOAD_FAILED".into(),
        unsupported_cpu: true,
    });
    rig.controller
        .update(cx, |controller, cx| controller.poll_problem(cx));

    let state = state(cx, &rig);
    assert_eq!(state["can_record"], false);
    assert_eq!(state["blocker"]["code"], "ENGINE_LOAD_FAILED");
    assert!(
        state["blocker"]["message"]
            .as_str()
            .unwrap()
            .contains("processor is not supported")
    );
    assert!(act(cx, &rig, |me, cx| me.toggle(cx)).is_err());
}

#[gpui_kit::test]
fn a_microphone_that_cannot_start_fails_the_run_with_its_message(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    *rig.mic_backend.start_error.lock().unwrap() = Some(CaptureError::new(
        hushpen_core::error::MIC_UNAVAILABLE,
        "no input device",
    ));
    assert!(act(cx, &rig, |me, cx| me.toggle(cx)).is_err());
    assert_eq!(phase(cx, &rig), Phase::Failed);
    assert_eq!(state(cx, &rig)["notice"]["code"], "MIC_UNAVAILABLE");
}

#[gpui_kit::test]
fn a_result_for_an_older_session_is_dropped(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("stale", "en"));
    act(cx, &rig, |me, cx| me.toggle(cx)).unwrap();
    act(cx, &rig, |me, cx| me.toggle(cx)).unwrap();
    let stale = rig.work.borrow_mut().pop_front().unwrap();
    // The user cancels, so the answer that is still on its way belongs to a dead session.
    send(cx, &rig, AppEvent::Esc).unwrap();
    stale();
    cx.run_until_parked();
    assert_eq!(phase(cx, &rig), Phase::Idle);
    assert_eq!(state(cx, &rig)["transcript"], "");
}

#[gpui_kit::test]
fn the_keys_notice_shows_in_the_home_state_only_when_the_keys_are_off(cx: &mut TestAppContext) {
    use crate::controller::KeysStatus;
    use std::rc::Rc;
    let rig = rig(cx, &["base"], "base");
    assert!(cx.update(|cx| rig.dictation.read(cx).keys_notice(cx).is_none()));
    rig.controller.update(cx, |controller, _| {
        controller.attach_keys(
            KeysStatus::Unavailable(hushpen_platform::keys::Unavailable::wayland()),
            Rc::new(|_| {}),
        );
    });
    let notice = cx.update(|cx| rig.dictation.read(cx).keys_notice(cx).map(str::to_owned));
    assert!(notice.unwrap().contains("Not available on Wayland"));
}
