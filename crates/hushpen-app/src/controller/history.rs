//! The controller's part of the history: one row for each dictation outcome, the audio that
//! goes with it, and the two actions that need the engine or the paste path (reprocess and
//! re-paste). The rows themselves are read and changed by the History view.

use super::{Controller, ENGINE_NO_SPEECH, InsertSupport, failure_for};
use crate::hook;
use futures::channel::oneshot;
use gpui_kit::Context;
use hushpen_core::dictation::RowStatus;
use hushpen_core::dictionary;
use hushpen_core::error::HISTORY_WRITE_FAILED;
use hushpen_core::transcript::strip_blank_markers;
use hushpen_engine::{JobOutcome, TranscribeSpec, Transcription};
use hushpen_store::history::{self, Reprocessed, Row, Segment};
use hushpen_store::retention::{self, Retention};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// How long a re-paste waits for the focus to leave Hushpen before it pastes anyway.
const FOCUS_WAIT: Duration = Duration::from_millis(500);
const FOCUS_STEP: Duration = Duration::from_millis(50);

/// What the steps before the save learned, kept until the row is written.
#[derive(Default)]
pub(super) struct Pending {
    pub raw: String,
    pub rule: String,
    pub duration_ms: i64,
    pub model_id: Option<String>,
    pub language_requested: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReprocessState {
    Running,
    /// The row stays as it was; the text says why.
    Failed(String),
}

impl Controller {
    /// The `audio/<id>.wav` path of a row, when the file is there.
    pub fn audio_file(&self, row: &Row) -> Option<PathBuf> {
        let path = self.storage.data.root().join(row.audio_path.as_deref()?);
        path.is_file().then_some(path)
    }

    pub fn history_revision(&self) -> u64 {
        self.history_revision
    }

    /// The audio retention setting, read each time so a change applies to the next run.
    pub fn retention(&self) -> Retention {
        self.storage
            .settings
            .get(retention::SETTING)
            .and_then(|value| value.as_str().map(Retention::from_setting))
            .unwrap_or(Retention::Days(retention::DEFAULT_DAYS))
    }

    pub fn reprocess_state(&self) -> Option<&(String, ReprocessState)> {
        self.reprocess.as_ref()
    }

    /// Saves the row of a finished, failed, or cancelled run and moves its audio to
    /// `audio/<id>.wav`. A write error is logged by its code; the text stays in memory for
    /// "Paste last transcript".
    pub(super) fn record_history(
        &mut self,
        status: RowStatus,
        text: &Option<String>,
        code: Option<&'static str>,
    ) {
        let session = self.machine.session();
        let wav = self.wav.clone().filter(|path| path.is_file());
        if wav.is_none() && text.is_none() {
            return;
        }
        let created_at = history::now_ms();
        let id = history::new_id(u64::try_from(created_at).unwrap_or(0));
        let (insert_outcome, target_app) = self.insert_summary(session, text.is_some());
        let detected = self
            .detected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .filter(|(owner, _)| *owner == session)
            .map(|(_, language)| language);
        let non_empty = |text: &str| Some(text.to_owned()).filter(|text| !text.is_empty());
        let status_key = match status {
            RowStatus::Done => "completed",
            RowStatus::Cancelled => "cancelled",
            RowStatus::Failed => "failed",
        };
        let keep_audio = wav.is_some() && self.retention().keeps(status_key);
        let dropped_audio = wav.is_some() && !keep_audio;
        let raw_text = match status {
            RowStatus::Cancelled => text.clone().or_else(|| non_empty(&self.pending.raw)),
            _ => non_empty(&self.pending.raw),
        };
        let row = Row {
            id: id.clone(),
            created_at,
            kind: "dictation".to_owned(),
            status: status_key.to_owned(),
            error_code: code.map(str::to_owned),
            duration_ms: self.pending.duration_ms,
            model_id: self.pending.model_id.clone(),
            language_requested: self.pending.language_requested.clone(),
            language_detected: detected,
            prompt: self.last_prompt.clone(),
            raw_text,
            rule_text: non_empty(&self.pending.rule),
            final_text: match status {
                RowStatus::Cancelled => None,
                _ => text.clone(),
            },
            insert_outcome: Some(insert_outcome),
            target_app,
            audio_path: keep_audio.then(|| format!("audio/{id}.wav")),
            audio_removed_at: dropped_audio.then_some(created_at),
            ..Row::default()
        };
        let segments = self
            .segments
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .filter(|(owner, _)| *owner == session)
            .map(|(_, segments)| segments)
            .unwrap_or_default();
        if let Err(error) = history::save(&self.storage.database, &row, &segments) {
            log::error!("{HISTORY_WRITE_FAILED} the history row was not saved: {error}");
            hook::record_event("history", &format!("write-failed {HISTORY_WRITE_FAILED}"));
            return;
        }
        if let Some(wav) = wav {
            if keep_audio {
                self.wav = Some(self.archive_audio(&wav, &id));
            } else {
                if let Err(error) = fs::remove_file(&wav) {
                    log::warn!("the audio of a finished run was not removed: {error}");
                }
                self.wav = None;
            }
        }
        self.history_revision += 1;
        hook::record_event("history", &format!("saved {}", row.status));
    }

