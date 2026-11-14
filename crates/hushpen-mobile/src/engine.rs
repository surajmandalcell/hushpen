use crate::{MobileError, lock};
use hushpen_core::idle::Idle;
use hushpen_engine::{
    EngineClient, EngineLog, InProcess, JobOutcome, Link, LoadSpec, Timing, TranscribeSpec,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Transcript {
    pub text: String,
    pub language: String,
    pub audio_ms: u64,
}

#[derive(uniffi::Object)]
pub struct Engine {
    pub(crate) data: PathBuf,
    pub(crate) client: EngineClient,
    active: Mutex<Option<u64>>,
    pub(crate) operation: Arc<Mutex<()>>,
}

#[uniffi::export]
impl Engine {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>, MobileError> {
        let data = PathBuf::from(data_dir);
        if !data.is_absolute() {
            return Err(MobileError::invalid("data folder must be absolute"));
        }
        std::fs::create_dir_all(&data).map_err(MobileError::failed)?;
        let detector = data
            .join("cache/resources")
            .join(hushpen_core::catalog::VAD_FILE);
        hushpen_store::model_files::install_bundled(
            &detector,
            include_bytes!("../../../assets/ggml-silero-v5.1.2.bin"),
            hushpen_core::catalog::VAD_SHA256,
        )
        .map_err(MobileError::failed)?;
        let log = EngineLog::open(&data.join("logs/engine.log")).map_err(MobileError::failed)?;
        Ok(Arc::new(Self {
            data,
            client: EngineClient::start_link(
                Link::InProcess(InProcess::speech_with_detector(detector)),
                Timing::default(),
                log,
            ),
            active: Mutex::new(None),
            operation: Arc::new(Mutex::new(())),
        }))
    }

    pub fn load(&self, model_id: String) -> Result<(), MobileError> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| MobileError::failed("speech engine is busy"))?;
        let entry = crate::models::phone_model(&model_id)?;
        let file = entry
            .files
            .first()
            .ok_or_else(|| MobileError::failed("model has no file"))?;
        let path = self.data.join("models/whisper").join(&file.file);
        if !hushpen_store::model_files::is_verified(&path)
            || hushpen_store::model_files::check(&path, file.bytes, &file.sha256)
                != hushpen_store::model_files::Check::Ready
        {
            return Err(MobileError::failed("download and verify the model first"));
        }
        let cpus = std::thread::available_parallelism().map_or(2, usize::from);
        self.client
            .load_model(LoadSpec {
                path,
                gpu: hushpen_engine::gpu_for_load(),
                threads: hushpen_core::threads::auto_threads(cpus),
                idle: Idle::KEEP,
            })
            .wait_timeout(Duration::from_secs(120))
            .ok_or_else(|| MobileError::failed("speech model load timed out"))?
            .map_err(|failure| MobileError::failed(failure.detail))?;
        Ok(())
    }

    pub fn transcribe(
        &self,
        wav_path: String,
        language: Option<String>,
    ) -> Result<Transcript, MobileError> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| MobileError::failed("speech engine is busy"))?;
        let path = Path::new(&wav_path);
        if !path.is_absolute() || !path.is_file() {
            return Err(MobileError::invalid(
                "recording must be an absolute file path",
            ));
        }
        let job = self.client.transcribe(TranscribeSpec {
            wav_path: path.into(),
            language,
            prompt: None,
        });
        *lock(&self.active) = Some(job.id());
        let outcome = job.wait_timeout(Duration::from_secs(300));
        *lock(&self.active) = None;
        match outcome {
            Some(JobOutcome::Done(done)) => Ok(Transcript {
                text: done.text,
                language: done.language,
                audio_ms: done.audio_ms,
            }),
            Some(JobOutcome::Cancelled) => Err(MobileError::failed("dictation cancelled")),
            Some(JobOutcome::Failed(failure)) => Err(MobileError::failed(failure.detail)),
            None => {
                job.cancel();
                Err(MobileError::failed("dictation timed out"))
            }
        }
    }

    pub fn cancel(&self) {
        if let Some(job) = *lock(&self.active) {
            self.client.cancel(job);
        }
    }

    pub fn unload(&self) -> Result<(), MobileError> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| MobileError::failed("speech engine is busy"))?;
        self.client.unload();
        Ok(())
    }

    pub fn shutdown(&self) {
        self.client.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_data_contains_the_pinned_detector_and_repairs_a_damaged_copy() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().to_string_lossy().into_owned();
        let engine = Engine::new(root.clone()).unwrap();
        let path = folder
            .path()
            .join("cache/resources")
            .join(hushpen_core::catalog::VAD_FILE);
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            hushpen_core::catalog::VAD_BYTES
        );
        assert_eq!(
            hushpen_store::model_files::hash_file(&path, |_| true).unwrap(),
            hushpen_core::catalog::VAD_SHA256
        );
        assert_eq!(
            hushpen_store::model_files::check(
                &path,
                hushpen_core::catalog::VAD_BYTES,
                hushpen_core::catalog::VAD_SHA256
            ),
            hushpen_store::model_files::Check::Ready
        );
        engine.shutdown();
        std::fs::write(&path, b"damaged").unwrap();
        let engine = Engine::new(root).unwrap();
        assert_eq!(
            hushpen_store::model_files::hash_file(&path, |_| true).unwrap(),
            hushpen_core::catalog::VAD_SHA256
        );
        engine.shutdown();
    }

    #[test]
    fn api_rejects_unknown_or_unverified_models_without_loading_native_code() {
        let folder = tempfile::tempdir().unwrap();
        let engine = Engine::new(folder.path().to_string_lossy().into()).unwrap();
        assert!(engine.load("unknown".into()).is_err());
        assert!(engine.load("parakeet-v3-q4_k".into()).is_err());
        assert!(engine.transcribe("relative.wav".into(), None).is_err());
        engine.shutdown();
        assert!(Engine::new("relative".into()).is_err());
    }
}
