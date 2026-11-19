//! Tracks only the text this dictation confirmed in its original field.

use super::{Controller, InsertSupport};
use hushpen_core::dictation::Mode;
use hushpen_core::insert::live::{StablePrefix, WordEdit, suffix_edit, untyped_suffix, word_edits};
use hushpen_core::onboarding::KeyGate;
use hushpen_engine::PartialSink;
use hushpen_platform::insert::{LiveError, LiveSpan, Target};
use std::sync::{Arc, Mutex, mpsc};

#[cfg(test)]
mod tests;

/// A single worker orders partials and watches the caret even during pauses in speech.
pub(super) struct Writer(Arc<Mutex<Option<Draft>>>);

impl Writer {
    pub(super) fn for_draft(
        draft: Option<Draft>,
        preview: PartialSink,
    ) -> (Option<Self>, PartialSink) {
        let Some(draft) = draft else {
            return (None, preview);
        };
        match Self::start(draft, preview.clone()) {
            Ok((writer, partial)) => (Some(writer), partial),
            Err(error) => {
                log::warn!("Could not start live insertion: {error}");
                (None, preview)
            }
        }
    }

    fn start(draft: Draft, preview: PartialSink) -> std::io::Result<(Self, PartialSink)> {
        let state = Arc::new(Mutex::new(Some(draft)));
        let weak = Arc::downgrade(&state);
        let (send, receive) = mpsc::channel::<String>();
        std::thread::Builder::new()
            .name("hushpen-live-insert".into())
            .spawn(move || {
                loop {
                    let update = receive.recv_timeout(super::TICK);
                    let Some(state) = weak.upgrade() else { break };
                    let mut slot = state.lock().unwrap_or_else(|p| p.into_inner());
                    let Some(draft) = slot.as_mut() else { break };
                    match update {
                        Ok(text) => preview(draft.partial(&text).to_owned()),
                        Err(mpsc::RecvTimeoutError::Timeout) => draft.watch_caret(),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok((
            Self(state),
            Arc::new(move |text| {
                let _ = send.send(text);
            }),
        ))
    }

    pub(super) fn stop(self) -> Option<Draft> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).take()
    }
}

pub(super) struct Draft {
    pub target: Target,
    pub span: Result<Box<dyn LiveSpan>, LiveError>,
    stable: StablePrefix,
    typed: String,
}

impl Controller {
    pub(super) fn begin_draft(&self) -> Option<Draft> {
        if self.gate != KeyGate::Open
            || !matches!(self.machine.mode(), Mode::Hold | Mode::HandsFree)
            || self.pasting_last.is_some()
            || self.machine.behind() > 0
        {
            return None;
        }
        let InsertSupport::Ready(inserter) = &self.insert else {
            return None;
        };
        let target = inserter.target();
        let span = inserter.begin_live(&target, &self.paste_overrides())?;
        Some(Draft {
            target,
            span: Ok(span),
            stable: StablePrefix::default(),
            typed: String::new(),
        })
    }
}

impl Draft {
    fn write(&mut self, keep: usize, suffix: &str) {
        if let Ok(span) = &mut self.span {
            match span.replace(keep, suffix) {
                Ok(()) => {
                    self.typed.truncate(keep);
                    self.typed.push_str(suffix);
                }
                Err(error) => self.span = Err(error),
            }
        }
    }

    /// Applies word fixes last to first. After one lands, a failure leaves a mixed draft, so
    /// History becomes the only safe copy.
    fn splice(&mut self, edits: &[WordEdit]) {
        let Ok(span) = &mut self.span else { return };
        for (done, edit) in edits.iter().rev().enumerate() {
            if let Err(error) = span.splice(edit.range.start, edit.range.end, edit.with) {
                self.span = Err(if done == 0 { error } else { LiveError::Uncertain });
                return;
            }
            self.typed.replace_range(edit.range.clone(), edit.with);
        }
    }

    fn check(&mut self) {
        if self.span.as_ref().is_ok_and(|span| !span.unchanged()) {
            self.span = Err(LiveError::Changed);
        }
    }

    fn watch_caret(&mut self) {
        if self.span.as_ref().is_ok_and(|span| !span.at_caret()) {
            self.span = Err(LiveError::Changed);
        }
    }

    pub(super) fn partial<'a>(&mut self, text: &'a str) -> &'a str {
        let (stable, tail) = self.stable.update(text);
        // A shorter prefix can mean a revision is still settling. Wait for the new word.
        if stable != self.typed && (!self.typed.starts_with(stable) || tail.is_empty()) {
            let (keep, add) = suffix_edit(&self.typed, stable);
            self.write(keep, add);
        } else {
            self.watch_caret();
        }
        if self.span.is_ok() {
            tail.trim_start()
        } else {
            text.strip_prefix(&self.typed).unwrap_or(text).trim_start()
        }
    }

    pub(super) fn finish_raw(&mut self, raw: &str) {
        if !self.typed.is_empty() {
            let (keep, add) = suffix_edit(&self.typed, raw);
            self.write(keep, add);
        }
    }

    /// When a write may have landed, History is the only safe copy of the full transcript.
    pub(super) fn finish<'a>(&mut self, raw: &'a str, cleaned: &'a str) -> Option<&'a str> {
        if self.typed == cleaned {
            self.check();
        } else {
            // Fix only the words that changed, so a late fix does not retype the whole draft.
            let (keep, add) = suffix_edit(&self.typed, cleaned);
            let tail = self.typed.len() - keep;
            match word_edits(&self.typed, cleaned) {
                Some(edits)
                    if edits.len() <= 64
                        && edits.iter().map(|edit| edit.range.len()).sum::<usize>() < tail =>
                {
                    self.splice(&edits);
                }
                _ => self.write(keep, add),
            }
        }
        match self.span {
            Ok(_) => Some(""),
            Err(LiveError::Uncertain) => None,
            Err(LiveError::Changed) if self.typed.is_empty() => Some(cleaned),
            Err(LiveError::Changed) if self.typed == raw => Some(""),
            Err(LiveError::Changed) => cleaned
                .strip_prefix(&self.typed)
                .or_else(|| untyped_suffix(&self.typed, raw)),
        }
    }

    pub(super) fn cancel(&mut self) {
        if !self.typed.is_empty() {
            self.write(0, "");
        }
    }
}
