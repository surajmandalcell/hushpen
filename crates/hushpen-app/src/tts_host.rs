//! The app's side of the voice worker: starts the `hushpen-tts` child through the supervisor at
//! the first read aloud with a voice model, and only when every file of that model is there
//! and verified. A user who never reads aloud with a model never starts the process. One worker
//! serves Kokoro, Chatterbox Turbo, and Chatterbox Nano: a read with another model loads that
//! model in place of the last. The worker drops the model by itself after five idle minutes.
//!
//! [`TtsHost::engine`] gives each model as a [`SpeechEngine`], so Read aloud streams and cancels
//! it the same way as the system voice. A worker crash or hang ends the chunk in a `SPEECH_*`
//! error, never a wait without end.

use crate::worker_bin::worker_path;
use hushpen_core::chatterbox::{self, BUILT_IN_VOICE};
use hushpen_core::error;
use hushpen_core::kokoro::{self, VOICES};
use hushpen_core::threads::auto_threads;
use hushpen_core::tts_protocol::TtsEngine;
use hushpen_engine::{
    ChildSpec, EngineLog, Failure, Timing, TtsClient, TtsJobHandle, TtsOutcome, TtsReply, TtsSpec,
    TtsStatus, TtsWork,
};
use hushpen_speech::{Pcm, SpeechEngine, SpeechError, Voice, VoiceQuality};
use hushpen_store::data_dir::DataDir;
use hushpen_store::model_files;
use hushpen_store::voices::Library;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The longest wait for the next piece of audio: a cold model load and one long sentence on a
/// slow CPU, with room to spare.
const PIECE_TIMEOUT: Duration = Duration::from_secs(30);

/// A voice model the worker runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceModel {
    Kokoro,
    ChatterboxTurbo,
    ChatterboxNano,
}

impl VoiceModel {
    /// The catalog id, also the model's folder in `models/tts/`.
    pub fn id(self) -> &'static str {
        match self {
            VoiceModel::Kokoro => kokoro::MODEL_ID,
            VoiceModel::ChatterboxTurbo => chatterbox::TURBO_ID,
            VoiceModel::ChatterboxNano => chatterbox::NANO_ID,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            VoiceModel::Kokoro => "Kokoro",
            VoiceModel::ChatterboxTurbo => "Chatterbox Turbo",
            VoiceModel::ChatterboxNano => "Chatterbox Nano",
        }
    }

    fn engine(self) -> TtsEngine {
        match self {
            VoiceModel::Kokoro => TtsEngine::Kokoro,
            VoiceModel::ChatterboxTurbo | VoiceModel::ChatterboxNano => TtsEngine::Chatterbox,
        }
    }

    fn files(self) -> &'static [&'static str] {
        match self {
            VoiceModel::Kokoro => &kokoro::FILES,
            VoiceModel::ChatterboxTurbo | VoiceModel::ChatterboxNano => &chatterbox::FILES,
        }
    }
}

struct State {
    client: Option<TtsClient>,
    /// The load the running worker was last given.
    applied: Option<TtsSpec>,
}

pub struct TtsHost {
    models_dir: PathBuf,
    voices: Library,
    log_path: PathBuf,
    program: PathBuf,
    state: Mutex<State>,
}

impl TtsHost {
    /// Starts nothing. The first [`SpeechEngine::speak`] of a model starts the worker.
    pub fn new(data: &DataDir) -> Self {
        Self::with_program(data, worker_path("hushpen-tts", "HUSHPEN_TTS_BIN"))
    }

    pub(crate) fn with_program(data: &DataDir, program: PathBuf) -> Self {
        Self {
            models_dir: data.tts_models_dir(),
            voices: Library::new(data.voices_dir()),
            log_path: data.tts_log_path(),
            program,
            state: Mutex::new(State {
                client: None,
                applied: None,
            }),
        }
    }

