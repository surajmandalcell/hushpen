use super::*;

#[test]
fn the_five_steps_run_in_order_and_each_key_finds_its_step() {
    let keys: Vec<_> = Step::ALL.iter().map(|step| step.key()).collect();
    assert_eq!(keys, ["permissions", "mic", "model", "practice", "updates"]);
    for (index, step) in Step::ALL.into_iter().enumerate() {
        assert_eq!(step.index(), index);
        assert_eq!(Step::from_key(step.key()), Some(step));
        assert_eq!(step.next(), Step::ALL.get(index + 1).copied());
    }
    assert_eq!(Step::from_key(""), None);
    assert_eq!(Step::from_key("Model"), None);
}

#[test]
fn an_empty_or_unknown_stored_step_starts_at_permissions() {
    assert_eq!(resume(""), Step::Permissions);
    assert_eq!(resume("nowhere"), Step::Permissions);
    assert_eq!(resume("practice"), Step::Practice);
}

#[test]
fn an_empty_data_folder_starts_at_the_permissions_step() {
    assert_eq!(start(false, "", false), Start::Setup(Step::Permissions));
}

#[test]
fn an_unfinished_run_resumes_at_the_stored_step() {
    assert_eq!(
        start(false, "practice", false),
        Start::Setup(Step::Practice)
    );
    assert_eq!(start(false, "model", true), Start::Setup(Step::Model));
}

#[test]
fn a_finished_onboarding_never_opens_again_while_every_permission_is_there() {
    assert_eq!(start(true, "", false), Start::Main);
    assert_eq!(start(true, "practice", false), Start::Main);
}

#[test]
fn a_lost_permission_reopens_only_the_permissions_step() {
    assert_eq!(start(true, "", true), Start::Repair);
    assert_eq!(start(true, "updates", true), Start::Repair);
}

#[test]
fn the_keys_stay_closed_until_the_practice_step_passes() {
    let gate = |completed, step, passed| key_gate(completed, step, passed);
    for step in [Step::Permissions, Step::MicTest, Step::Model] {
        assert_eq!(gate(false, step, false), KeyGate::Closed, "{step:?}");
    }
    assert_eq!(gate(false, Step::Practice, false), KeyGate::Practice);
    assert_eq!(gate(false, Step::Practice, true), KeyGate::Open);
    assert_eq!(gate(false, Step::Updates, true), KeyGate::Open);
    assert_eq!(gate(true, Step::Permissions, false), KeyGate::Open);
}

#[test]
fn silence_fails_the_mic_test_even_when_a_model_is_there() {
    assert_eq!(
        mic_verdict(0.0, Heard::Text("hello".into())),
        MicVerdict::NoSound
    );
    assert_eq!(mic_verdict(0.04, Heard::NotTried), MicVerdict::NoSound);
}

#[test]
fn sound_without_a_model_passes_with_no_transcript() {
    assert_eq!(
        mic_verdict(0.3, Heard::NotTried),
        MicVerdict::Passed { transcript: None }
    );
}

#[test]
fn sound_with_words_passes_with_the_words() {
    assert_eq!(
        mic_verdict(0.3, Heard::Text("  The quick fox. ".into())),
        MicVerdict::Passed {
            transcript: Some("The quick fox.".into())
        }
    );
}

#[test]
fn sound_with_no_words_or_only_blank_markers_is_no_speech() {
    assert_eq!(
        mic_verdict(0.3, Heard::Text("".into())),
        MicVerdict::NoSpeech
    );
    assert_eq!(
        mic_verdict(0.3, Heard::Text("[BLANK_AUDIO]".into())),
        MicVerdict::NoSpeech
    );
}

#[test]
fn an_engine_failure_does_not_pass_the_mic_test() {
    assert_eq!(
        mic_verdict(0.3, Heard::Failed),
        MicVerdict::TranscriptFailed
    );
}
