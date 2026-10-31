//! The model library: which speech, cleanup, and voice models are on disk, which speech model is
//! active, and the downloads, deletes, and hash checks behind the Models view.
//!
//! A model counts as downloaded only when every one of its files matches the pinned size and
//! sha256. Slow work (downloads, hashing) runs on worker threads. They send [`ModelEvent`]s into
//! a channel and one task on the main thread applies them, so the UI never waits on a download.

mod details;
pub(crate) mod files;
pub mod panel;
mod search;
pub mod sheet;

use crate::about::Opener;
use crate::engine_host::EngineHost;
use crate::hook;
use crate::net::download::{Failure, Outcome, Progress, Spec};
use crate::storage::Storage;
pub use details::SheetFocus;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{Context, FocusHandle};
use hushpen_core::catalog::{Catalog, Languages, ModelEntry, ModelKind};
use hushpen_core::error::{
    DOWNLOAD_FAILED, MODEL_HASH_MISMATCH, MODEL_HOST_BLOCKED, MODEL_IN_USE, MODEL_NO_SPACE,
};
use hushpen_store::model_files;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const SETTING: &str = "dictation.modelId";

/// How often a ready cleanup model's file is looked for again.
const PRESENCE_POLL: Duration = Duration::from_secs(1);

/// A partial download that nothing wrote to for this long is deleted at start-up.
const STALE_PARTIAL: Duration = Duration::from_secs(7 * 24 * 3600);

/// Runs slow work off the main thread. Tests queue it and run it on the test thread, because
/// the test scheduler refuses to be woken from another thread.
pub type Spawner = Rc<dyn Fn(Box<dyn FnOnce() + Send>) -> std::io::Result<()>>;

pub fn thread_spawner() -> Spawner {
    Rc::new(|work| {
        std::thread::Builder::new()
            .name("hushpen-models".into())
            .spawn(work)
            .map(drop)
    })
}

/// Runs one download. The real one talks to Hugging Face; tests pass a fake.
pub type Transfer =
    dyn Fn(&Spec, &AtomicBool, &mut dyn FnMut(Progress)) -> Result<Outcome, Failure> + Send + Sync;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelState {
    NotDownloaded,
    Downloading {
        done: u64,
        total: u64,
    },
    /// The files are being hashed: at start-up, or the last step of a download.
    Verifying,
    Ready,
    /// A file on disk is not the pinned file. The model is never used.
    Failed,
}

