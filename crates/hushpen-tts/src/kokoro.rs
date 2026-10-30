//! The Kokoro model on sherpa-onnx: the worker's real backend.

use crate::backend::{Backend, CancelFlag, JobError, LoadError, LoadSpec, Session};
use crate::phonemes::Lexicon;
use crate::sherpa::{Api, KokoroFiles, Tts};
use hushpen_core::error::{SPEECH_FAILED, SPEECH_VOICE_MISSING};
use hushpen_core::kokoro::{
    DEFAULT_VOICE, FILES, LEXICON_FILE, MODEL_FILE, SAMPLE_RATE, TOKENS_FILE, VOICES, VOICES_FILE,
    find_voice,
};
use std::path::PathBuf;
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
        let (model, voices, tokens, lexicon) = (
            spec.dir.join(MODEL_FILE),
            spec.dir.join(VOICES_FILE),
            spec.dir.join(TOKENS_FILE),
            spec.dir.join(LEXICON_FILE),
        );
        let lexicon = Lexicon::read(&lexicon).map_err(failed)?;
        let files = KokoroFiles {
            model: &model,
            voices: &voices,
            tokens: &tokens,
        };
        let threads = i32::try_from(spec.threads).unwrap_or(i32::MAX);
        let tts = Tts::kokoro(api, &files, threads).map_err(failed)?;
        let speakers = tts.speakers();
        if usize::try_from(speakers).unwrap_or(0) < VOICES.len() || tts.sample_rate() != SAMPLE_RATE
        {
            return Err(failed(format!(
                "not the Kokoro model: {speakers} speakers at {} Hz",
                tts.sample_rate()
            )));
        }
        Ok(Box::new(KokoroSession { tts, lexicon }))
    }
}

struct KokoroSession {
    tts: Tts,
    lexicon: Lexicon,
}

impl Session for KokoroSession {
    fn sample_rate(&self) -> u32 {
        self.tts.sample_rate()
    }

    fn speak(
        &mut self,
        text: &str,
        voice: Option<&str>,
        speed: f32,
        cancel: &CancelFlag,
        sink: &mut dyn FnMut(&[f32]),
    ) -> Result<(), JobError> {
        let id = voice.unwrap_or(DEFAULT_VOICE);
        let speaker = find_voice(id)
            .ok_or_else(|| JobError::Failed {
                code: SPEECH_VOICE_MISSING,
                detail: id.to_owned(),
            })?
            .speaker;
        let sentences = self.lexicon.sentences(text);
        if sentences.is_empty() {
            return Ok(());
        }
        let result = self
            .tts
            .generate(&sentences, speaker, speed, &mut |samples| {
                if cancel.is_cancelled() {
                    return false;
                }
                sink(samples);
                true
            });
        if cancel.is_cancelled() {
            return Err(JobError::Cancelled);
        }
        result.map_err(|detail| JobError::Failed {
            code: SPEECH_FAILED,
            detail,
        })
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
