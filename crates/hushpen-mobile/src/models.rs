use crate::MobileError;
use hushpen_core::catalog::{self, ModelEntry};
use hushpen_store::model_files::{self, Check, Verdict};
use hushpen_store::net::{download, fetch::UreqFetcher, policy::Policy};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ModelKind {
    Speech,
    Cleanup,
    SystemCleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ModelStatus {
    Missing,
    Ready,
    NeedsVerification,
    Damaged,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub kind: ModelKind,
    pub bytes: u64,
    pub recommended: bool,
    pub status: ModelStatus,
}

#[uniffi::export(with_foreign)]
pub trait DownloadProgress: Send + Sync {
    fn bytes(&self, done: u64, total: u64);
    fn verifying(&self);
}

#[derive(uniffi::Object)]
pub struct ModelRepository {
    data: PathBuf,
    physical_memory_bytes: u64,
    busy: AtomicBool,
    cancel: AtomicBool,
}

struct Downloading<'a>(&'a AtomicBool);

impl Drop for Downloading<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[uniffi::export]
impl ModelRepository {
    #[uniffi::constructor]
    pub fn new(data: String) -> Result<Arc<Self>, MobileError> {
        Self::with_memory(data, 0)
    }

    #[uniffi::constructor]
    pub fn with_memory(data: String, physical_memory_bytes: u64) -> Result<Arc<Self>, MobileError> {
        let data = PathBuf::from(data);
        if !data.is_absolute() {
            return Err(MobileError::invalid(
                "the data folder must be an absolute path",
            ));
        }
        std::fs::create_dir_all(data.join("cache/downloads")).map_err(MobileError::failed)?;
        std::fs::create_dir_all(data.join("models/whisper")).map_err(MobileError::failed)?;
        std::fs::create_dir_all(data.join("models/llm")).map_err(MobileError::failed)?;
        model_files::sweep_partials(
            &data.join("cache/downloads"),
            std::time::Duration::from_secs(7 * 24 * 3600),
        );
        Ok(Arc::new(Self {
            data,
            physical_memory_bytes,
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        }))
    }

    pub fn models(&self) -> Vec<Model> {
        let mut models: Vec<_> = catalog::phone_models(self.physical_memory_bytes)
            .into_iter()
            .map(|entry| {
                let pin = &entry.files[0];
                Model {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    kind: if entry.kind == catalog::ModelKind::Cleanup {
                        ModelKind::Cleanup
                    } else {
                        ModelKind::Speech
                    },
                    bytes: entry.bytes(),
                    recommended: entry.default,
                    status: match model_files::check(&self.path(entry), pin.bytes, &pin.sha256) {
                        Check::Missing => ModelStatus::Missing,
                        Check::Ready => ModelStatus::Ready,
                        Check::NeedsHash => ModelStatus::NeedsVerification,
                        Check::WrongSize => ModelStatus::Damaged,
                    },
                }
            })
            .collect();
        let cleanup = models
            .iter()
            .position(|model| model.kind == ModelKind::Cleanup)
            .unwrap_or(models.len());
        models.insert(
            cleanup,
            Model {
                id: "system".into(),
                name: "System model".into(),
                kind: ModelKind::SystemCleanup,
                bytes: 0,
                recommended: true,
                status: ModelStatus::System,
            },
        );
        models
    }

    /// Runs on the caller's worker thread. A cancelled download returns false.
    pub fn download(
        &self,
        id: String,
        progress: Option<Arc<dyn DownloadProgress>>,
    ) -> Result<bool, MobileError> {
        let entry = self.model(&id)?;
        let _busy = self.begin()?;
        let pin = &entry.files[0];
        let dest = self.path(entry);
        if self.verify_entry(entry)? {
            return Ok(true);
        }
        let partial = self
            .data
            .join("cache/downloads")
            .join(format!("{}.download", pin.file));
        let existing =
            std::fs::metadata(&partial).map_or(0, |metadata| metadata.len().min(pin.bytes));
        let free = model_files::free_bytes(&self.data).map_err(MobileError::failed)?;
        if !model_files::has_room(free, pin.bytes - existing) {
            return Err(MobileError::failed(hushpen_core::error::MODEL_NO_SPACE));
        }
        let policy = Policy::model_download();
        let downloader = download::Downloader::new(policy, Box::new(UreqFetcher::new(&policy)));
        let outcome = downloader
            .run(
                &download::Spec {
                    url: pin.url.clone(),
                    bytes: pin.bytes,
                    sha256: pin.sha256.clone(),
                    partial,
                    dest,
                },
                &self.cancel,
                &mut |event| {
                    if let Some(progress) = &progress {
                        match event {
                            download::Progress::Bytes { done, total } => {
                                progress.bytes(done, total)
                            }
                            download::Progress::Verifying => progress.verifying(),
                        }
                    }
                },
            )
            .map_err(|failure| {
                MobileError::failed(format!("{}: {}", failure.code, failure.detail))
            })?;
        Ok(outcome == download::Outcome::Done)
    }

