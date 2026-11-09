//! First-run onboarding: the five steps, where it opens, and what lets a step pass.
//!
//! Everything here is a pure decision. The app reads the system, calls these functions, and
//! draws the answer.

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
    Native,
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
