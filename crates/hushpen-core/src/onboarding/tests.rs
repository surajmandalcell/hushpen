use super::*;
use crate::permission::Access;

fn mac(microphone: Access, accessibility: Access, input_monitoring: Access) -> Permissions {
    Permissions {
        microphone,
        accessibility,
        input_monitoring,
    }
}

fn linux_facts() -> Facts {
    Facts {
        platform: Platform::Linux,
        session: Session::X11,
        keys: Availability::Ready,
        paste: Availability::Ready,
        mic_listed: true,
        mic_present: true,
        mac: mac(
            Access::NotApplicable,
            Access::NotApplicable,
            Access::NotApplicable,
        ),
    }
}

fn mac_facts(permissions: Permissions) -> Facts {
    Facts {
        platform: Platform::MacOs,
        session: Session::X11,
        keys: Availability::Ready,
        paste: Availability::Ready,
        mic_listed: true,
        mic_present: true,
        mac: permissions,
    }
}

fn state_of(rows: &[Row], key: RowKey) -> RowState {
    rows.iter().find(|row| row.key == key).unwrap().state
}

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
fn linux_lists_the_session_the_keys_the_paste_and_the_microphone() {
    let rows = permission_rows(&linux_facts());
    let keys: Vec<_> = rows.iter().map(|row| row.key.key()).collect();
    assert_eq!(keys, ["x11", "keys", "paste", "microphone"]);
    assert!(rows.iter().all(|row| row.state == RowState::Ready));
    assert!(rows.iter().all(|row| row.opens.is_none()));
    assert!(rows_ready(&rows));
}

#[test]
fn a_missing_microphone_is_missing_and_holds_continue_back() {
    let mut facts = linux_facts();
    facts.mic_present = false;
    let rows = permission_rows(&facts);
    assert_eq!(state_of(&rows, RowKey::Microphone), RowState::Missing);
    assert!(!rows_ready(&rows));
    facts.mic_present = true;
    assert!(rows_ready(&permission_rows(&facts)));
}

#[test]
fn a_microphone_list_that_is_not_read_yet_does_not_count_as_ready() {
    let mut facts = linux_facts();
    facts.mic_listed = false;
    facts.mic_present = false;
    let rows = permission_rows(&facts);
    assert_eq!(state_of(&rows, RowKey::Microphone), RowState::Missing);
}

#[test]
fn wayland_says_keys_and_paste_are_not_available_and_does_not_hold_continue_back() {
    let mut facts = linux_facts();
    facts.session = Session::Wayland;
    facts.keys = Availability::NotOnWayland;
    facts.paste = Availability::NotOnWayland;
    let rows = permission_rows(&facts);
    assert_eq!(state_of(&rows, RowKey::X11Session), RowState::Unavailable);
    assert_eq!(state_of(&rows, RowKey::GlobalKeys), RowState::Unavailable);
    assert_eq!(state_of(&rows, RowKey::Paste), RowState::Unavailable);
    for key in [RowKey::GlobalKeys, RowKey::Paste] {
        let row = rows.iter().find(|row| row.key == key).unwrap();
        assert!(row.detail.contains("Wayland"), "{}", row.detail);
    }
    assert!(rows_ready(&rows));
}

#[test]
fn another_reason_the_keys_are_off_shows_its_own_words() {
    let mut facts = linux_facts();
    facts.keys = Availability::Off("The key listener could not start.".into());
    let rows = permission_rows(&facts);
    let row = rows
        .iter()
        .find(|row| row.key == RowKey::GlobalKeys)
        .unwrap();
    assert_eq!(row.state, RowState::Unavailable);
    assert_eq!(row.detail, "The key listener could not start.");
}

#[test]
fn macos_lists_the_three_permissions_each_with_its_own_pane() {
    let rows = permission_rows(&mac_facts(mac(
        Access::NotDetermined,
        Access::Denied,
        Access::Granted,
    )));
    let keys: Vec<_> = rows.iter().map(|row| row.key.key()).collect();
    assert_eq!(keys, ["microphone", "accessibility", "inputMonitoring"]);
    assert_eq!(state_of(&rows, RowKey::Microphone), RowState::Missing);
    assert_eq!(state_of(&rows, RowKey::Accessibility), RowState::Missing);
    assert_eq!(state_of(&rows, RowKey::InputMonitoring), RowState::Ready);
    let opens: Vec<_> = rows.iter().map(|row| row.opens).collect();
    assert_eq!(
        opens,
        [
            Some(Permission::Microphone),
            Some(Permission::Accessibility),
            Some(Permission::InputMonitoring)
        ]
    );
    assert!(!rows_ready(&rows));
}

#[test]
fn macos_is_ready_when_all_three_are_granted() {
    let rows = permission_rows(&mac_facts(mac(
        Access::Granted,
        Access::Granted,
        Access::Granted,
    )));
    assert!(rows.iter().all(|row| row.state == RowState::Ready));
    assert!(rows_ready(&rows));
}

#[test]
fn every_macos_permission_has_its_privacy_pane_url() {
    assert_eq!(
        Permission::Microphone.settings_url(),
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
    );
    assert_eq!(
        Permission::Accessibility.settings_url(),
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
    );
    assert_eq!(
        Permission::InputMonitoring.settings_url(),
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
    );
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
