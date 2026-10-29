//! The read-aloud shortcut: the text selected in the focused app is read out loud with the
//! voice of Files > Read aloud, and the same shortcut stops a reading that plays. Like Command
//! Mode, the selection is read on a worker thread and is never logged.

use super::command::{SELECTION_WAIT, SelectionRead};
use super::{Controller, InsertSupport, failure_for};
use crate::hook;
use gpui_kit::{App, Context};
use hushpen_core::dictation::State;
use hushpen_core::error::{READ_NO_SELECTION, SPEECH_FAILED};
use std::rc::Rc;
use std::sync::Arc;

/// Plays this text, or gives the reason it cannot.
pub type Speak = Rc<dyn Fn(String, &mut App) -> Result<(), String>>;

/// The player that the shortcut drives.
#[derive(Clone)]
pub struct Speaker {
    pub playing: Rc<dyn Fn(&App) -> bool>,
    pub stop: Rc<dyn Fn(&mut App)>,
    pub speak: Speak,
}

impl Controller {
    pub fn attach_speaker(&mut self, speaker: Speaker) {
        self.speaker = Some(speaker);
    }

    /// Stops the reading that plays, or reads the selection and plays it.
    pub(super) fn read_aloud_pressed(&mut self, cx: &mut Context<Self>) {
        let Some(speaker) = self.speaker.clone() else {
            hook::record_event("read", "shortcut none");
            return;
        };
        if (speaker.playing)(cx) {
            self.speaking = None;
            (speaker.stop)(cx);
            hook::record_event("read", "shortcut stop");
            return;
        }
        // The selection read presses the copy keys, which must not land in a running session.
        let resting = matches!(
            self.machine.state(),
            State::Idle | State::Done | State::Cancelled | State::Failed
        );
        if !resting || self.selecting.is_some() || self.speaking.is_some() {
            hook::record_event("read", "shortcut busy");
            return;
        }
        let InsertSupport::Ready(inserter) = &self.insert else {
            self.no_selection();
            return;
        };
        let inserter = Arc::clone(inserter);
        let target = inserter.target();
        self.selection_seq += 1;
        let seq = self.selection_seq;
        let answers = self.spoken.clone();
        let spawned = (self.spawn)(Box::new(move || {
            let text = inserter.selection(&target);
            let _ = answers.unbounded_send((seq, text));
        }));
        if let Err(error) = spawned {
            log::warn!("{READ_NO_SELECTION} could not start the selection read: {error}");
            self.no_selection();
            return;
        }
        let deadline = self.spoken.clone();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(SELECTION_WAIT).await;
            let _ = deadline.unbounded_send((seq, None));
        })
        .detach();
        self.speaking = Some(seq);
    }

    /// The answer of the selection read, or `None` once [`SELECTION_WAIT`] passed.
    pub(super) fn spoken_selection(&mut self, (seq, text): SelectionRead, cx: &mut Context<Self>) {
        if self.speaking != Some(seq) {
            return;
        }
        self.speaking = None;
        let Some(speaker) = self.speaker.clone() else {
            return;
        };
        match text.filter(|text| !text.trim().is_empty()) {
            Some(text) => match (speaker.speak)(text, cx) {
                Ok(()) => hook::record_event("read", "shortcut play"),
                Err(message) => {
                    hook::record_event("read", &format!("shortcut failed {SPEECH_FAILED}"));
                    self.show_flash(SPEECH_FAILED, message);
                }
            },
            None => self.no_selection(),
        }
        cx.notify();
    }

    fn no_selection(&mut self) {
        hook::record_event("read", &format!("shortcut blocked {READ_NO_SELECTION}"));
        self.show_flash(
            READ_NO_SELECTION,
            failure_for(READ_NO_SELECTION).1.to_owned(),
        );
    }
}
