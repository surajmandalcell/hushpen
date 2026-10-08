//! The dictation pipeline as a pure reducer.
//!
//! Every surface (hotkey, Home, tray, flow bar) turns its input into an [`AppEvent`] and calls
//! [`DictationMachine::handle`]. The machine owns no clock, thread, or file: time comes in as
//! the `now` argument (milliseconds on any monotonic scale), and every action comes out as an
//! [`Effect`] for the app to run. Slow effects report back as events that carry the session id
//! of the run that asked for them, so a late answer from a cancelled run changes nothing.

mod machine;
#[cfg(test)]
mod tests;

pub use machine::DictationMachine;

/// Shortest hold that counts as a recording. A shorter press is a tap.
pub const MIN_HOLD_MS: u64 = 250;
/// A second press this soon after the first tap's press starts hands-free.
pub const DOUBLE_TAP_MS: u64 = 500;
/// How long the pipeline warns before the maximum duration.
pub const WARN_BEFORE_MAX_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Idle,
    Listening,
    Transcribing,
    Cleaning,
    Inserting,
    Done,
    Cancelled,
    Failed,
}

impl State {
    pub const ALL: [State; 8] = [
        State::Idle,
        State::Listening,
        State::Transcribing,
        State::Cleaning,
        State::Inserting,
        State::Done,
        State::Cancelled,
        State::Failed,
    ];

    pub fn key(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Listening => "listening",
            State::Transcribing => "transcribing",
            State::Cleaning => "cleaning",
            State::Inserting => "inserting",
            State::Done => "done",
            State::Cancelled => "cancelled",
            State::Failed => "failed",
        }
    }

    /// A session is running: Esc belongs to the pipeline and must not reach the focused app.
    pub fn takes_escape(self) -> bool {
        matches!(
            self,
            State::Listening | State::Transcribing | State::Cleaning | State::Inserting
        )
    }
}

/// How the running session was started, which decides how it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The hold key is down; releasing it stops.
    Hold,
    /// Started by a double tap, the hands-free toggle, or the flow bar; the next start event
    /// stops it.
    HandsFree,
    /// Started on Home: the text is copied and never pasted.
    Home,
}

impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Mode::Hold => "hold",
            Mode::HandsFree => "handsFree",
            Mode::Home => "home",
        }
    }
}

/// Everything a surface or a worker can tell the pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    HoldDown,
    HoldUp,
    /// The hands-free shortcut: `Hold+Space`.
    HandsFreeToggle,
    /// The record button on Home.
    HomeToggle,
    /// A click on the flow bar or the tray entry.
    FlowBarClick,
    Esc,
    /// Time passed. Sent at least every 250 ms while a session runs, and again at each deadline.
    Tick,
    CaptureError {
        code: &'static str,
    },
    Transcribed {
        session: u64,
        text: String,
    },
    TranscribeFailed {
        session: u64,
        code: &'static str,
    },
    /// The rule and dictionary text, or the AI text when the model answered in time.
    Cleaned {
        session: u64,
        text: String,
    },
    Inserted {
        session: u64,
    },
    InsertFailed {
        session: u64,
        code: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Start,
    Stop,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Paste into the focused app.
    Paste,
    /// Copy only; Home shows the text itself.
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    Done,
    /// A cancelled run keeps its audio, and its raw text once there is one.
    Cancelled,
    Failed,
}

/// What the app must do. The machine never does it itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    StartCapture {
        session: u64,
    },
    /// `keep: false` deletes the session audio and leaves no row.
    StopCapture {
        keep: bool,
    },
    Cue(Cue),
    /// Delete the finished session audio: the run had no words.
    DiscardAudio,
    Transcribe {
        session: u64,
    },
    CancelTranscribe,
    /// Rule and dictionary cleanup, then the AI step when it is on.
    Clean {
        session: u64,
        raw: String,
    },
    StopLlm,
    Insert {
        session: u64,
        text: String,
        delivery: Delivery,
    },
    SaveRow {
        status: RowStatus,
        text: Option<String>,
        code: Option<&'static str>,
    },
    UpdatePasteLast {
        text: String,
    },
    /// Show "1 minute left".
    MaxDurationWarning {
        seconds_left: u64,
    },
    /// Show the error for this code.
    Notify {
        code: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// `dictation.maxMinutes`.
    pub max_minutes: u64,
    pub done_ms: u64,
    pub cancelled_ms: u64,
    pub failed_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_minutes: 6,
            done_ms: 1_500,
            cancelled_ms: 500,
            failed_ms: 2_000,
        }
    }
}

impl Config {
    /// The defaults with the `dictation.maxMinutes` setting applied.
    pub fn with_max_minutes(max_minutes: u64) -> Self {
        Self {
            max_minutes,
            ..Self::default()
        }
    }

    pub fn max_ms(&self) -> u64 {
        self.max_minutes * 60_000
    }
}
