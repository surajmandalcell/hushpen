//! The import queue of the Files view: audio files go into a queue, the speech engine reads
//! them one at a time, and each result becomes a History row of type Import.
//!
//! The engine child decodes the file, so a damaged file can only fail its own item. The
//! queue lives here: one job runs, the rest wait in the order they were added, and a cancel
//! never touches the items behind it. A finished item can be copied, exported, opened in
//! History, or, when it failed, tried again.

mod item;
pub mod panel;

pub use item::{Item, Progress, State};

use item::Started;

use crate::controller::{CancelToken, Clock, Controller, segments_of};
use crate::engine_host::EngineHost;
use crate::history::export::{ExportFile, default_directory};
use crate::hook;
use crate::models::{Models, Spawner};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{App, ClipboardItem, Context, Entity, EventEmitter, FocusHandle};
use hushpen_core::catalog::Languages;
use hushpen_core::dictionary;
use hushpen_core::error::{
    ENGINE_BAD_LANGUAGE, ENGINE_CRASHED, ENGINE_NO_MODEL, ENGINE_NO_SPEECH,
    ENGINE_TRANSLATE_UNSUPPORTED, ENGINE_UNAVAILABLE, HISTORY_WRITE_FAILED, IMPORT_DECODE_FAILED,
    IMPORT_UNSUPPORTED_CODEC,
};
use hushpen_core::export::Format;
use hushpen_core::language;
use hushpen_core::transcript::strip_blank_markers;
use hushpen_engine::{ImportProgress, ImportSpec, JobOutcome, ProgressSink, Transcription};
use hushpen_store::dictionary as store_dictionary;
use hushpen_store::history::{self, Row, Segment};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

/// What the picker offers. The engine reads these; a file that is none of them still gets a
/// try, because the decoder looks at the bytes and not at the name.
pub const SUPPORTED: &str = "WAV, MP3, FLAC, Ogg Vorbis, M4A, and AIFF";

/// The settings the transcribe options keep.
pub const LANGUAGE_SETTING: &str = "files.language";
pub const TRANSLATE_SETTING: &str = "files.translate";

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

/// What the queue asks of the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRequest {
    /// Open History on this row.
    ShowInHistory(String),
}

/// The pop-up list that is open under a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Language,
    /// The formats of the Export button of this item.
    Export(u64),
}

enum ImportEvent {
    Progress { item: u64, progress: ImportProgress },
    Finished { item: u64, outcome: JobOutcome },
}

pub struct Parts {
    pub storage: Rc<Storage>,
    pub models: Entity<Models>,
    pub controller: Entity<Controller>,
    pub run: ImportRunner,
    pub spawn: Spawner,
    pub clock: Clock,
}

pub struct Import {
    storage: Rc<Storage>,
    models: Entity<Models>,
    controller: Entity<Controller>,
    run: ImportRunner,
    spawn: Spawner,
    clock: Clock,
    events: UnboundedSender<ImportEvent>,
    items: Vec<Item>,
    next_id: u64,
    /// Why the last add added nothing, or what it left out.
    notice: Option<String>,
    /// What the last Copy or Export did.
    info: Option<String>,
    menu: Option<Menu>,
    choosing: bool,
    exporting: bool,
    pub(crate) choose_focus: FocusHandle,
    pub(crate) clear_focus: FocusHandle,
    pub(crate) language_focus: FocusHandle,
    pub(crate) translate_focus: FocusHandle,
    /// The rows of the open menu: every language, or the export formats.
    pub(crate) option_focus: Vec<FocusHandle>,
}