    pub fn cancel_download(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    pub fn selection_after_download(
        &self,
        current: String,
        downloaded: String,
    ) -> Result<String, MobileError> {
        let entry = self.model(&downloaded)?;
        if entry.kind != catalog::ModelKind::Speech {
            return Err(MobileError::invalid("choose a speech model for dictation"));
        }
        selection_after_download(&self.models(), &current, &downloaded)
    }

    pub fn verify(&self, id: String) -> Result<bool, MobileError> {
        let entry = self.model(&id)?;
        let _busy = self.begin()?;
        self.verify_entry(entry)
    }

    pub fn model_path(&self, id: String) -> Result<String, MobileError> {
        let entry = self.model(&id)?;
        let _busy = self.begin()?;
        if !self.verify_entry(entry)? {
            return Err(MobileError::failed("the model has not passed verification"));
        }
        Ok(self.path(entry).to_string_lossy().into_owned())
    }
}

fn selection_after_download(
    models: &[Model],
    current: &str,
    downloaded: &str,
) -> Result<String, MobileError> {
    use hushpen_core::models::{self, ModelState};
    if !models
        .iter()
        .any(|model| model.id == downloaded && model.status == ModelStatus::Ready)
    {
        return Err(MobileError::failed(
            "the downloaded model has not passed verification",
        ));
    }
    let current = models::effective(current, Some(catalog::SPEECH_DEFAULT));
    Ok(if models::should_adopt(
        catalog::ModelKind::Speech,
        downloaded,
        current,
        current,
        models.iter().map(|model| {
            (
                model.id.as_str(),
                if model.status == ModelStatus::Ready {
                    ModelState::Ready
                } else {
                    ModelState::NotDownloaded
                },
                false,
            )
        }),
    ) {
        downloaded
    } else {
        current
    }
    .to_owned())
}

impl ModelRepository {
    fn model(&self, id: &str) -> Result<&'static ModelEntry, MobileError> {
        catalog::phone_models(self.physical_memory_bytes)
            .into_iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| MobileError::invalid("this model is not available for this phone"))
    }

    fn path(&self, entry: &ModelEntry) -> PathBuf {
        let folder = if entry.kind == catalog::ModelKind::Cleanup {
            "models/llm"
        } else {
            "models/whisper"
        };
        self.data.join(folder).join(&entry.files[0].file)
    }

    fn begin(&self) -> Result<Downloading<'_>, MobileError> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(MobileError::failed("a model operation is already running"));
        }
        self.cancel.store(false, Ordering::Release);
        Ok(Downloading(&self.busy))
    }

    fn verify_entry(&self, entry: &ModelEntry) -> Result<bool, MobileError> {
        let pin = &entry.files[0];
        verify_file(&self.path(entry), pin.bytes, &pin.sha256, &self.cancel)
    }
}

fn verify_file(
    path: &Path,
    bytes: u64,
    sha256: &str,
    cancel: &AtomicBool,
) -> Result<bool, MobileError> {
    match model_files::check(path, bytes, sha256) {
        Check::Ready => Ok(true),
        Check::Missing | Check::WrongSize => Ok(false),
        Check::NeedsHash => {
            model_files::verify(path, bytes, sha256, |_| !cancel.load(Ordering::Acquire))
                .map(|verdict| matches!(verdict, Verdict::Ready { .. }))
                .or_else(|error| {
                    if error.kind() == std::io::ErrorKind::Interrupted {
                        Ok(false)
                    } else {
                        Err(error)
                    }
                })
                .map_err(MobileError::failed)
        }
    }
}

