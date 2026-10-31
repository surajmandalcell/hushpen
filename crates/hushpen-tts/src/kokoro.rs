//! The Kokoro model on sherpa-onnx: the worker's real backend.

use crate::backend::{Backend, CancelFlag, JobError, LoadError, LoadSpec, Session};
use crate::mix;
use crate::phonemes::Lexicon;
use crate::sherpa::{Api, KokoroFiles, Tts};
use hushpen_core::error::{SPEECH_FAILED, SPEECH_VOICE_MISSING};
use hushpen_core::kokoro::{
    DEFAULT_VOICE, FILES, LEXICON_FILE, MIX_DIR, MIX_SPEAKER, MODEL_FILE, SAMPLE_RATE, TOKENS_FILE,
    VOICES, VOICES_FILE, VoiceMix, find_voice,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Opens the library on the first load, so a worker without it still starts and answers.
pub struct SherpaBackend {
    library: PathBuf,
    api: Mutex<Option<Arc<Api>>>,
}

impl SherpaBackend {
    pub fn new(library: PathBuf) -> Self {
        Self {
            library,
            api: Mutex::new(None),
        }
    }

    /// The opened library. A failed open is tried again on the next call.
    pub fn api(&self) -> Result<Arc<Api>, String> {
        let mut api = self
            .api
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(api) = api.as_ref() {
            return Ok(Arc::clone(api));
        }
        let opened = Arc::new(Api::open(&self.library)?);
        *api = Some(Arc::clone(&opened));
        Ok(opened)
    }
}

impl Backend for SherpaBackend {
    fn load(&self, spec: &LoadSpec) -> Result<Box<dyn Session>, LoadError> {
        let failed = |detail: String| LoadError { detail };
        if let Some(missing) = FILES.iter().find(|file| !spec.dir.join(file).is_file()) {
            return Err(failed(format!("{} has no {missing}", spec.dir.display())));
        }
        let api = self.api().map_err(failed)?;
        let lexicon = Lexicon::read(&spec.dir.join(LEXICON_FILE)).map_err(failed)?;
        let voices = spec.dir.join(VOICES_FILE);
        let mut session = KokoroSession {
            api,
            model: spec.dir.join(MODEL_FILE),
            tokens: spec.dir.join(TOKENS_FILE),
            mixes: spec
                .dir
                .parent()
                .map_or_else(|| spec.dir.join(MIX_DIR), |tts| tts.join(MIX_DIR)),
            threads: i32::try_from(spec.threads).unwrap_or(i32::MAX),
            tts: None,
            loaded: voices.clone(),
            voices,
            lexicon,
        };
        let tts = session
            .load_voices(&session.voices.clone())
            .map_err(failed)?;
        let speakers = tts.speakers();
        if usize::try_from(speakers).unwrap_or(0) < VOICES.len() || tts.sample_rate() != SAMPLE_RATE
        {
            return Err(failed(format!(
                "not the Kokoro model: {speakers} speakers at {} Hz",
                tts.sample_rate()
            )));
        }
        Ok(Box::new(session))
    }
}

struct KokoroSession {
    api: Arc<Api>,
    model: PathBuf,
    tokens: PathBuf,
    /// The pinned `voices.bin`, never written.
    voices: PathBuf,
    /// Where the voices files of mixes go: `models/tts/mixes`.
    mixes: PathBuf,
    threads: i32,
    /// `None` after a failed reload; the next job loads again.
    tts: Option<Tts>,
    /// The voices file `tts` was loaded with: `voices` or a mix file. Every offered voice
    /// speaks the same from either, because a mix file changes only [`MIX_SPEAKER`].
    loaded: PathBuf,
    lexicon: Lexicon,
}

impl KokoroSession {
    /// The model with `voices` as its voices file, loaded again only when it was loaded with
    /// another one.
    fn load_voices(&mut self, voices: &Path) -> Result<&mut Tts, String> {
        if self.loaded != voices {
            // The model leaves memory before it comes back with the other file.
            self.tts = None;
        }
        if self.tts.is_none() {
            let files = KokoroFiles {
                model: &self.model,
                voices,
                tokens: &self.tokens,
            };
            self.tts = Some(Tts::kokoro(Arc::clone(&self.api), &files, self.threads)?);
            self.loaded = voices.to_path_buf();
        }
        self.tts
            .as_mut()
            .ok_or_else(|| "the model did not load".to_owned())
    }

    /// The speaker of `voice` and the voices file that holds it: `None` for whichever file is
    /// loaded.
    fn speaker(&self, voice: &str) -> Result<(i32, Option<PathBuf>), JobError> {
        if let Some(found) = find_voice(voice) {
            return Ok((found.speaker, None));
        }
        let mix = VoiceMix::parse(voice).ok_or_else(|| JobError::Failed {
            code: SPEECH_VOICE_MISSING,
            detail: voice.to_owned(),
        })?;
        let file =
            mix::ensure(&self.voices, &self.mixes, &mix).map_err(|detail| JobError::Failed {
                code: SPEECH_FAILED,
                detail,
            })?;
        Ok((MIX_SPEAKER, Some(file)))
    }
}

impl Session for KokoroSession {
    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn speak(
        &mut self,
        text: &str,
        voice: Option<&str>,
        speed: f32,
        cancel: &CancelFlag,
        sink: &mut dyn FnMut(&[f32]),
    ) -> Result<(), JobError> {
        let (speaker, file) = self.speaker(voice.unwrap_or(DEFAULT_VOICE))?;
        let sentences = self.lexicon.sentences(text);
        if sentences.is_empty() {
            return Ok(());
        }
        let file = file.unwrap_or_else(|| self.loaded.clone());
        let failed = |detail| JobError::Failed {
            code: SPEECH_FAILED,
            detail,
        };
        let result = self.load_voices(&file).map_err(failed)?.generate(
            &sentences,
            speaker,
            speed,
            &mut |samples| {
                if cancel.is_cancelled() {
                    return false;
                }
                sink(samples);
                true
            },
        );
        if cancel.is_cancelled() {
            return Err(JobError::Cancelled);
        }
        result.map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::TtsEngine;
    use std::time::Duration;

    #[test]
    fn a_folder_without_the_model_files_fails_before_the_library_opens() {
        let dir = std::env::temp_dir().join("hushpen-tts-no-model");
        let backend = SherpaBackend::new(PathBuf::from("/nonexistent/lib"));
        let spec = LoadSpec {
            dir,
            engine: TtsEngine::Kokoro,
            threads: 2,
            idle: Some(Duration::from_secs(1)),
        };
        let Err(problem) = backend.load(&spec) else {
            panic!("no model, no load");
        };
        assert!(problem.detail.contains(MODEL_FILE), "{}", problem.detail);
    }
}