    /// `model` as an engine of Read aloud.
    pub fn engine(self: &Arc<Self>, model: VoiceModel) -> Arc<dyn SpeechEngine> {
        Arc::new(ModelVoice {
            host: Arc::clone(self),
            model,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The worker's status, or `None` while none runs.
    pub fn status(&self) -> Option<TtsStatus> {
        self.lock().client.as_ref().map(TtsClient::status)
    }

    /// The running worker with `model` loaded or loading. Starts the worker on first use.
    fn client(&self, model: VoiceModel) -> Result<TtsClient, SpeechError> {
        let cpus = std::thread::available_parallelism().map_or(2, usize::from);
        let wanted =
            wanted_load(&self.models_dir, model, cpus).ok_or_else(|| not_installed(model))?;
        let mut state = self.lock();
        let client = state
            .client
            .get_or_insert_with(|| self.start_client())
            .clone();
        if state.applied.as_ref() != Some(&wanted) {
            state.applied = Some(wanted.clone());
            let handle = client.load_model(wanted);
            // The wait must not block the caller; the outcome needs a log line.
            std::thread::spawn(move || match handle.wait() {
                Ok(done) => log::info!("TTS_MODEL_LOADED model={} ms={}", done.model, done.ms),
                Err(failure) => {
                    log::warn!("TTS_MODEL_FAILED {} {}", failure.code, failure.detail);
                }
            });
        }
        Ok(client)
    }

    fn start_client(&self) -> TtsClient {
        let log = EngineLog::open_scoped(&self.log_path, "tts").unwrap_or_else(|error| {
            log::warn!("TTS_LOG_OFF could not open tts.log: {error}");
            EngineLog::discard()
        });
        let spec = ChildSpec {
            program: self.program.clone(),
            args: Vec::<OsString>::new(),
        };
        TtsClient::start(spec, Timing::default(), log)
    }

    /// Stops the worker. Safe to call twice.
    pub fn shutdown(&self) {
        let client = {
            let mut state = self.lock();
            state.applied = None;
            state.client.take()
        };
        if let Some(client) = client {
            client.shutdown();
        }
    }

    /// What the worker takes as the voice: a Kokoro id, or the clip of a Chatterbox voice.
    fn voice_param(
        &self,
        model: VoiceModel,
        voice: Option<&str>,
    ) -> Result<Option<String>, SpeechError> {
        match (model, voice) {
            (VoiceModel::Kokoro, voice) => Ok(voice.map(str::to_owned)),
            (_, None) => Ok(None),
            (_, Some(id)) if id == BUILT_IN_VOICE => Ok(None),
            (_, Some(id)) => self
                .voices
                .get(id)
                .and_then(|_| self.voices.clip_path(id))
                .map(|clip| Some(clip.to_string_lossy().into_owned()))
                .ok_or_else(|| {
                    SpeechError::new(error::SPEECH_VOICE_MISSING, format!("no voice `{id}`"))
                }),
        }
    }
}

/// One voice model of the worker, as Read aloud sees it.
struct ModelVoice {
    host: Arc<TtsHost>,
    model: VoiceModel,
}

impl SpeechEngine for ModelVoice {
    fn available(&self) -> bool {
        downloaded(&self.host.models_dir, self.model)
    }

    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        if !downloaded(&self.host.models_dir, self.model) {
            return Err(not_installed(self.model));
        }
        Ok(match self.model {
            VoiceModel::Kokoro => VOICES
                .iter()
                .map(|voice| Voice {
                    id: voice.id.to_owned(),
                    name: voice.name(),
                    language: voice.language().to_owned(),
                    quality: VoiceQuality::Premium,
                })
                .collect(),
            VoiceModel::ChatterboxTurbo | VoiceModel::ChatterboxNano => {
                let cloned = self.host.voices.list().into_iter().map(|voice| Voice {
                    id: voice.id,
                    name: voice.name,
                    language: "en".to_owned(),
                    quality: VoiceQuality::Premium,
                });
                std::iter::once(built_in_voice()).chain(cloned).collect()
            }
        })
    }

    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
        let mut pcm = Pcm {
            rate: kokoro::SAMPLE_RATE,
            samples: Vec::new(),
        };
        self.speak_into(text, voice, speed, &mut |piece| {
            pcm.rate = piece.rate;
            pcm.samples.extend(piece.samples);
            true
        })?;
        Ok(pcm)
    }

    fn speak_into(
        &self,
        text: &str,
        voice: Option<&str>,
        speed: f32,
        sink: &mut dyn FnMut(Pcm) -> bool,
    ) -> Result<(), SpeechError> {
        let voice = self.host.voice_param(self.model, voice)?;
        let job = self.host.client(self.model)?.run(TtsWork {
            text: text.to_owned(),
            voice,
            speed,
        });
        loop {
            match job.next(PIECE_TIMEOUT) {
                Some(TtsReply::Progress(audio)) => {
                    let piece = Pcm {
                        rate: audio.sample_rate,
                        samples: audio.samples,
                    };
                    if !sink(piece) {
                        job.cancel();
                        drain(&job);
                        return Ok(());
                    }
                }
                Some(TtsReply::End(TtsOutcome::Done(_))) => return Ok(()),
                Some(TtsReply::End(TtsOutcome::Cancelled)) => {
                    return Err(SpeechError::failed("the voice worker cancelled the chunk"));
                }
                Some(TtsReply::End(TtsOutcome::Failed(failure))) => {
                    return Err(speech_error(&failure));
                }
                None => {
                    job.cancel();
                    return Err(SpeechError::failed(
                        "the voice worker sent no audio in time",
                    ));
                }
            }
        }
    }
}

/// Waits for a cancelled job to end, so the next chunk never meets its last audio.
fn drain(job: &TtsJobHandle) {
    while let Some(TtsReply::Progress(_)) = job.next(PIECE_TIMEOUT) {}
}

/// The voice that comes with a Chatterbox model, listed before the cloned ones.
pub fn built_in_voice() -> Voice {
    Voice {
        id: BUILT_IN_VOICE.to_owned(),
        name: "Built-in".to_owned(),
        language: "en".to_owned(),
        quality: VoiceQuality::Premium,
    }
}

fn not_installed(model: VoiceModel) -> SpeechError {
    SpeechError::new(
        error::SPEECH_UNAVAILABLE,
        format!("the {} voice model is not downloaded", model.name()),
    )
}

/// The worker's code as a Read aloud code. The worker's own code stays in the detail.
fn speech_error(failure: &Failure) -> SpeechError {
    let code = match failure.code.as_str() {
        error::SPEECH_VOICE_MISSING => error::SPEECH_VOICE_MISSING,
        error::SPEECH_NO_TEXT => error::SPEECH_NO_TEXT,
        error::SPEECH_NOT_ENGLISH => error::SPEECH_NOT_ENGLISH,
        error::SPEECH_VOICE_CLIP => error::SPEECH_VOICE_CLIP,
        error::TTS_UNAVAILABLE | error::TTS_LOAD_FAILED | error::TTS_NO_MODEL => {
            error::SPEECH_UNAVAILABLE
        }
        _ => error::SPEECH_FAILED,
    };
    SpeechError::new(code, format!("{} {}", failure.code, failure.detail))
}

/// Whether every file of `model` is there with a matching `.verified` stamp.
pub fn downloaded(models_dir: &Path, model: VoiceModel) -> bool {
    let dir = models_dir.join(model.id());
    model
        .files()
        .iter()
        .all(|file| model_files::is_verified(&dir.join(file)))
}

/// The load for `model`, or `None` while it is not [`downloaded`].
pub fn wanted_load(models_dir: &Path, model: VoiceModel, cpus: usize) -> Option<TtsSpec> {
    downloaded(models_dir, model).then(|| TtsSpec {
        threads: auto_threads(cpus),
        ..TtsSpec::new(model.engine(), models_dir.join(model.id()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamped_model(models_dir: &Path, model: VoiceModel) -> PathBuf {
        let dir = models_dir.join(model.id());
        std::fs::create_dir_all(&dir).unwrap();
        for file in model.files() {
            let path = dir.join(file);
            std::fs::write(&path, file.as_bytes()).unwrap();
            model_files::write_stamp(&path, "abc").unwrap();
        }
        dir
    }

    fn host(data: &DataDir) -> Arc<TtsHost> {
        Arc::new(TtsHost::with_program(data, PathBuf::from("/bin/sh")))
    }

    #[test]
    fn a_verified_model_gives_its_folder_its_engine_and_the_threads() {
        let models = tempfile::tempdir().unwrap();
        let dir = stamped_model(models.path(), VoiceModel::Kokoro);
        let wanted = wanted_load(models.path(), VoiceModel::Kokoro, 16).unwrap();
        assert_eq!(wanted.dir, dir);
        assert_eq!(wanted.engine, TtsEngine::Kokoro);
        assert_eq!(wanted.threads, 8);
        assert_eq!(wanted.idle_secs, 300);
        assert_eq!(
            wanted_load(models.path(), VoiceModel::Kokoro, 2)
                .unwrap()
                .threads,
            2
        );
        assert_eq!(
            wanted_load(models.path(), VoiceModel::ChatterboxNano, 4),
            None
        );
        let nano = stamped_model(models.path(), VoiceModel::ChatterboxNano);
        let wanted = wanted_load(models.path(), VoiceModel::ChatterboxNano, 4).unwrap();
        assert_eq!((wanted.dir, wanted.engine), (nano, TtsEngine::Chatterbox));
    }

    #[test]
    fn one_missing_or_changed_file_wants_no_worker() {
        let models = tempfile::tempdir().unwrap();
        assert_eq!(wanted_load(models.path(), VoiceModel::Kokoro, 4), None);
        let dir = stamped_model(models.path(), VoiceModel::Kokoro);
        std::fs::write(dir.join(kokoro::TOKENS_FILE), b"changed tokens").unwrap();
        assert_eq!(wanted_load(models.path(), VoiceModel::Kokoro, 4), None);
        stamped_model(models.path(), VoiceModel::Kokoro);
        std::fs::remove_file(dir.join(kokoro::LEXICON_FILE)).unwrap();
        assert_eq!(wanted_load(models.path(), VoiceModel::Kokoro, 4), None);
        let turbo = stamped_model(models.path(), VoiceModel::ChatterboxTurbo);
        std::fs::remove_file(turbo.join(chatterbox::DEFAULT_VOICE_FILE)).unwrap();
        assert!(!downloaded(models.path(), VoiceModel::ChatterboxTurbo));
    }

    #[test]
    fn without_the_model_there_are_no_voices_and_no_worker() {
        let data_dir = tempfile::tempdir().unwrap();
        let data = DataDir::open(data_dir.path()).unwrap();
        let host = host(&data);
        for model in [VoiceModel::Kokoro, VoiceModel::ChatterboxTurbo] {
            let engine = host.engine(model);
            assert_eq!(engine.voices().unwrap_err().code, error::SPEECH_UNAVAILABLE);
            let refused = engine.speak("Hello.", None, 1.0).unwrap_err();
            assert_eq!(refused.code, error::SPEECH_UNAVAILABLE);
            assert!(refused.detail.contains(model.name()), "{}", refused.detail);
        }
        assert!(host.status().is_none(), "nothing starts without the model");
    }

    #[test]
    fn with_the_model_the_english_voices_are_listed() {
        let data_dir = tempfile::tempdir().unwrap();
        let data = DataDir::open(data_dir.path()).unwrap();
        stamped_model(&data.tts_models_dir(), VoiceModel::Kokoro);
        let host = host(&data);
        let voices = host.engine(VoiceModel::Kokoro).voices().unwrap();
        assert_eq!(voices.len(), VOICES.len());
        let heart = voices
            .iter()
            .find(|v| v.id == kokoro::DEFAULT_VOICE)
            .unwrap();
        assert_eq!(
            (heart.name.as_str(), heart.language.as_str()),
            ("Heart", "en-US")
        );
        assert!(host.status().is_none(), "listing voices starts no worker");
    }

    #[test]
    fn chatterbox_lists_its_own_voice_then_the_cloned_ones_and_reads_with_their_clips() {
        let data_dir = tempfile::tempdir().unwrap();
        let data = DataDir::open(data_dir.path()).unwrap();
        stamped_model(&data.tts_models_dir(), VoiceModel::ChatterboxNano);
        let library = Library::new(data.voices_dir());
        let anna = library
            .add("Anna", 6_000, 1, |path| std::fs::write(path, b"clip"))
            .unwrap();
        let host = host(&data);
        let voices = host.engine(VoiceModel::ChatterboxNano).voices().unwrap();
        let names: Vec<&str> = voices.iter().map(|voice| voice.name.as_str()).collect();
        assert_eq!(names, ["Built-in", "Anna"]);
        assert!(voices.iter().all(|voice| voice.language == "en"));
        let model = VoiceModel::ChatterboxNano;
        assert_eq!(host.voice_param(model, None).unwrap(), None);
        assert_eq!(host.voice_param(model, Some(BUILT_IN_VOICE)).unwrap(), None);
        let clip = host.voice_param(model, Some(&anna.id)).unwrap().unwrap();
        assert_eq!(Path::new(&clip), library.clip_path(&anna.id).unwrap());
        for gone in ["v404", "../etc"] {
            let missing = host.voice_param(model, Some(gone)).unwrap_err();
            assert_eq!(missing.code, error::SPEECH_VOICE_MISSING);
        }
        assert_eq!(
            host.voice_param(VoiceModel::Kokoro, Some("af_bella"))
                .unwrap()
                .as_deref(),
            Some("af_bella")
        );
    }

    #[test]
    fn worker_codes_become_read_aloud_codes() {
        let cases = [
            (error::SPEECH_VOICE_MISSING, error::SPEECH_VOICE_MISSING),
            (error::SPEECH_NO_TEXT, error::SPEECH_NO_TEXT),
            (error::SPEECH_NOT_ENGLISH, error::SPEECH_NOT_ENGLISH),
            (error::SPEECH_VOICE_CLIP, error::SPEECH_VOICE_CLIP),
            (error::TTS_LOAD_FAILED, error::SPEECH_UNAVAILABLE),
            (error::TTS_UNAVAILABLE, error::SPEECH_UNAVAILABLE),
            (error::TTS_CRASHED, error::SPEECH_FAILED),
        ];
        for (worker, speech) in cases {
            let failure = Failure::new(worker, "why");
            let mapped = speech_error(&failure);
            assert_eq!(mapped.code, speech);
            assert!(mapped.detail.starts_with(worker), "{}", mapped.detail);
        }
    }
}