pub(crate) fn phone_model(id: &str) -> Result<&'static ModelEntry, MobileError> {
    catalog::phone_models(u64::MAX)
        .into_iter()
        .find(|entry| entry.kind == catalog::ModelKind::Speech && entry.id == id)
        .ok_or_else(|| MobileError::failed("the phone catalog is missing a speech model"))
}

pub(crate) fn cleanup_model(id: &str) -> Result<&'static ModelEntry, MobileError> {
    catalog::phone_models(u64::MAX)
        .into_iter()
        .find(|entry| entry.kind == catalog::ModelKind::Cleanup && entry.id == id)
        .ok_or_else(|| MobileError::invalid("this model is not a phone cleanup model"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_phone_memory_offers_small_speech_models_and_system_cleanup() {
        let directory = tempfile::tempdir().unwrap();
        let repository =
            ModelRepository::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let models = repository.models();
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            [catalog::SPEECH_DEFAULT, "base-q5_1", "small-q5_1", "system"]
        );
        assert_eq!(
            models.iter().map(|model| model.status).collect::<Vec<_>>(),
            [
                ModelStatus::Missing,
                ModelStatus::Missing,
                ModelStatus::Missing,
                ModelStatus::System
            ]
        );
        assert!(repository.model_path("../../outside".into()).is_err());
        assert!(repository.download("s1-mini".into(), None).is_err());
    }

    #[test]
    fn repository_operations_enforce_phone_memory_and_model_kind() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_string_lossy().into_owned();
        let repository = ModelRepository::with_memory(root.clone(), 6_000_000_000).unwrap();
        assert!(
            repository.models().iter().any(|model| {
                model.id == catalog::SPEECH_TURBO && model.kind == ModelKind::Speech
            })
        );
        let cleanup = repository.model(catalog::LLM_CLEANUP).unwrap();
        assert_eq!(
            repository.path(cleanup),
            directory.path().join("models/llm/s1-mini.gguf")
        );
        assert!(repository.verify(catalog::LLM_GENERAL.into()).is_err());
        assert!(
            repository
                .download(catalog::LLM_GENERAL.into(), None)
                .is_err()
        );
        assert!(repository.model_path(catalog::LLM_GENERAL.into()).is_err());
        assert!(
            repository
                .selection_after_download(String::new(), catalog::LLM_CLEANUP.into())
                .is_err()
        );
        assert!(phone_model(catalog::LLM_CLEANUP).is_err());
        assert!(cleanup_model(catalog::SPEECH_DEFAULT).is_err());
        let repository = ModelRepository::with_memory(root, 8_000_000_000).unwrap();
        assert_eq!(repository.models().len(), 7);
        assert_eq!(
            repository.model(catalog::LLM_GENERAL).unwrap().kind,
            catalog::ModelKind::Cleanup
        );
    }

    #[test]
    fn verification_uses_pinned_size_and_hash_and_rejects_changed_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("model.bin");
        let cancel = AtomicBool::new(false);
        assert!(!verify_file(&path, 3, "unused", &cancel).unwrap());
        std::fs::write(&path, b"abc").unwrap();
        let sha = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_file(&path, 3, sha, &cancel).unwrap());
        assert!(model_files::is_verified(&path));
        assert!(!verify_file(&path, 3, &"0".repeat(64), &cancel).unwrap());
        assert!(!model_files::is_verified(&path));
    }

    #[test]
    fn a_download_keeps_a_ready_default_and_replaces_a_missing_one() {
        let model = |id: &str, status| Model {
            id: id.into(),
            name: id.into(),
            kind: ModelKind::Speech,
            bytes: 0,
            recommended: false,
            status,
        };
        let mut rows = vec![
            model(catalog::SPEECH_DEFAULT, ModelStatus::Ready),
            model("base-q5_1", ModelStatus::Ready),
        ];
        assert_eq!(
            selection_after_download(&rows, catalog::SPEECH_DEFAULT, "base-q5_1").unwrap(),
            catalog::SPEECH_DEFAULT
        );
        assert_eq!(
            selection_after_download(&rows, "base-q5_1", catalog::SPEECH_DEFAULT).unwrap(),
            "base-q5_1"
        );
        rows[0].status = ModelStatus::Missing;
        assert_eq!(
            selection_after_download(&rows, catalog::SPEECH_DEFAULT, "base-q5_1").unwrap(),
            "base-q5_1"
        );
        assert!(selection_after_download(&rows, "base-q5_1", catalog::SPEECH_DEFAULT).is_err());
    }
}
