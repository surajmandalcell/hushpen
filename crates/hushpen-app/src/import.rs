//! The Import view: audio files go into a queue, the speech engine reads them one at a time,
//! and each result becomes a History row of type Import.
//!
//! The engine child decodes the file, so a damaged file can only fail its own item. The
//! queue lives here: one job runs, the rest wait in the order they were added, and a cancel
//! never touches the items behind it.

pub mod panel;

use crate::controller::{CancelToken, Controller, segments_of};
use crate::engine_host::EngineHost;
use crate::hook;
use crate::models::{Models, Spawner};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{App, Context, Entity, FocusHandle};
use hushpen_core::dictionary;
use hushpen_core::error::{
    ENGINE_BAD_LANGUAGE, ENGINE_CRASHED, ENGINE_NO_MODEL, ENGINE_NO_SPEECH, ENGINE_UNAVAILABLE,
    HISTORY_WRITE_FAILED, IMPORT_DECODE_FAILED, IMPORT_UNSUPPORTED_CODEC,
};
use hushpen_core::language;
use hushpen_core::transcript::strip_blank_markers;
use hushpen_engine::{ImportSpec, JobOutcome, ProgressSink, Transcription};
use hushpen_store::dictionary as store_dictionary;
use hushpen_store::history::{self, Row, Segment};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

/// What the picker offers. The engine reads these; a file that is none of them still gets a
/// try, because the decoder looks at the bytes and not at the name.
pub const SUPPORTED: &str = "WAV, MP3, FLAC, Ogg Vorbis, M4A, and AIFF";

/// Runs one import and waits for it. Tests pass a fake.
pub type ImportRunner =
    Arc<dyn Fn(ImportSpec, ProgressSink, Arc<CancelToken>) -> JobOutcome + Send + Sync>;

/// The runner that talks to the engine child.
pub fn host_runner(host: &Rc<EngineHost>) -> ImportRunner {
    let client = host.client().clone();
    Arc::new(move |spec, progress, token| {
        let job = client.import(spec, Some(progress));
        let id = job.id();
        let cancel = client.clone();
        token.on_cancel(move || cancel.cancel(id));
        job.wait()
    })
}

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

pub struct Item {
    pub id: u64,
    pub path: PathBuf,
    pub name: String,
    pub state: State,
    pub progress: Progress,
    /// The code and the words of a failure.
    pub failure: Option<(String, String)>,
    /// The History row of a finished item.
    pub row_id: Option<String>,
    pub text: Option<String>,
    /// What the item was started with, kept for its History row.
    started: Option<Started>,
    /// Cancel or Remove, whichever the item shows.
    pub(crate) focus: FocusHandle,
}

#[derive(Clone)]
struct Started {
    model: String,
    language: Option<String>,
    prompt: Option<String>,
    token: Arc<CancelToken>,
}

enum ImportEvent {
    Progress { item: u64, fraction: f32 },
    Finished { item: u64, outcome: JobOutcome },
}

pub struct Parts {
    pub storage: Rc<Storage>,
    pub models: Entity<Models>,
    pub controller: Entity<Controller>,
    pub run: ImportRunner,
    pub spawn: Spawner,
}

pub struct Import {
    storage: Rc<Storage>,
    models: Entity<Models>,
    controller: Entity<Controller>,
    run: ImportRunner,
    spawn: Spawner,
    events: UnboundedSender<ImportEvent>,
    items: Vec<Item>,
    next_id: u64,
    /// Why the last add added nothing, or what it left out.
    notice: Option<String>,
    choosing: bool,
    pub(crate) choose_focus: FocusHandle,
    pub(crate) clear_focus: FocusHandle,
}