impl ModelState {
    pub fn key(self) -> &'static str {
        match self {
            ModelState::NotDownloaded => "not_downloaded",
            ModelState::Downloading { .. } => "downloading",
            ModelState::Verifying => "verifying",
            ModelState::Ready => "ready",
            ModelState::Failed => "failed",
        }
    }

    pub fn percent(self) -> Option<u64> {
        match self {
            ModelState::Downloading { done, total } if total > 0 => Some(done * 100 / total),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub code: &'static str,
    pub message: String,
}

/// What the library needs to know about the computer it runs on.
pub struct Machine {
    /// Opens a model's source page and its folder.
    pub opener: Rc<dyn Opener>,
}

/// The slots of [`Row::focus`].
pub(crate) const PRIMARY: usize = 0;
pub(crate) const USE: usize = 1;
pub(crate) const DELETE: usize = 2;
pub(crate) const INFO: usize = 3;

pub struct Row {
    pub entry: ModelEntry,
    pub state: ModelState,
    /// The Recommended badge: the default speech or voice model, and the cleanup model this
    /// computer is offered first.
    pub recommended: bool,
    /// The download in progress replaces an earlier version of the model's files, which the
    /// app started on its own. The row shows Updating, never a failure.
    pub updating: bool,
    /// Download or Cancel, Use, Delete, and the details button: see [`PRIMARY`] and the others.
    pub(crate) focus: [FocusHandle; 4],
}

pub enum ModelEvent {
    Progress {
        id: String,
        job: u64,
        step: Progress,
    },
    Finished {
        id: String,
        job: u64,
        result: Result<Outcome, Failure>,
    },
    Verified {
        id: String,
        ready: bool,
    },
}

struct Job {
    number: u64,
    cancel: Arc<AtomicBool>,
    /// The user cancelled. The worker may still be inside a network call.
    cancelled: bool,
}

pub struct Models {
    storage: Rc<Storage>,
    opener: Rc<dyn Opener>,
    transfer: Arc<Transfer>,
    spawn: Spawner,
    engine: Option<Rc<EngineHost>>,
    events: UnboundedSender<ModelEvent>,
    /// In [`ModelKind`] order: speech, then cleanup, then voice.
    rows: Vec<Row>,
    notice: Option<Notice>,
    jobs: HashMap<String, Job>,
    /// Downloads the user asked for again while the cancelled worker was still winding down.
    restart: HashSet<String>,
    next_job: u64,
    /// The row whose details sheet is open.
    details: Option<usize>,
    pub(crate) sheet_focus: SheetFocus,
    search: search::Search,
}

impl Models {
    pub fn new(
        storage: Rc<Storage>,
        catalog: &'static Catalog,
        machine: Machine,
        transfer: Arc<Transfer>,
        spawn: Spawner,
        engine: Option<Rc<EngineHost>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (events, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some(event) = queue.next().await {
                if this
                    .update(cx, |models, cx| models.handle(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PRESENCE_POLL).await;
                if this
                    .update(cx, |models, cx| models.refresh_llm_presence(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let swept = model_files::sweep_partials(&storage.data.downloads_dir(), STALE_PARTIAL);
        if swept > 0 {
            log::info!("MODELS_SWEPT removed {swept} partial downloads older than 7 days");
        }
        sweep_retired_llm(&storage.data.llm_models_dir());
        sweep_retired_speech(&storage, catalog);
        let offered = catalog.offered_llm();
        let suggested = offered.first().map(|entry| entry.id.clone());
        let rows = catalog
            .whisper
            .iter()
            .chain(offered)
            .chain(&catalog.voice)
            .map(|entry| Row {
                entry: entry.clone(),
                state: ModelState::NotDownloaded,
                recommended: match entry.kind {
                    ModelKind::Cleanup => suggested.as_ref() == Some(&entry.id),
                    ModelKind::Speech | ModelKind::Voice => entry.default,
                },
                updating: false,
                focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
            })
            .collect();
        let mut models = Self {
            storage,
            opener: machine.opener,
            transfer,
            spawn,
            engine,
            events,
            rows,
            notice: None,
            jobs: HashMap::new(),
            restart: HashSet::new(),
            next_job: 0,
            details: None,
            sheet_focus: SheetFocus::new(cx),
            search: search::Search::default(),
        };
        for id in models.scan() {
            models.update_files(&id, cx);
        }
        models.load_active();
        models
    }

    /// Downloads the changed files of a model whose files on disk are an earlier pin. Only the
    /// files that do not match the current pin are fetched; the others are kept.
    fn update_files(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        self.rows[index].updating = true;
        hook::record_event("models", &format!("update {id}"));
        if let Err(error) = self.download(id, cx) {
            self.rows[index].updating = false;
            log::warn!("MODELS_UPDATE could not start the update of {id}: {error}");
        }
    }

    /// Every model, in the order of the Models view.
    pub fn all_rows(&self) -> &[Row] {
        &self.rows
    }

    /// The models of one kind. Their place in [`Self::all_rows`] starts at the first index.
    pub fn of_kind(&self, kind: ModelKind) -> (usize, &[Row]) {
        let start = self.rows.partition_point(|row| row.entry.kind < kind);
        let end = self.rows.partition_point(|row| row.entry.kind <= kind);
        (start, &self.rows[start..end])
    }

    /// The speech models.
    pub fn rows(&self) -> &[Row] {
        self.of_kind(ModelKind::Speech).1
    }

    /// The cleanup models this computer is offered, which Settings > Cleanup also lists.
    pub fn llm_rows(&self) -> &[Row] {
        self.of_kind(ModelKind::Cleanup).1
    }

    /// A cleanup model file that someone deleted while the app runs shows as not downloaded,
    /// so Settings can offer the download again.
    fn refresh_llm_presence(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for row in &mut self.rows {
            if row.entry.kind == ModelKind::Cleanup
                && row.state == ModelState::Ready
                && !files::paths(&self.storage.data, &row.entry).all(|(_, path)| path.is_file())
            {
                row.state = ModelState::NotDownloaded;
                hook::record_event("models", &format!("missing {}", row.entry.id));
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    /// The state of a cleanup model.
    pub fn llm_state(&self, id: &str) -> Option<ModelState> {
        self.llm_rows()
            .iter()
            .find(|row| row.entry.id == id)
            .map(|row| row.state)
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    /// The id the next dictation uses, or empty when none is chosen.
    pub fn active(&self) -> String {
        self.storage
            .settings
            .get(SETTING)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    /// The id a dictation really uses: the chosen model, or the catalog default while none is
    /// chosen.
    pub fn effective(&self) -> String {
        let chosen = self.active();
        if !chosen.trim().is_empty() {
            return chosen;
        }
        self.rows()
            .iter()
            .find(|row| row.entry.default)
            .map(|row| row.entry.id.clone())
            .unwrap_or_default()
    }

    /// The effective model, once its file is verified. Home records only with this.
    pub fn usable(&self) -> Option<&Row> {
        let id = self.effective();
        self.rows()
            .iter()
            .find(|row| row.entry.id == id)
            .filter(|row| row.state == ModelState::Ready)
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|row| row.entry.id == id)
    }

    fn index_or_err(&self, id: &str) -> Result<usize, String> {
        self.index_of(id)
            .ok_or_else(|| format!("no model with id '{id}'"))
    }

    pub fn handle(&mut self, event: ModelEvent, cx: &mut Context<Self>) {
        match event {
            ModelEvent::Progress { id, job, step } => {
                if self
                    .jobs
                    .get(&id)
                    .is_none_or(|current| current.number != job || current.cancelled)
                {
                    return;
                }
                if let Some(index) = self.index_of(&id) {
                    self.rows[index].state = match step {
                        Progress::Bytes { done, total } => ModelState::Downloading { done, total },
                        Progress::Verifying => ModelState::Verifying,
                    };
                    cx.notify();
                }
            }
            ModelEvent::Finished { id, job, result } => self.finished(&id, job, result, cx),
            ModelEvent::Verified { id, ready } => {
                if let Some(index) = self.index_of(&id)
                    && self.rows[index].state == ModelState::Verifying
                    && !self.jobs.contains_key(&id)
                {
                    self.rows[index].state = if ready {
                        ModelState::Ready
                    } else {
                        let message = failure_text(&self.rows[index].entry);
                        self.set_notice(MODEL_HASH_MISMATCH, message);
                        ModelState::Failed
                    };
                    hook::record_event("models", &format!("verified {id} ready={ready}"));
                    self.load_active();
                    cx.notify();
                }
            }
        }
    }

    fn finished(
        &mut self,
        id: &str,
        job: u64,
        result: Result<Outcome, Failure>,
        cx: &mut Context<Self>,
    ) {
        if self
            .jobs
            .get(id)
            .is_none_or(|current| current.number != job)
        {
            return;
        }
        let was_cancelled = self.jobs.remove(id).is_some_and(|job| job.cancelled);
        let Some(index) = self.index_of(id) else {
            return;
        };
        let was_update = std::mem::take(&mut self.rows[index].updating);
        let name = self.rows[index].entry.name.clone();
        if was_cancelled {
            // The state already says "not downloaded". A late success is dropped, so the
            // user's cancel wins.
            if matches!(result, Ok(Outcome::Done)) {
                let _ = files::remove(&self.storage.data, &self.rows[index].entry);
            }
        } else {
            match result {
                Ok(Outcome::Done) => {
                    self.rows[index].state = ModelState::Ready;
                    self.notice = None;
                    hook::record_event("models", &format!("downloaded {id}"));
                    self.load_active();
                }
                Ok(Outcome::Cancelled) => self.rows[index].state = ModelState::NotDownloaded,
                Err(failure) => {
                    self.rows[index].state = ModelState::NotDownloaded;
                    // An update nobody asked for does not nag when the network is away; the
                    // next start tries it again, and Download tries it now.
                    let stopped = !matches!(failure.code, MODEL_HASH_MISMATCH | MODEL_HOST_BLOCKED);
                    if !(was_update && stopped) {
                        self.notice = Some(notice_for_failure(&failure, &name));
                    }
                    hook::record_event("models", &format!("failed {id} {}", failure.code));
                }
            }
        }
        if self.restart.remove(id) {
            self.rows[index].state = ModelState::NotDownloaded;
            let _ = self.download(id, cx);
        }
        cx.notify();
    }

    fn set_notice(&mut self, code: &'static str, message: String) {
        self.notice = Some(Notice { code, message });
    }

    /// Loads the active model into the engine, but only once its file is verified. The engine
    /// starts with no model so that a changed file is never loaded.
    pub(crate) fn load_active(&self) {
        if let (true, Some(engine)) = (self.usable().is_some(), &self.engine) {
            engine.apply_settings(&self.storage.settings.values());
        }
    }

    /// Starts a download, or resumes the partial files of an earlier try. Refused when the disk
    /// has less room than the files still to fetch, plus 10%.
    pub fn download(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self.index_or_err(id)?;
        match self.rows[index].state {
            ModelState::NotDownloaded | ModelState::Failed => {}
            ModelState::Downloading { .. } | ModelState::Verifying => {
                return Err(format!("{id} is already being downloaded or checked"));
            }
            ModelState::Ready => return Err(format!("{id} is already downloaded")),
        }
        let entry = self.rows[index].entry.clone();
        if self.jobs.get(id).is_some_and(|job| job.cancelled) {
            self.restart.insert(id.to_owned());
            self.rows[index].state = ModelState::Downloading {
                done: 0,
                total: entry.bytes(),
            };
            cx.notify();
            return Ok(());
        }
        let specs = files::missing_specs(&self.storage.data, &entry);
        let needed = files::bytes_to_fetch(&specs);
        if let Ok(free) = model_files::free_bytes(self.storage.data.root())
            && !model_files::has_room(free, needed)
        {
            self.set_notice(MODEL_NO_SPACE, no_space_text(&entry.name, needed, free));
            hook::record_event("models", &format!("no space {id}"));
            cx.notify();
            return Err(format!("{MODEL_NO_SPACE}: {id} needs {needed} bytes"));
        }
        self.next_job += 1;
        let number = self.next_job;
        let cancel = Arc::new(AtomicBool::new(false));
        let (events, transfer, flag) = (
            self.events.clone(),
            Arc::clone(&self.transfer),
            Arc::clone(&cancel),
        );
        let model = id.to_owned();
        let total = specs.iter().map(|spec| spec.bytes).sum();
        (self.spawn)(Box::new(move || {
            let mut report = |step| {
                let _ = events.unbounded_send(ModelEvent::Progress {
                    id: model.clone(),
                    job: number,
                    step,
                });
            };
            let result = files::run_download(&specs, &*transfer, &flag, &mut report);
            let _ = events.unbounded_send(ModelEvent::Finished {
                id: model,
                job: number,
                result,
            });
        }))
        .map_err(|error| format!("could not start the download: {error}"))?;
        self.jobs.insert(
            id.to_owned(),
            Job {
                number,
                cancel,
                cancelled: false,
            },
        );
        self.rows[index].state = ModelState::Downloading { done: 0, total };
        self.notice = None;
        hook::record_event("models", &format!("download {id}"));
        cx.notify();
        Ok(())
    }

    /// Stops a download. The model shows as not downloaded at once; the worker removes the
    /// partial file as soon as it sees the flag.
    pub fn cancel(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self.index_or_err(id)?;
        self.restart.remove(id);
        let Some(job) = self.jobs.get_mut(id) else {
            return Err(format!("{id} is not being downloaded"));
        };
        job.cancel.store(true, Ordering::Relaxed);
        job.cancelled = true;
        self.rows[index].updating = false;
        self.rows[index].state = ModelState::NotDownloaded;
        self.notice = None;
        hook::record_event("models", &format!("cancel {id}"));
        cx.notify();
        Ok(())
    }

    /// Deletes the files and their stamps. The active speech model stays (`MODEL_IN_USE`).
    pub fn delete(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self.index_or_err(id)?;
        let name = self.rows[index].entry.name.clone();
        if self.rows[index].entry.kind == ModelKind::Speech && self.effective() == id {
            self.set_notice(
                MODEL_IN_USE,
                format!("{name} is in use. Choose another model first."),
            );
            cx.notify();
            return Err(format!("{MODEL_IN_USE}: {id} is the active model"));
        }
        if matches!(
            self.rows[index].state,
            ModelState::Downloading { .. } | ModelState::Verifying
        ) {
            return Err(format!("{id} is busy; cancel the download first"));
        }
        files::remove(&self.storage.data, &self.rows[index].entry)
            .map_err(|error| format!("could not delete {id}: {error}"))?;
        self.rows[index].state = ModelState::NotDownloaded;
        self.notice = None;
        hook::record_event("models", &format!("delete {id}"));
        cx.notify();
        Ok(())
    }

    /// Makes `id` the model of the next dictation. Only a verified speech model can be chosen.
    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self.index_or_err(id)?;
        let name = self.rows[index].entry.name.clone();
        if self.rows[index].entry.kind != ModelKind::Speech {
            return Err(format!("{id} is not a speech model"));
        }
        match self.rows[index].state {
            ModelState::Ready => {}
            ModelState::Failed => {
                self.set_notice(
                    MODEL_HASH_MISMATCH,
                    format!("{name} failed verification and cannot be used. Download it again."),
                );
                cx.notify();
                return Err(format!("{MODEL_HASH_MISMATCH}: {id} failed verification"));
            }
            _ => return Err(format!("{id} is not downloaded yet")),
        }
        self.storage
            .settings
            .set(SETTING, json!(id))
            .map_err(|error| error.to_string())?;
        self.load_active();
        self.notice = None;
        hook::record_event("models", &format!("select {id}"));
        cx.notify();
        Ok(())
    }

    pub fn state_json(&self) -> Value {
        let active = self.effective();
        let list = |kind: ModelKind| {
            self.of_kind(kind)
                .1
                .iter()
                .map(|row| {
                    json!({
                        "id": row.entry.id,
                        "name": row.entry.name,
                        "bytes": row.entry.bytes(),
                        "languages": match row.entry.languages {
                            Languages::Multilingual => "multilingual",
                            Languages::English => "english",
                            Languages::European => "european",
                        },
                        "state": row.state.key(),
                        "updating": row.updating,
                        "progress": row.state.percent(),
                        "active": kind == ModelKind::Speech && row.entry.id == active,
                        "default": row.entry.default,
                        "recommended": row.recommended,
                        "files": row.entry.files.iter().map(|f| &f.file).collect::<Vec<_>>(),
                    })
                })
                .collect::<Vec<_>>()
        };
        json!({
            "active": active,
            "notice": self.notice.as_ref().map(|notice| json!({
                "code": notice.code,
                "message": notice.message,
            })),
            "details": self.details().map(|row| row.entry.id.clone()),
            "query": self.search.query,
            "models": list(ModelKind::Speech),
            "llm_models": list(ModelKind::Cleanup),
            "voice_models": list(ModelKind::Voice),
        })
    }
}

/// Deletes the files of cleanup models an earlier version offered, such as Gemma 4 E2B (3.1 GB).
/// Only `<id>.gguf` and its stamp go; nothing else in the folder is touched.
fn sweep_retired_llm(dir: &std::path::Path) {
    for id in hushpen_core::catalog::RETIRED_LLM {
        let path = dir.join(format!("{id}.gguf"));
        if !path.exists() {
            continue;
        }
        match model_files::remove(&path) {
            Ok(()) => log::info!("MODELS_RETIRED removed {id}.gguf"),
            Err(error) => log::warn!("MODELS_RETIRED could not remove {id}.gguf: {error}"),
        }
    }
}

/// Moves a retired selection before the engine loads, then removes only its known model files.
fn sweep_retired_speech(storage: &Storage, catalog: &Catalog) {
    use hushpen_core::catalog::RETIRED_SPEECH;

    let selected = storage
        .settings
        .get(SETTING)
        .and_then(|v| v.as_str().map(str::to_owned));
    if selected
        .as_deref()
        .is_some_and(|id| RETIRED_SPEECH.contains(&id) && catalog.whisper(id).is_none())
        && let Some(default) = catalog.default_whisper()
        && let Err(error) = storage.settings.set(SETTING, json!(default.id))
    {
        log::warn!("MODELS_RETIRED could not replace speech selection: {error}");
        return;
    }
    for id in RETIRED_SPEECH {
        if catalog.whisper(id).is_some() {
            continue;
        }
        let path = storage
            .data
            .whisper_models_dir()
            .join(format!("ggml-{id}.bin"));
        if path.exists() || model_files::stamp_path(&path).exists() {
            match model_files::remove(&path) {
                Ok(()) => log::info!("MODELS_RETIRED removed ggml-{id}.bin"),
                Err(error) => log::warn!("MODELS_RETIRED could not remove ggml-{id}.bin: {error}"),
            }
        }
    }
}

/// What a failed row says. The view notice shows the full text.
pub fn failure_text(entry: &ModelEntry) -> String {
    format!(
        "{} failed verification: what is on disk is not the pinned {} download, so it is never used. Download it again to replace it.",
        entry.name,
        panel::format_size(entry.bytes())
    )
}

fn no_space_text(name: &str, needed: u64, free: u64) -> String {
    format!(
        "{name} needs {} of free disk space, plus 10% to spare. Only {} is free.",
        panel::format_size(needed),
        panel::format_size(free)
    )
}

fn notice_for_failure(failure: &Failure, name: &str) -> Notice {
    let message = match failure.code {
        MODEL_HASH_MISMATCH => {
            format!("The download of {name} did not match its checksum. It was removed.")
        }
        MODEL_HOST_BLOCKED => {
            "The download was refused because it did not come from an approved host.".to_owned()
        }
        _ => format!("The download of {name} stopped. Try again to continue it."),
    };
    Notice {
        code: match failure.code {
            MODEL_HASH_MISMATCH => MODEL_HASH_MISMATCH,
            MODEL_HOST_BLOCKED => MODEL_HOST_BLOCKED,
            _ => DOWNLOAD_FAILED,
        },
        message,
    }
}

/// The transfer the app uses: Hugging Face only, over the real HTTP client.
pub fn production_transfer() -> Arc<Transfer> {
    use crate::net::download::Downloader;
    use crate::net::fetch::UreqFetcher;
    use crate::net::policy::Policy;
    let policy = Policy::model_download();
    let downloader = Downloader::new(policy, Box::new(UreqFetcher::new(&policy)));
    Arc::new(move |spec, cancel, progress| downloader.run(spec, cancel, progress))
}

/// What a test's library asked to open.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Opened(pub std::cell::RefCell<Vec<String>>);

#[cfg(test)]
impl Opener for Opened {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.0.borrow_mut().push(url.to_owned());
        Ok(())
    }

    fn open_path(&self, path: &std::path::Path) -> Result<(), String> {
        self.0.borrow_mut().push(path.display().to_string());
        Ok(())
    }
}

#[cfg(test)]
impl Machine {
    /// A computer whose opener only records.
    pub(crate) fn test() -> Self {
        Self {
            opener: Rc::new(Opened::default()),
        }
    }
}

#[cfg(test)]
mod llm_tests;
#[cfg(test)]
mod sheet_tests;
#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod update_tests;
