//! The model library: which speech models are on disk, which one is active, and the
//! downloads, deletes, and hash checks behind the Models view.
//!
//! A model counts as downloaded only when its file matches the pinned size and sha256. Slow
//! work (downloads, hashing) runs on worker threads. They send [`ModelEvent`]s into a channel
//! and one task on the main thread applies them, so the UI never waits on a download.

pub mod panel;

use crate::engine_host::EngineHost;
use crate::hook;
use crate::net::download::{Failure, Outcome, Progress, Spec};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{Context, FocusHandle};
use hushpen_core::catalog::{Catalog, Languages, ModelEntry};
use hushpen_core::error::{DOWNLOAD_FAILED, MODEL_HASH_MISMATCH, MODEL_HOST_BLOCKED, MODEL_IN_USE};
use hushpen_store::model_files::{self, Check, Verdict};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const SETTING: &str = "dictation.modelId";

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
    /// The file is being hashed: at start-up, or the last step of a download.
    Verifying,
    Ready,
    /// The file on disk is not the pinned file. It is never used.
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

pub struct Row {
    pub entry: ModelEntry,
    pub state: ModelState,
    /// Download or cancel, use, delete.
    pub(crate) focus: [FocusHandle; 3],
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
    transfer: Arc<Transfer>,
    spawn: Spawner,
    engine: Option<Rc<EngineHost>>,
    events: UnboundedSender<ModelEvent>,
    rows: Vec<Row>,
    notice: Option<Notice>,
    jobs: HashMap<String, Job>,
    /// Downloads the user asked for again while the cancelled worker was still winding down.
    restart: HashSet<String>,
    next_job: u64,
}