    /// What the insertion of this session did: the outcome key and the app that got the text.
    /// A run with no text inserted nothing.
    fn insert_summary(&self, session: u64, has_text: bool) -> (String, Option<String>) {
        let slot = self.last_insert.lock().unwrap_or_else(|p| p.into_inner());
        match &*slot {
            Some((owner, _, report)) if *owner == session && has_text => (
                report.outcome.key().to_owned(),
                Some(report.target.clone()).filter(|target| !target.is_empty()),
            ),
            _ => ("none".to_owned(), None),
        }
    }

    /// Moves the session audio to the audio folder. When the move fails the file stays where
    /// it was and the path that comes back is that one.
    fn archive_audio(&self, wav: &Path, id: &str) -> PathBuf {
        let folder = self.storage.data.audio_dir();
        let target = folder.join(format!("{id}.wav"));
        let moved = fs::create_dir_all(&folder)
            .and_then(|()| fs::rename(wav, &target).or_else(|_| copy_then_remove(wav, &target)));
        match moved {
            Ok(()) => target,
            Err(error) => {
                log::warn!("the audio of a history row could not be moved: {error}");
                wav.to_path_buf()
            }
        }
    }

    /// Runs the engine again on the audio of a row with the current model, language, and
    /// dictionary, then replaces the row's text. Nothing is inserted and no row is added.
    pub fn reprocess(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if matches!(self.reprocess, Some((_, ReprocessState::Running))) {
            return Err("Another transcript is being reprocessed. Wait for it to finish.".into());
        }
        let row = history::get(&self.storage.database, id)
            .map_err(|error| error.to_string())?
            .ok_or("That transcript is no longer in the history.")?;
        let wav = self
            .audio_file(&row)
            .ok_or("The audio of this transcript is not available.")?;
        if let Some(blocker) = self.blocker(cx) {
            return Err(blocker.message);
        }
        let model = self.models.read(cx).effective();
        let language = self.language();
        let prompt = Some(dictionary::build_prompt(&self.dictionary())).filter(|p| !p.is_empty());
        let spec = TranscribeSpec {
            wav_path: wav,
            language: (language != hushpen_core::language::AUTO).then_some(language),
            prompt: prompt.clone(),
        };
        let token = Arc::new(super::CancelToken::default());
        let run = Arc::clone(&self.engine.run);
        let (reply, answer) = oneshot::channel();
        let spawned = (self.spawn)(Box::new(move || {
            let _ = reply.send(run(spec, token));
        }));
        if let Err(error) = spawned {
            log::warn!("could not start the reprocess: {error}");
            return Err("Reprocessing could not start. Try again.".into());
        }
        let id = id.to_owned();
        self.reprocess = Some((id.clone(), ReprocessState::Running));
        hook::record_event("history", &format!("reprocess started {id}"));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let outcome = answer.await.ok();
            let _ = this.update(cx, |me, cx| {
                me.reprocessed(&id, model, prompt, outcome, cx);
            });
        })
        .detach();
        Ok(())
    }

    fn reprocessed(
        &mut self,
        id: &str,
        model: String,
        prompt: Option<String>,
        outcome: Option<JobOutcome>,
        cx: &mut Context<Self>,
    ) {
        let failure = |message: &str| ReprocessState::Failed(message.to_owned());
        let state = match outcome {
            Some(JobOutcome::Done(done)) => {
                let raw = strip_blank_markers(&done.text);
                let rule = self.rule_cleanup(&raw);
                if rule.trim().is_empty() {
                    failure(failure_for(ENGINE_NO_SPEECH).1)
                } else {
                    let segments = segments_of(&done);
                    let language = Some(done.language).filter(|code| code != "und");
                    let result = Reprocessed {
                        model_id: Some(model),
                        language_detected: language,
                        prompt,
                        raw_text: raw,
                        rule_text: rule.trim().to_owned(),
                        final_text: rule.trim().to_owned(),
                        segments,
                    };
                    match history::reprocess(&self.storage.database, id, &result) {
                        Ok(()) => {
                            self.drop_finished_audio(id);
                            self.history_revision += 1;
                            hook::record_event("history", &format!("reprocessed {id}"));
                            self.reprocess = None;
                            cx.notify();
                            return;
                        }
                        Err(error) => {
                            log::error!(
                                "{HISTORY_WRITE_FAILED} the reprocessed row was not saved: {error}"
                            );
                            failure("The new text could not be saved.")
                        }
                    }
                }
            }
            Some(JobOutcome::Failed(failed)) => failure(failure_for(&failed.code).1),
            Some(JobOutcome::Cancelled) | None => failure("Reprocessing was cancelled."),
        };
        hook::record_event("history", &format!("reprocess failed {id}"));
        self.reprocess = Some((id.to_owned(), state));
        cx.notify();
    }

    /// With "never keep audio", a row that a reprocess has just finished no longer needs its
    /// audio.
    fn drop_finished_audio(&self, id: &str) {
        if self.retention() != Retention::Never {
            return;
        }
        let path = history::get(&self.storage.database, id)
            .ok()
            .flatten()
            .and_then(|row| row.audio_path);
        if let Some(path) = path
            && let Err(error) = retention::remove_audio(
                &self.storage.database,
                self.storage.data.root(),
                id,
                &path,
                history::now_ms(),
            )
        {
            log::warn!("the audio of a reprocessed row was not removed: {error}");
        }
    }

    /// True while the focused window is Hushpen's own, so a paste would only copy.
    pub fn target_is_own(&self) -> bool {
        match &self.insert {
            InsertSupport::Ready(inserter) => inserter.target().own,
            _ => false,
        }
    }

    /// Puts `text` into the app that has the focus once Hushpen no longer has it: waits up to
    /// 500 ms for the focus to leave, then pastes. It does not touch the pipeline or add a row.
    pub fn repaste(&mut self, text: String, cx: &mut Context<Self>) {
        self.notice = None;
        hook::record_event("insert", "repaste");
        cx.spawn(async move |this, cx| {
            let mut waited = Duration::ZERO;
            while waited < FOCUS_WAIT
                && this.update(cx, |me, _| me.target_is_own()).unwrap_or(false)
            {
                cx.background_executor().timer(FOCUS_STEP).await;
                waited += FOCUS_STEP;
            }
            let _ = this.update(cx, |me, cx| {
                me.paste_text(text, cx);
                cx.notify();
            });
        })
        .detach();
    }
}

/// The timed pieces of a transcription, in the shape of the `segment` table.
pub(super) fn segments_of(done: &Transcription) -> Vec<Segment> {
    done.segments
        .iter()
        .enumerate()
        .map(|(idx, segment)| Segment {
            idx: idx as i64,
            start_ms: i64::try_from(segment.start_ms).unwrap_or(0),
            end_ms: i64::try_from(segment.end_ms).unwrap_or(0),
            text: segment.text.clone(),
        })
        .collect()
}

fn copy_then_remove(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::copy(from, to)?;
    fs::remove_file(from)
}
