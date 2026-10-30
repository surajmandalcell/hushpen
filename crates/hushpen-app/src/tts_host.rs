//! The app's side of the Kokoro voice worker: starts the `hushpen-tts` child through the
//! supervisor at the first read aloud, and only when every file of the voice model is there and
//! verified. A user who never reads aloud with Kokoro never starts the process. The worker drops
//! the model by itself after five idle minutes.
//!
//! [`TtsHost`] is a [`SpeechEngine`], so Read aloud streams and cancels Kokoro the same way as
//! the system voice. A worker crash or hang ends the chunk in a `SPEECH_*` error, never a wait
//! without end.

use crate::worker_bin::worker_path;
use hushpen_core::error;
use hushpen_core::kokoro::{self, VOICES};
use hushpen_core::threads::auto_threads;
use hushpen_core::tts_protocol::TtsEngine;
use hushpen_engine::{
    ChildSpec, EngineLog, Failure, Timing, TtsClient, TtsOutcome, TtsReply, TtsSpec, TtsStatus,
    TtsWork,
};
use hushpen_speech::{Pcm, SpeechEngine, SpeechError, Voice, VoiceQuality};
use hushpen_store::data_dir::DataDir;
use hushpen_store::model_files;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// The longest wait for the next piece of audio: a cold model load and one long sentence on a
/// slow CPU, with room to spare.
const PIECE_TIMEOUT: Duration = Duration::from_secs(30);

struct State {
    client: Option<TtsClient>,
    /// The load the running worker was last given.
    applied: Option<TtsSpec>,
}

pub struct TtsHost {
    models_dir: PathBuf,
    log_path: PathBuf,
    program: PathBuf,
    state: Mutex<State>,
}

impl TtsHost {
    /// Starts nothing. The first [`SpeechEngine::speak`] starts the worker.
    pub fn new(data: &DataDir) -> Self {
        Self::with_program(data, worker_path("hushpen-tts", "HUSHPEN_TTS_BIN"))
    }

    pub(crate) fn with_program(data: &DataDir, program: PathBuf) -> Self {
        Self {
            models_dir: data.tts_models_dir(),
            log_path: data.tts_log_path(),
            program,
            state: Mutex::new(State {
                client: None,
                applied: None,
            }),
        }
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

    /// The running worker, given the installed model. Starts the worker on first use.
    fn client(&self) -> Result<TtsClient, SpeechError> {
        let cpus = std::thread::available_parallelism().map_or(2, usize::from);
        let wanted = wanted_load(&self.models_dir, cpus).ok_or_else(not_installed)?;
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
}

impl SpeechEngine for TtsHost {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        wanted_load(&self.models_dir, 1).ok_or_else(not_installed)?;
        Ok(VOICES
            .iter()
            .map(|voice| Voice {
                id: voice.id.to_owned(),
                name: voice.name(),
                language: voice.language().to_owned(),
                quality: VoiceQuality::Premium,
            })
            .collect())
    }

    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
        let job = self.client()?.run(TtsWork {
            text: text.to_owned(),
            voice: voice.map(str::to_owned),
            speed,
        });
        let mut pcm = Pcm {
            rate: kokoro::SAMPLE_RATE,
            samples: Vec::new(),
        };
        loop {
            match job.next(PIECE_TIMEOUT) {
                Some(TtsReply::Progress(audio)) => {
                    pcm.rate = audio.sample_rate;
                    pcm.samples.extend(audio.samples);
                }
                Some(TtsReply::End(TtsOutcome::Done(_))) => return Ok(pcm),
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

fn not_installed() -> SpeechError {
    SpeechError::new(
        error::SPEECH_UNAVAILABLE,
        "the Kokoro voice model is not downloaded",
    )
}

/// The worker's code as a Read aloud code. The worker's own code stays in the detail.
fn speech_error(failure: &Failure) -> SpeechError {
    let code = match failure.code.as_str() {
        error::SPEECH_VOICE_MISSING => error::SPEECH_VOICE_MISSING,
        error::SPEECH_NO_TEXT => error::SPEECH_NO_TEXT,
        error::TTS_UNAVAILABLE | error::TTS_LOAD_FAILED | error::TTS_NO_MODEL => {
            error::SPEECH_UNAVAILABLE
        }
        _ => error::SPEECH_FAILED,
    };
    SpeechError::new(code, format!("{} {}", failure.code, failure.detail))
}

/// The model to load, or `None` while a file of the Kokoro entry is missing or has no matching
/// `.verified` stamp.
pub fn wanted_load(models_dir: &Path, cpus: usize) -> Option<TtsSpec> {
    let dir = models_dir.join(kokoro::MODEL_ID);
    if !kokoro::FILES
        .iter()
        .all(|file| model_files::is_verified(&dir.join(file)))
    {
        return None;
    }
    Some(TtsSpec {
        threads: auto_threads(cpus),
        ..TtsSpec::new(TtsEngine::Kokoro, dir)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamped_model(models_dir: &Path) -> PathBuf {
        let dir = models_dir.join(kokoro::MODEL_ID);
        std::fs::create_dir_all(&dir).unwrap();
        for file in kokoro::FILES {
            let path = dir.join(file);
            std::fs::write(&path, file.as_bytes()).unwrap();
            model_files::write_stamp(&path, "abc").unwrap();
        }
        dir
    }

    #[test]
    fn a_verified_model_gives_its_folder_and_the_threads() {
        let models = tempfile::tempdir().unwrap();
        let dir = stamped_model(models.path());
        let wanted = wanted_load(models.path(), 16).unwrap();
        assert_eq!(wanted.dir, dir);
        assert_eq!(wanted.threads, 8);
        assert_eq!(wanted.idle_secs, 300);
        assert_eq!(wanted_load(models.path(), 2).unwrap().threads, 2);
    }

    #[test]
    fn one_missing_or_changed_file_wants_no_worker() {
        let models = tempfile::tempdir().unwrap();
        assert_eq!(wanted_load(models.path(), 4), None);
        let dir = stamped_model(models.path());
        std::fs::write(dir.join(kokoro::TOKENS_FILE), b"changed tokens").unwrap();
        assert_eq!(wanted_load(models.path(), 4), None);
        stamped_model(models.path());
        std::fs::remove_file(dir.join(kokoro::LEXICON_FILE)).unwrap();
        assert_eq!(wanted_load(models.path(), 4), None);
    }

    #[test]
    fn without_the_model_there_are_no_voices_and_no_worker() {
        let data_dir = tempfile::tempdir().unwrap();
        let data = DataDir::open(data_dir.path()).unwrap();
        let host = TtsHost::with_program(&data, PathBuf::from("/bin/sh"));
        assert_eq!(host.voices().unwrap_err().code, error::SPEECH_UNAVAILABLE);
        assert_eq!(
            host.speak("Hello.", None, 1.0).unwrap_err().code,
            error::SPEECH_UNAVAILABLE
        );
        assert!(host.status().is_none(), "nothing starts without the model");
    }

    #[test]
    fn with_the_model_the_english_voices_are_listed() {
        let data_dir = tempfile::tempdir().unwrap();
        let data = DataDir::open(data_dir.path()).unwrap();
        stamped_model(&data.tts_models_dir());
        let host = TtsHost::with_program(&data, PathBuf::from("/bin/sh"));
        let voices = host.voices().unwrap();
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
    fn worker_codes_become_read_aloud_codes() {
        let cases = [
            (error::SPEECH_VOICE_MISSING, error::SPEECH_VOICE_MISSING),
            (error::SPEECH_NO_TEXT, error::SPEECH_NO_TEXT),
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
