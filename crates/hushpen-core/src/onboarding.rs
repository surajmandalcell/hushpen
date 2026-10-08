//! First-run onboarding: the five steps, where it opens, and what lets a step pass.
//!
//! Everything here is a pure decision. The app reads the system, calls these functions, and
//! draws the answer.

use crate::permission::{Permission, Permissions};
use crate::shortcut::Platform;
use crate::transcript::strip_blank_markers;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Permissions,
    MicTest,
    Model,
    Practice,
    Updates,
}

impl Step {
    pub const ALL: [Step; 5] = [
        Step::Permissions,
        Step::MicTest,
        Step::Model,
        Step::Practice,
        Step::Updates,
    ];

    /// The value of `onboarding.step` and the name in hook ids.
    pub fn key(self) -> &'static str {
        match self {
            Step::Permissions => "permissions",
            Step::MicTest => "mic",
            Step::Model => "model",
            Step::Practice => "practice",
            Step::Updates => "updates",
        }
    }

    pub fn from_key(key: &str) -> Option<Step> {
        Self::ALL.into_iter().find(|step| step.key() == key)
    }

    pub fn title(self) -> &'static str {
        match self {
            Step::Permissions => "Permissions",
            Step::MicTest => "Microphone test",
            Step::Model => "Speech model",
            Step::Practice => "Shortcut practice",
            Step::Updates => "Updates",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap_or(0)
    }

    pub fn next(self) -> Option<Step> {
        Self::ALL.get(self.index() + 1).copied()
    }
}

/// The step to open at, from the stored `onboarding.step`. A value that is empty or unknown
/// starts at the first step.
pub fn resume(stored: &str) -> Step {
    Step::from_key(stored).unwrap_or(Step::Permissions)
}

/// What the window shows at start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// The normal window.
    Main,
    /// First run, or a run that was left unfinished.
    Setup(Step),
    /// Onboarding was finished, but a permission is gone: only the permissions step opens.
    Repair,
}

pub fn start(completed: bool, stored_step: &str, permission_gone: bool) -> Start {
    match (completed, permission_gone) {
        (false, _) => Start::Setup(resume(stored_step)),
        (true, true) => Start::Repair,
        (true, false) => Start::Main,
    }
}

/// Whether the global shortcuts may start a dictation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyGate {
    /// No dictation starts from a key.
    Closed,
    /// Dictations start from keys, but the text stays in the practice field.
    Practice,
    Open,
}

pub fn key_gate(completed: bool, step: Step, practice_passed: bool) -> KeyGate {
    if completed || practice_passed || step > Step::Practice {
        KeyGate::Open
    } else if step == Step::Practice {
        KeyGate::Practice
    } else {
        KeyGate::Closed
    }
}

/// Which display session Hushpen runs in (Linux).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    X11,
    Wayland,
    NoDisplay,
}

/// What the app found out about one feature that needs the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Ready,
    /// Wayland does not let an app read global keys or press keys for another app.
    NotOnWayland,
    Off(String),
}

/// Everything the permissions page shows, read from the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub platform: Platform,
    pub session: Session,
    pub keys: Availability,
    pub paste: Availability,
    /// The microphone list has been read at least once.
    pub mic_listed: bool,
    pub mic_present: bool,
    pub mac: Permissions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKey {
    X11Session,
    GlobalKeys,
    Paste,
    Microphone,
    Accessibility,
    InputMonitoring,
}