impl Models {
    pub fn new(
        storage: Rc<Storage>,
        catalog: &'static Catalog,
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
        let rows = catalog
            .whisper
            .iter()
            .map(|entry| Row {
                entry: entry.clone(),
                state: ModelState::NotDownloaded,
                focus: [
                    cx.focus_handle().tab_stop(true),
                    cx.focus_handle().tab_stop(true),
                    cx.focus_handle().tab_stop(true),
                ],
            })
            .collect();
        let mut models = Self {
            storage,
            transfer,
            spawn,
            engine,
            events,
            rows,
            notice: None,
            jobs: HashMap::new(),
            restart: HashSet::new(),
            next_job: 0,
        };
        models.scan();
        models.load_active();
        models
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
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

    fn model_path(&self, entry: &ModelEntry) -> PathBuf {
        self.storage.data.whisper_models_dir().join(&entry.file)
    }

    fn partial_path(&self, entry: &ModelEntry) -> PathBuf {
        self.storage
            .data
            .downloads_dir()
            .join(format!("{}.download", entry.file))
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.rows.iter().position(|row| row.entry.id == id)
    }

    /// Reads every file's state. A file with a stamp that still matches is ready at once; any
    /// other file is hashed on a worker thread and stays `Verifying` until the hash is known.
    fn scan(&mut self) {
        let mut to_hash = Vec::new();
        for row in &mut self.rows {
            if matches!(row.state, ModelState::Downloading { .. }) {
                continue;
            }
            let path = self.storage.data.whisper_models_dir().join(&row.entry.file);
            row.state = match model_files::check(&path, row.entry.bytes, &row.entry.sha256) {
                Check::Missing => ModelState::NotDownloaded,
                Check::Ready => ModelState::Ready,
                Check::WrongSize => ModelState::Failed,
                Check::NeedsHash => {
                    to_hash.push((row.entry.clone(), path));
                    ModelState::Verifying
                }
            };
        }
        if to_hash.is_empty() {
            return;
        }
        let events = self.events.clone();
        let spawned = (self.spawn)(Box::new(move || {
            for (entry, path) in to_hash {
                let ready = matches!(
                    model_files::verify(&path, entry.bytes, &entry.sha256, |_| true),
                    Ok(Verdict::Ready { .. })
                );
                if !ready {
                    log::warn!(
                        "{MODEL_HASH_MISMATCH} model={} failed the hash check",
                        entry.id
                    );
                }
                let _ = events.unbounded_send(ModelEvent::Verified {
                    id: entry.id,
                    ready,
                });
            }
        }));
        if let Err(error) = spawned {
            log::warn!("{DOWNLOAD_FAILED} could not start the hash check: {error}");
            for row in &mut self.rows {
                if row.state == ModelState::Verifying {
                    row.state = ModelState::Failed;
                }
            }
        }
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
        let name = self.rows[index].entry.name.clone();
        if was_cancelled {
            // The state already says "not downloaded". A late success is dropped, so the
            // user's cancel wins.
            if matches!(result, Ok(Outcome::Done)) {
                let _ = model_files::remove(&self.model_path(&self.rows[index].entry));
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
                    self.notice = Some(notice_for_failure(&failure, &name));
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
    fn load_active(&self) {
        let ready = self
            .index_of(&self.active())
            .is_some_and(|index| self.rows[index].state == ModelState::Ready);
        if let (true, Some(engine)) = (ready, &self.engine) {
            engine.apply_settings(&self.storage.settings.values());
        }
    }

    /// Starts a download, or resumes the partial file of an earlier try.
    pub fn download(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self
            .index_of(id)
            .ok_or_else(|| format!("no model with id '{id}'"))?;
        match self.rows[index].state {
            ModelState::NotDownloaded | ModelState::Failed => {}
            ModelState::Downloading { .. } | ModelState::Verifying => {
                return Err(format!("{id} is already being downloaded or checked"));
            }
            ModelState::Ready => return Err(format!("{id} is already downloaded")),
        }
        if self.jobs.get(id).is_some_and(|job| job.cancelled) {
            self.restart.insert(id.to_owned());
            self.rows[index].state = ModelState::Downloading {
                done: 0,
                total: self.rows[index].entry.bytes,
            };
            cx.notify();
            return Ok(());
        }
        let entry = self.rows[index].entry.clone();
        let spec = Spec {
            url: entry.url.clone(),
            bytes: entry.bytes,
            sha256: entry.sha256.clone(),
            partial: self.partial_path(&entry),
            dest: self.model_path(&entry),
        };
        self.next_job += 1;
        let number = self.next_job;
        let cancel = Arc::new(AtomicBool::new(false));
        let (events, transfer, flag) = (
            self.events.clone(),
            Arc::clone(&self.transfer),
            Arc::clone(&cancel),
        );
        let model = id.to_owned();
        (self.spawn)(Box::new(move || {
            let mut report = |step| {
                let _ = events.unbounded_send(ModelEvent::Progress {
                    id: model.clone(),
                    job: number,
                    step,
                });
            };
            let result = transfer(&spec, &flag, &mut report);
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
        self.rows[index].state = ModelState::Downloading {
            done: 0,
            total: entry.bytes,
        };
        self.notice = None;
        hook::record_event("models", &format!("download {id}"));
        cx.notify();
        Ok(())
    }

    /// Stops a download. The model shows as not downloaded at once; the worker removes the
    /// partial file as soon as it sees the flag.
    pub fn cancel(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self
            .index_of(id)
            .ok_or_else(|| format!("no model with id '{id}'"))?;
        self.restart.remove(id);
        let Some(job) = self.jobs.get_mut(id) else {
            return Err(format!("{id} is not being downloaded"));
        };
        job.cancel.store(true, Ordering::Relaxed);
        job.cancelled = true;
        self.rows[index].state = ModelState::NotDownloaded;
        self.notice = None;
        hook::record_event("models", &format!("cancel {id}"));
        cx.notify();
        Ok(())
    }

    /// Deletes the file and its stamp. The active model stays (`MODEL_IN_USE`).
    pub fn delete(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self
            .index_of(id)
            .ok_or_else(|| format!("no model with id '{id}'"))?;
        let name = self.rows[index].entry.name.clone();
        if self.active() == id {
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
        model_files::remove(&self.model_path(&self.rows[index].entry))
            .map_err(|error| format!("could not delete {id}: {error}"))?;
        self.rows[index].state = ModelState::NotDownloaded;
        self.notice = None;
        hook::record_event("models", &format!("delete {id}"));
        cx.notify();
        Ok(())
    }

    /// Makes `id` the model of the next dictation. Only a verified model can be chosen.
    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let index = self
            .index_of(id)
            .ok_or_else(|| format!("no model with id '{id}'"))?;
        let name = self.rows[index].entry.name.clone();
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
        let active = self.active();
        json!({
            "active": active,
            "notice": self.notice.as_ref().map(|notice| json!({
                "code": notice.code,
                "message": notice.message,
            })),
            "models": self.rows.iter().map(|row| json!({
                "id": row.entry.id,
                "name": row.entry.name,
                "bytes": row.entry.bytes,
                "languages": match row.entry.languages {
                    Languages::Multilingual => "multilingual",
                    Languages::English => "english",
                },
                "state": row.state.key(),
                "progress": row.state.percent(),
                "active": row.entry.id == active,
                "default": row.entry.default,
            })).collect::<Vec<_>>(),
        })
    }
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

#[cfg(test)]
mod tests;