impl Import {
    pub fn new(parts: Parts, cx: &mut Context<Self>) -> Self {
        let (events, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some(event) = queue.next().await {
                if this
                    .update(cx, |import, cx| import.handle(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            storage: parts.storage,
            models: parts.models,
            controller: parts.controller,
            run: parts.run,
            spawn: parts.spawn,
            events,
            items: Vec::new(),
            next_id: 1,
            notice: None,
            choosing: false,
            choose_focus: cx.focus_handle().tab_stop(true),
            clear_focus: cx.focus_handle().tab_stop(true),
        }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn choosing(&self) -> bool {
        self.choosing
    }

    /// True from the add of a file until its result is saved. A data folder move would lose the
    /// result of a job that is still running.
    pub fn busy(&self) -> bool {
        self.items.iter().any(|item| {
            matches!(
                item.state,
                State::Running | State::Cancelling | State::Queued
            )
        })
    }

    pub fn has_finished(&self) -> bool {
        self.items.iter().any(|item| item.state.finished())
    }

    /// Why no file can be imported now, or `None` while one can.
    pub fn blocker(&self, cx: &App) -> Option<String> {
        let blocker = self.controller.read(cx).blocker(cx)?;
        Some(match blocker.code {
            ENGINE_NO_MODEL => {
                "A speech model is needed before you can import. Get one in Models.".to_owned()
            }
            _ => blocker.message,
        })
    }

    /// Adds files to the queue. A path that is not a regular file (a device, a pipe, a folder,
    /// a name that is gone) is refused here, before anything opens it, and the rest still go in.
    /// Returns how many were queued.
    pub fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) -> usize {
        if paths.is_empty() {
            return 0;
        }
        if let Some(why) = self.blocker(cx) {
            hook::record_event("import", "refused no-model");
            self.notice = Some(why);
            cx.notify();
            return 0;
        }
        let mut refused = Vec::new();
        let mut queued = 0;
        for path in paths {
            let name = file_name(&path);
            if !is_regular_file(&path) {
                hook::record_event("import", &format!("refused {name}"));
                refused.push(name);
                continue;
            }
            let id = self.next_id;
            self.next_id += 1;
            hook::record_event("import", &format!("queued {name}"));
            self.items.push(Item {
                id,
                path,
                name,
                state: State::Queued,
                progress: Progress::default(),
                failure: None,
                row_id: None,
                text: None,
                started: None,
                focus: cx.focus_handle().tab_stop(true),
            });
            queued += 1;
        }
        self.notice = match refused.as_slice() {
            [] => None,
            [only] => Some(format!(
                "\u{201c}{only}\u{201d} cannot be opened. Only audio files can be imported."
            )),
            [first, rest @ ..] => Some(format!(
                "\u{201c}{first}\u{201d} and {} more cannot be opened. Only audio files can be imported.",
                rest.len()
            )),
        };
        self.start_next(cx);
        cx.notify();
        queued
    }

    /// Opens the file picker, like the Choose files button.
    pub fn choose(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.choosing {
            return Err("The file picker is already open.".into());
        }
        self.choosing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let paths = pick_files(cx).await;
            let _ = this.update(cx, |import, cx| {
                import.choosing = false;
                if let Some(paths) = paths {
                    import.add_paths(paths, cx);
                }
                cx.notify();
            });
        })
        .detach();
        Ok(())
    }

    /// Stops a queued item at once, or tells the engine to stop the running one. A running item
    /// shows Cancelled when the engine answers, which takes under a second.
    pub fn cancel(&mut self, id: u64, cx: &mut Context<Self>) -> Result<(), String> {
        let item = self
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or("That file is not in the queue.")?;
        match item.state {
            State::Queued => {
                item.state = State::Cancelled;
                hook::record_event("import", &format!("cancelled {}", item.name));
            }
            State::Running => {
                item.state = State::Cancelling;
                hook::record_event("import", &format!("cancel {}", item.name));
                if let Some(started) = &item.started {
                    started.token.cancel();
                }
            }
            State::Cancelling => {}
            _ => return Err("That file is not waiting or running.".into()),
        }
        cx.notify();
        Ok(())
    }

    /// Takes one finished item off the list. Its History row stays.
    pub fn remove(&mut self, id: u64, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self
            .items
            .iter()
            .position(|item| item.id == id)
            .ok_or("That file is not in the queue.")?;
        if !self.items[index].state.finished() {
            return Err("Cancel the file first.".into());
        }
        self.items.remove(index);
        cx.notify();
        Ok(())
    }

    /// Takes every finished item off the list.
    pub fn clear_finished(&mut self, cx: &mut Context<Self>) {
        self.items.retain(|item| !item.state.finished());
        self.notice = None;
        cx.notify();
    }

    /// The pointer to the item that has its own handle for Cancel and Remove.
    pub fn id_at(&self, index: usize) -> Option<u64> {
        self.items.get(index).map(|item| item.id)
    }

    fn running(&self) -> bool {
        self.items
            .iter()
            .any(|item| matches!(item.state, State::Running | State::Cancelling))
    }

    /// Starts the oldest waiting item when nothing runs.
    fn start_next(&mut self, cx: &mut Context<Self>) {
        while !self.running() {
            let Some(index) = self
                .items
                .iter()
                .position(|item| item.state == State::Queued)
            else {
                return;
            };
            if let Some(why) = self.blocker(cx) {
                self.fail(index, ENGINE_NO_MODEL, why);
                continue;
            }
            let language = self.controller.read(cx).language();
            let language = (language != language::AUTO).then_some(language);
            let prompt =
                Some(dictionary::build_prompt(&self.dictionary())).filter(|p| !p.is_empty());
            let model = self.models.read(cx).effective();
            let token = Arc::new(CancelToken::default());
            let item = &mut self.items[index];
            item.state = State::Running;
            item.started = Some(Started {
                model,
                language: language.clone(),
                prompt: prompt.clone(),
                token: Arc::clone(&token),
            });
            hook::record_event("import", &format!("start {}", item.name));
            let (id, name) = (item.id, item.name.clone());
            let spec = ImportSpec {
                path: item.path.clone(),
                language,
                prompt,
            };
            let run = Arc::clone(&self.run);
            let events = self.events.clone();
            let sink: ProgressSink = {
                let events = events.clone();
                Arc::new(move |fraction| {
                    let _ = events.unbounded_send(ImportEvent::Progress { item: id, fraction });
                })
            };
            let spawned = (self.spawn)(Box::new(move || {
                let outcome = run(spec, sink, token);
                let _ = events.unbounded_send(ImportEvent::Finished { item: id, outcome });
            }));
            if let Err(error) = spawned {
                log::warn!("{ENGINE_CRASHED} the import of {name} could not start: {error}");
                self.fail(
                    index,
                    ENGINE_CRASHED,
                    "The import could not start. Try again.".to_owned(),
                );
            }
        }
    }

    fn dictionary(&self) -> Vec<dictionary::Entry> {
        store_dictionary::list(&self.storage.database).unwrap_or_else(|error| {
            log::warn!("the dictionary could not be read: {error}");
            Vec::new()
        })
    }

    fn fail(&mut self, index: usize, code: &str, message: String) {
        let item = &mut self.items[index];
        item.state = State::Failed;
        hook::record_event("import", &format!("failed {} {code}", item.name));
        item.failure = Some((code.to_owned(), message));
    }

    fn handle(&mut self, event: ImportEvent, cx: &mut Context<Self>) {
        match event {
            ImportEvent::Progress { item, fraction } => {
                let Some(item) = self.items.iter_mut().find(|found| found.id == item) else {
                    return;
                };
                if item.state == State::Running && item.progress.raise(fraction) {
                    hook::record_event(
                        "import",
                        &format!("progress {} {}", item.name, item.progress.percent()),
                    );
                    cx.notify();
                }
            }
            ImportEvent::Finished { item, outcome } => {
                if let Some(index) = self.items.iter().position(|found| found.id == item) {
                    self.finish(index, outcome, cx);
                }
                self.start_next(cx);
                cx.notify();
            }
        }
    }

    fn finish(&mut self, index: usize, outcome: JobOutcome, cx: &mut Context<Self>) {
        let name = self.items[index].name.clone();
        let cancelled = self.items[index].state == State::Cancelling;
        // The user asked to stop, so a result that was already on its way is not kept.
        if cancelled || matches!(outcome, JobOutcome::Cancelled) {
            self.items[index].state = State::Cancelled;
            hook::record_event("import", &format!("cancelled {name}"));
            return;
        }
        match outcome {
            JobOutcome::Done(done) => self.finish_done(index, done, cx),
            JobOutcome::Failed(failure) => {
                let message = failure_message(&failure.code, &failure.detail);
                log::warn!("{} the import of a file failed", failure.code);
                self.fail(index, &failure.code, message);
            }
            JobOutcome::Cancelled => {}
        }
    }

    fn finish_done(&mut self, index: usize, done: Transcription, cx: &mut Context<Self>) {
        let text = strip_blank_markers(&done.text).trim().to_owned();
        if text.is_empty() {
            self.fail(
                index,
                ENGINE_NO_SPEECH,
                failure_message(ENGINE_NO_SPEECH, ""),
            );
            return;
        }
        let item = &self.items[index];
        let Some(started) = item.started.clone() else {
            return;
        };
        let (row, segments) = import_row(&item.name, &started, &text, &done);
        if let Err(error) = history::save(&self.storage.database, &row, &segments) {
            log::error!(
                "{HISTORY_WRITE_FAILED} the history row of an import was not saved: {error}"
            );
            hook::record_event("history", &format!("write-failed {HISTORY_WRITE_FAILED}"));
            self.fail(
                index,
                HISTORY_WRITE_FAILED,
                failure_message(HISTORY_WRITE_FAILED, ""),
            );
            return;
        }
        let item = &mut self.items[index];
        item.state = State::Done;
        item.progress.raise(1.0);
        item.row_id = Some(row.id.clone());
        item.text = Some(text);
        hook::record_event("import", &format!("done {}", item.name));
        hook::record_event("history", "saved completed");
        self.controller
            .update(cx, |controller, cx| controller.history_changed(cx));
    }

    pub fn state_json(&self, cx: &App) -> Value {
        json!({
            "busy": self.busy(),
            "choosing": self.choosing,
            "notice": self.notice,
            "blocker": self.blocker(cx),
            "supported": SUPPORTED,
            "items": self.items.iter().map(|item| json!({
                "id": item.id,
                "name": item.name,
                "path": item.path.to_string_lossy(),
                "state": item.state.key(),
                "progress": item.progress.percent(),
                "code": item.failure.as_ref().map(|(code, _)| code),
                "message": item.failure.as_ref().map(|(_, message)| message),
                "row_id": item.row_id,
                "text": item.text,
            })).collect::<Vec<_>>(),
        })
    }
}

/// The History row of a finished import: type Import, the file name as its source, and no
/// audio, because the file stays where the user keeps it.
fn import_row(
    name: &str,
    started: &Started,
    text: &str,
    done: &Transcription,
) -> (Row, Vec<Segment>) {
    let created_at = history::now_ms();
    let row = Row {
        id: history::new_id(u64::try_from(created_at).unwrap_or(0)),
        created_at,
        kind: "import".to_owned(),
        status: "completed".to_owned(),
        duration_ms: i64::try_from(done.audio_ms).unwrap_or(0),
        model_id: Some(started.model.clone()),
        language_requested: started.language.clone(),
        language_detected: Some(done.language.clone()).filter(|code| code != "und"),
        prompt: started.prompt.clone(),
        raw_text: Some(text.to_owned()),
        final_text: Some(text.to_owned()),
        insert_outcome: Some("none".to_owned()),
        source_name: Some(name.to_owned()),
        ..Row::default()
    };
    (row, segments_of(done))
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// A `stat` and nothing else: a pipe is never opened, so nothing can block on it.
fn is_regular_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

/// What an item says when its job failed. The code stays on the item for the log and the hook.
pub fn failure_message(code: &str, detail: &str) -> String {
    match code {
        IMPORT_UNSUPPORTED_CODEC if !detail.is_empty() => detail.to_owned(),
        IMPORT_UNSUPPORTED_CODEC => "This kind of file is not supported yet.".to_owned(),
        IMPORT_DECODE_FAILED => {
            "This file could not be read as audio. It may be damaged or empty.".to_owned()
        }
        ENGINE_NO_SPEECH => "No speech was heard in this file.".to_owned(),
        ENGINE_NO_MODEL => "The speech model is not loaded yet. Try again in a moment.".to_owned(),
        ENGINE_UNAVAILABLE => {
            "The speech engine is not available. Try again in a moment.".to_owned()
        }
        ENGINE_BAD_LANGUAGE => {
            "That language is not supported. Choose another one in Home.".to_owned()
        }
        HISTORY_WRITE_FAILED => "The transcript could not be saved to the history.".to_owned(),
        _ => "The speech engine stopped while reading this file. Try again.".to_owned(),
    }
}

/// The files the user picks: the path the test hook queued, or the system's picker.
async fn pick_files(cx: &mut gpui_kit::AsyncApp) -> Option<Vec<PathBuf>> {
    #[cfg(feature = "test-automation")]
    if let Some(paths) = hushpen_testhook::dialogs::take_answer() {
        return Some(paths);
    }
    let asked = cx.update(|cx| {
        cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        })
    });
    match asked.await {
        Ok(Ok(Some(paths))) if !paths.is_empty() => Some(paths),
        Ok(Ok(_)) => None,
        Ok(Err(error)) => {
            log::warn!("the file picker of the system did not open: {error}");
            None
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests;