impl EventEmitter<ImportRequest> for Import {}

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
            clock: parts.clock,
            events,
            items: Vec::new(),
            next_id: 1,
            notice: None,
            info: None,
            menu: None,
            choosing: false,
            exporting: false,
            choose_focus: cx.focus_handle().tab_stop(true),
            clear_focus: cx.focus_handle().tab_stop(true),
            language_focus: cx.focus_handle().tab_stop(true),
            translate_focus: cx.focus_handle().tab_stop(true),
            option_focus: (0..=language::LANGUAGES.len())
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
        }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn info(&self) -> Option<&str> {
        self.info.as_deref()
    }

    pub fn menu(&self) -> Option<Menu> {
        self.menu
    }

    pub fn choosing(&self) -> bool {
        self.choosing
    }

    pub fn now_ms(&self) -> u64 {
        (self.clock)()
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
                "A speech model is needed before you can transcribe. Get one in Models.".to_owned()
            }
            _ => blocker.message,
        })
    }

    /// The name of the speech model the next file uses.
    pub fn model_name(&self, cx: &App) -> String {
        let models = self.models.read(cx);
        let id = models.effective();
        models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .map_or(id, |row| row.entry.name.clone())
    }

    /// `auto` or the code of the spoken language of the next file.
    pub fn language(&self) -> String {
        self.storage
            .settings
            .get(LANGUAGE_SETTING)
            .and_then(|value| value.as_str().map(str::to_owned))
            .filter(|code| language::is_setting_value(code))
            .unwrap_or_else(|| language::AUTO.to_owned())
    }

    /// Why the language cannot be chosen, or `None` while it can.
    pub fn language_off_reason(&self, cx: &App) -> Option<&'static str> {
        self.english_only(cx)
            .then_some("This model understands English only.")
    }

    /// Whether the next file is translated to English, as the user last chose.
    pub fn translate(&self) -> bool {
        self.storage
            .settings
            .get(TRANSLATE_SETTING)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    /// Why the model cannot translate, or `None` while it can. Whisper's English-only models
    /// have no translate task, and the turbo models were trained without it.
    pub fn translate_off_reason(&self, cx: &App) -> Option<&'static str> {
        if self.english_only(cx) {
            Some("English-only models cannot translate. Choose a multilingual model in Models.")
        } else if self.models.read(cx).effective().contains("turbo") {
            Some("Turbo models cannot translate. Choose another model in Models.")
        } else {
            None
        }
    }

    fn english_only(&self, cx: &App) -> bool {
        let models = self.models.read(cx);
        let id = models.effective();
        models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .is_some_and(|row| row.entry.languages == Languages::English)
    }

    pub fn set_language(&mut self, code: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if !language::is_setting_value(code) {
            return Err(format!("`{code}` is not a language Hushpen knows."));
        }
        self.storage
            .settings
            .set(LANGUAGE_SETTING, json!(code))
            .map_err(|error| error.to_string())?;
        self.menu = None;
        cx.notify();
        Ok(())
    }

    pub fn set_translate(&mut self, on: bool, cx: &mut Context<Self>) -> Result<(), String> {
        if on && let Some(why) = self.translate_off_reason(cx) {
            return Err(why.to_owned());
        }
        self.storage
            .settings
            .set(TRANSLATE_SETTING, json!(on))
            .map_err(|error| error.to_string())?;
        cx.notify();
        Ok(())
    }

    /// Opens `menu`, or closes it when it is the one open.
    pub fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = (self.menu != Some(menu)).then_some(menu);
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
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
            let Some(size_bytes) = regular_file_size(&path) else {
                hook::record_event("import", &format!("refused {name}"));
                refused.push(name);
                continue;
            };
            let id = self.next_id;
            self.next_id += 1;
            hook::record_event("import", &format!("queued {name}"));
            self.items.push(Item::new(id, path, name, size_bytes, cx));
            queued += 1;
        }
        self.notice = match refused.as_slice() {
            [] => None,
            [only] => Some(format!(
                "\u{201c}{only}\u{201d} cannot be opened. Only audio files can be transcribed."
            )),
            [first, rest @ ..] => Some(format!(
                "\u{201c}{first}\u{201d} and {} more cannot be opened. Only audio files can be transcribed.",
                rest.len()
            )),
        };
        self.info = None;
        self.start_next(cx);
        cx.notify();
        queued
    }

    /// Opens the file picker, like the Choose Files button.
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
        let item = self.item_mut(id)?;
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
        let index = self.index_of(id)?;
        if !self.items[index].state.finished() {
            return Err("Cancel the file first.".into());
        }
        self.items.remove(index);
        if self.menu == Some(Menu::Export(id)) {
            self.menu = None;
        }
        cx.notify();
        Ok(())
    }

    /// Takes every finished item off the list.
    pub fn clear_finished(&mut self, cx: &mut Context<Self>) {
        self.items.retain(|item| !item.state.finished());
        self.notice = None;
        self.info = None;
        self.menu = None;
        cx.notify();
    }

    /// Puts a failed item back in the queue, behind the files that wait.
    pub fn retry(&mut self, id: u64, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self.index_of(id)?;
        if self.items[index].state != State::Failed {
            return Err("Only a file that failed can be tried again.".into());
        }
        let mut item = self.items.remove(index);
        item.reset();
        hook::record_event("import", &format!("retry {}", item.name));
        self.items.push(item);
        self.start_next(cx);
        cx.notify();
        Ok(())
    }

    /// Copies the transcript of a finished item.
    pub fn copy(&mut self, id: u64, cx: &mut Context<Self>) -> Result<(), String> {
        let item = self.item(id)?;
        let text = item
            .text
            .clone()
            .ok_or("Only a finished file has a transcript to copy.")?;
        self.info = Some(format!(
            "Copied the transcript of \u{201c}{}\u{201d}.",
            item.name
        ));
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        hook::record_event("import", "copied");
        cx.notify();
        Ok(())
    }

    /// Writes the transcript of a finished item to a file, through the save dialog.
    pub fn export(
        &mut self,
        id: u64,
        format: Format,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.exporting {
            return Err("A save dialog is already open.".into());
        }
        let row = self.row_of(id)?;
        let file = ExportFile::of(&self.storage.database, format, &[row])?;
        self.exporting = true;
        self.menu = None;
        self.info = None;
        hook::record_event("import", &format!("export-dialog {}", format.extension()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let saved = file.save(cx, default_directory()).await;
            let _ = this.update(cx, |import, cx| {
                import.exporting = false;
                import.info = match saved {
                    Ok(Some(exported)) => {
                        hook::record_event(
                            "import",
                            &format!("export-written {}", exported.path.display()),
                        );
                        Some(exported.notice())
                    }
                    Ok(None) => None,
                    Err(message) => Some(message),
                };
                cx.notify();
            });
        })
        .detach();
        Ok(())
    }

    /// Asks the window to open History on the row of a finished item.
    pub fn show_in_history(&mut self, id: u64, cx: &mut Context<Self>) -> Result<(), String> {
        let row = self.row_of(id)?;
        cx.emit(ImportRequest::ShowInHistory(row));
        Ok(())
    }

    fn row_of(&self, id: u64) -> Result<String, String> {
        self.item(id)?
            .row_id
            .clone()
            .ok_or_else(|| "Only a finished file has a transcript in History.".to_owned())
    }

    fn index_of(&self, id: u64) -> Result<usize, String> {
        self.items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| "That file is not in the queue.".to_owned())
    }

    fn item(&self, id: u64) -> Result<&Item, String> {
        Ok(&self.items[self.index_of(id)?])
    }

    fn item_mut(&mut self, id: u64) -> Result<&mut Item, String> {
        let index = self.index_of(id)?;
        Ok(&mut self.items[index])
    }

    /// The pointer to the item that has its own handles for its buttons.
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
            let language = self.language();
            let language = (language != language::AUTO).then_some(language);
            let translate = self.translate() && self.translate_off_reason(cx).is_none();
            let prompt =
                Some(dictionary::build_prompt(&self.dictionary())).filter(|p| !p.is_empty());
            let model = self.models.read(cx).effective();
            let token = Arc::new(CancelToken::default());
            let now = self.now_ms();
            let item = &mut self.items[index];
            item.start(now);
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
                translate,
            };
            let run = Arc::clone(&self.run);
            let events = self.events.clone();
            let sink: ProgressSink = {
                let events = events.clone();
                Arc::new(move |progress| {
                    let _ = events.unbounded_send(ImportEvent::Progress { item: id, progress });
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
                    "The file could not start. Try again.".to_owned(),
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
            ImportEvent::Progress { item, progress } => {
                let Some(item) = self.items.iter_mut().find(|found| found.id == item) else {
                    return;
                };
                if let Some(ms) = progress.audio_ms {
                    item.duration_ms = Some(ms);
                    cx.notify();
                }
                if item.state == State::Running && item.progress.raise(progress.fraction) {
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
        item.duration_ms = Some(done.audio_ms);
        item.row_id = Some(row.id.clone());
        item.text = Some(text);
        hook::record_event("import", &format!("done {}", item.name));
        hook::record_event("history", "saved completed");
        self.controller
            .update(cx, |controller, cx| controller.history_changed(cx));
    }

    pub fn state_json(&self, cx: &App) -> Value {
        let now = self.now_ms();
        json!({
            "busy": self.busy(),
            "choosing": self.choosing,
            "exporting": self.exporting,
            "notice": self.notice,
            "info": self.info,
            "blocker": self.blocker(cx),
            "supported": SUPPORTED,
            "model": self.model_name(cx),
            "language": self.language(),
            "language_off": self.language_off_reason(cx),
            "translate": self.translate(),
            "translate_off": self.translate_off_reason(cx),
            "menu": match self.menu {
                Some(Menu::Language) => json!("language"),
                Some(Menu::Export(id)) => json!({"export": id}),
                None => Value::Null,
            },
            "items": self.items.iter().map(|item| json!({
                "id": item.id,
                "name": item.name,
                "path": item.path.to_string_lossy(),
                "state": item.state.key(),
                "progress": item.progress.percent(),
                "size_bytes": item.size_bytes,
                "duration_ms": item.duration_ms,
                "left_ms": item.time_left_ms(now),
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

/// The size of a regular file, from a `stat` and nothing else: a pipe is never opened, so
/// nothing can block on it. `None` for anything that is not a regular file.
fn regular_file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|meta| meta.len())
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
        ENGINE_BAD_LANGUAGE => "That language is not supported. Choose another one.".to_owned(),
        ENGINE_TRANSLATE_UNSUPPORTED => "This model cannot translate. Turn Translate off, \
             or choose a multilingual model in Models."
            .to_owned(),
        HISTORY_WRITE_FAILED => "The transcript could not be saved to the history.".to_owned(),
        _ => "The speech engine stopped while reading this file. Try again.".to_owned(),
    }
}

/// The files the user picks: the path the test hook queued, or the system's picker.
pub(crate) async fn pick_files(cx: &mut gpui_kit::AsyncApp) -> Option<Vec<PathBuf>> {
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