impl RowKey {
    /// The name in hook ids and in the `onboarding` state section.
    pub fn key(self) -> &'static str {
        match self {
            RowKey::X11Session => "x11",
            RowKey::GlobalKeys => "keys",
            RowKey::Paste => "paste",
            RowKey::Microphone => "microphone",
            RowKey::Accessibility => "accessibility",
            RowKey::InputMonitoring => "inputMonitoring",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Ready,
    /// The user can fix it, and Continue waits for it.
    Missing,
    /// This system cannot do it. Continue does not wait.
    Unavailable,
}

impl RowState {
    pub fn key(self) -> &'static str {
        match self {
            RowState::Ready => "ready",
            RowState::Missing => "missing",
            RowState::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub key: RowKey,
    pub title: &'static str,
    pub state: RowState,
    pub detail: String,
    /// The macOS permission whose System Settings pane the row's button opens. The button stays
    /// after the grant, so the user can look at the pane again.
    pub opens: Option<Permission>,
}

pub fn permission_rows(facts: &Facts) -> Vec<Row> {
    match facts.platform {
        Platform::Linux => vec![
            x11_row(facts),
            availability_row(
                RowKey::GlobalKeys,
                "Global keys",
                &facts.keys,
                "The hold key can start a dictation from any app.",
            ),
            availability_row(
                RowKey::Paste,
                "Paste",
                &facts.paste,
                "Finished text can be pasted into the app you use.",
            ),
            microphone_row(facts, None),
        ],
        Platform::MacOs => vec![
            microphone_row(facts, Some(Permission::Microphone)),
            mac_row(
                RowKey::Accessibility,
                "Accessibility",
                Permission::Accessibility,
                facts,
                "Hushpen can press the paste key for you.",
                "Allow Accessibility so Hushpen can paste the text for you.",
            ),
            mac_row(
                RowKey::InputMonitoring,
                "Input monitoring",
                Permission::InputMonitoring,
                facts,
                "Hushpen can hear the hold key.",
                "Allow Input Monitoring so Hushpen can hear the hold key. If the keys stay off after you allow it, quit Hushpen and open it again.",
            ),
        ],
    }
}

fn row(key: RowKey, title: &'static str, state: RowState, detail: &str) -> Row {
    Row {
        key,
        title,
        state,
        detail: detail.to_owned(),
        opens: None,
    }
}

fn x11_row(facts: &Facts) -> Row {
    match facts.session {
        Session::X11 => row(
            RowKey::X11Session,
            "X11 session",
            RowState::Ready,
            "Hushpen found an X11 session. Global keys and paste need it.",
        ),
        Session::Wayland => row(
            RowKey::X11Session,
            "X11 session",
            RowState::Unavailable,
            "This is a Wayland session. Global keys and paste need X11.",
        ),
        Session::NoDisplay => row(
            RowKey::X11Session,
            "X11 session",
            RowState::Unavailable,
            "No display was found. Global keys and paste need an X11 session.",
        ),
    }
}

fn availability_row(
    key: RowKey,
    title: &'static str,
    availability: &Availability,
    ready: &str,
) -> Row {
    match availability {
        Availability::Ready => row(key, title, RowState::Ready, ready),
        Availability::NotOnWayland => row(
            key,
            title,
            RowState::Unavailable,
            "Not available on Wayland. Use the record button in Hushpen instead.",
        ),
        Availability::Off(reason) => row(key, title, RowState::Unavailable, reason),
    }
}

fn microphone_row(facts: &Facts, opens: Option<Permission>) -> Row {
    let access = facts.mac.microphone;
    let (state, detail) = if access.missing() {
        (RowState::Missing, "Allow Hushpen to use the microphone.")
    } else if !facts.mic_listed {
        (RowState::Missing, "Looking for a microphone.")
    } else if !facts.mic_present {
        (
            RowState::Missing,
            "No microphone was found. Connect one to continue.",
        )
    } else {
        (RowState::Ready, "A microphone is ready.")
    };
    Row {
        opens,
        ..row(RowKey::Microphone, "Microphone", state, detail)
    }
}

fn mac_row(
    key: RowKey,
    title: &'static str,
    permission: Permission,
    facts: &Facts,
    ready: &str,
    missing: &str,
) -> Row {
    let (state, detail) = if facts.mac.get(permission).missing() {
        (RowState::Missing, missing)
    } else {
        (RowState::Ready, ready)
    };
    Row {
        opens: Some(permission),
        ..row(key, title, state, detail)
    }
}

/// Continue on the permissions step waits for every row that can be fixed.
pub fn rows_ready(rows: &[Row]) -> bool {
    rows.iter().all(|row| row.state != RowState::Missing)
}

/// A level above this (0 to 1, from the level meter) counts as sound.
pub const HEARD_LEVEL: f32 = 0.05;

/// What the transcript of the mic test gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// No usable model yet, so there is no transcript to test.
    NotTried,
    Text(String),
    /// The engine failed.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicVerdict {
    NoSound,
    NoSpeech,
    TranscriptFailed,
    Passed { transcript: Option<String> },
}

pub fn mic_verdict(peak: f32, heard: Heard) -> MicVerdict {
    if peak < HEARD_LEVEL {
        return MicVerdict::NoSound;
    }
    match heard {
        Heard::NotTried => MicVerdict::Passed { transcript: None },
        Heard::Failed => MicVerdict::TranscriptFailed,
        Heard::Text(text) => {
            let text = strip_blank_markers(&text);
            let text = text.trim();
            if text.is_empty() {
                MicVerdict::NoSpeech
            } else {
                MicVerdict::Passed {
                    transcript: Some(text.to_owned()),
                }
            }
        }
    }
}

pub const HINT_NO_SOUND: &str =
    "No sound was heard. Check the microphone and speak a little louder, then try again.";
pub const HINT_NO_SPEECH: &str =
    "Sound was heard, but no words. Speak a little closer to the microphone and try again.";
pub const HINT_TRANSCRIPT_FAILED: &str =
    "The words could not be read. The microphone works. Try again in a moment.";
pub const NOTE_NO_MODEL: &str =
    "The microphone works. You will see the words after you download the speech model.";

#[cfg(test)]
mod tests;
