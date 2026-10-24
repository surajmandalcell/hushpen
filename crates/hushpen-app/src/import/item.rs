//! One file in the import queue: its state, its progress, and the time left.

use crate::controller::CancelToken;
use gpui_kit::{Context, FocusHandle};
use std::path::PathBuf;
use std::sync::Arc;

/// Below this much progress the time left is a guess that jumps, so none shows.
const LEFT_FROM: f32 = 0.05;

/// The buttons a row can show at once: Copy, Export, Show in History, and Remove.
pub const BUTTONS: usize = 4;

/// How far a job is. It only goes up, and a report outside 0 to 1 is cut to it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Progress(f32);

impl Progress {
    /// Takes a report. Returns whether the percent that shows changed.
    pub fn raise(&mut self, report: f32) -> bool {
        if report.is_nan() {
            return false;
        }
        let before = self.percent();
        self.0 = self.0.max(report.clamp(0.0, 1.0));
        self.percent() != before
    }

    pub fn fraction(self) -> f32 {
        self.0
    }

    pub fn percent(self) -> u32 {
        (self.0 * 100.0).floor() as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Queued,
    Running,
    /// The cancel is sent; the engine has not answered yet.
    Cancelling,
    Done,
    Failed,
    Cancelled,
}

impl State {
    pub fn key(self) -> &'static str {
        match self {
            State::Queued => "queued",
            State::Running => "running",
            State::Cancelling => "cancelling",
            State::Done => "done",
            State::Failed => "failed",
            State::Cancelled => "cancelled",
        }
    }

    pub fn finished(self) -> bool {
        matches!(self, State::Done | State::Failed | State::Cancelled)
    }
}

/// What an item was started with, kept for its History row.
#[derive(Clone)]
pub(super) struct Started {
    pub model: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub token: Arc<CancelToken>,
}

pub struct Item {
    pub id: u64,
    pub path: PathBuf,
    pub name: String,
    pub state: State,
    pub progress: Progress,
    pub size_bytes: u64,
    /// The length of the audio: from the headers once the file starts, then what the engine
    /// read.
    pub duration_ms: Option<u64>,
    /// When the job started, on the queue's clock.
    started_ms: Option<u64>,
    /// The code and the words of a failure.
    pub failure: Option<(String, String)>,
    /// The History row of a finished item.
    pub row_id: Option<String>,
    pub text: Option<String>,
    pub(super) started: Option<Started>,
    /// The row's buttons, in the order they show.
    pub(crate) focus: [FocusHandle; BUTTONS],
}

impl Item {
    pub(super) fn new<T>(
        id: u64,
        path: PathBuf,
        name: String,
        size_bytes: u64,
        cx: &mut Context<T>,
    ) -> Self {
        Self {
            id,
            path,
            name,
            state: State::Queued,
            progress: Progress::default(),
            size_bytes,
            duration_ms: None,
            started_ms: None,
            failure: None,
            row_id: None,
            text: None,
            started: None,
            focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
        }
    }

    pub(super) fn start(&mut self, now_ms: u64) {
        self.state = State::Running;
        self.started_ms = Some(now_ms);
    }

    /// Back to waiting, as if just added.
    pub(super) fn reset(&mut self) {
        self.state = State::Queued;
        self.progress = Progress::default();
        self.started_ms = None;
        self.failure = None;
        self.started = None;
    }

    /// The time left of a running job.
    pub fn time_left_ms(&self, now_ms: u64) -> Option<u64> {
        if self.state != State::Running {
            return None;
        }
        time_left(
            self.progress.fraction(),
            now_ms.saturating_sub(self.started_ms?),
        )
    }
}

/// The time left from how fast the job went so far.
fn time_left(fraction: f32, spent_ms: u64) -> Option<u64> {
    (fraction >= LEFT_FROM)
        .then(|| (spent_ms as f64 * f64::from(1.0 - fraction) / f64::from(fraction)).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_left_follows_the_pace_so_far() {
        assert_eq!(time_left(0.25, 20_000), Some(60_000));
        assert_eq!(time_left(1.0, 20_000), Some(0));
        assert_eq!(time_left(0.01, 20_000), None, "too early to tell");
    }
}
