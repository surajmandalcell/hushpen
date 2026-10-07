//! The whisper.cpp implementation of [`AsrEngine`].
//!
//! Cancel uses the raw abort callback. `FullParams::set_abort_callback_safe` in whisper-rs
//! 0.16.0 casts its boxed closure to the wrong type, so it reads garbage and `full()` aborts
//! at once. The flag below is an `AtomicBool` that the C callback reads directly.

use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
    get_lang_id, get_lang_str,
};

use crate::asr::{
    AsrEngine, CancelFlag, EngineError, LoadOptions, Segment, TranscribeOptions, Transcript,
};

extern "C" fn abort_requested(user_data: *mut c_void) -> bool {
    // SAFETY: `user_data` is the `AtomicBool` of a `CancelFlag` that outlives the `full()` call.
    unsafe { (*(user_data as *const AtomicBool)).load(Ordering::Relaxed) }
}

/// The whisper.cpp version string the fork was built from, for example `1.9.4`.
pub fn whisper_cpp_version() -> &'static str {
    whisper_rs::get_whisper_version()
}

fn gpu_backend_present() -> bool {
    // SAFETY: a pure lookup in the ggml backend registry.
    unsafe {
        !whisper_rs_sys::ggml_backend_dev_by_type(
            whisper_rs_sys::ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU,
        )
        .is_null()
    }
}

/// One model held in memory. Keep it alive between jobs so dictation stays warm.
pub struct WhisperEngine {
    // The state keeps its own `Arc` of the context in whisper-rs; `_context` only names the owner.
    state: WhisperState,
    _context: WhisperContext,
    threads: usize,
    gpu: bool,
    multilingual: bool,
}

impl WhisperEngine {
    pub fn load(model: &Path, options: LoadOptions) -> Result<Self, EngineError> {
        let path = model
            .to_str()
            .ok_or_else(|| EngineError::LoadFailed("model path is not valid UTF-8".into()))?;
        let mut params = WhisperContextParameters::default();
        params.use_gpu(options.gpu);
        let context = WhisperContext::new_with_params(path, params)
            .map_err(|err| EngineError::LoadFailed(err.to_string()))?;
        let state = context
            .create_state()
            .map_err(|err| EngineError::LoadFailed(err.to_string()))?;
        Ok(Self {
            state,
            multilingual: context.is_multilingual(),
            _context: context,
            threads: options.threads.max(1),
            gpu: options.gpu && gpu_backend_present(),
        })
    }
}

impl AsrEngine for WhisperEngine {
    fn gpu_in_use(&self) -> bool {
        self.gpu
    }

    fn transcribe(
        &mut self,
        pcm: &[f32],
        options: &TranscribeOptions,
        cancel: &CancelFlag,
    ) -> Result<Transcript, EngineError> {
        if pcm.is_empty() {
            return Err(EngineError::NoAudio);
        }
        // whisper checks the abort callback only between graph nodes, after the mel spectrogram
        // and the first encoder convolution. That is up to a second on a slow CPU, so a flag
        // that is already set must not reach it.
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        let language = options.language.as_deref().filter(|code| *code != "auto");
        if let Some(code) = language
            && get_lang_id(code).is_none()
        {
            return Err(EngineError::BadLanguage(code.to_owned()));
        }

        // An English-only model always decodes English and reports no usable language id.
        let language = if self.multilingual {
            language
        } else {
            Some("en")
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(i32::try_from(self.threads).unwrap_or(i32::MAX));
        params.set_language(language);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        if let Some(prompt) = options.prompt.as_deref().filter(|p| !p.is_empty()) {
            params.set_initial_prompt(prompt);
        }
        // SAFETY: the pointer is the flag inside `cancel`, which this call borrows until
        // `full()` returns; whisper reads it only during `full()`.
        unsafe {
            params.set_abort_callback(Some(abort_requested));
            params.set_abort_callback_user_data(
                std::sync::Arc::as_ptr(cancel.as_atomic()) as *mut c_void
            );
        }

        let outcome = self.state.full(params, pcm);
        // An abort surfaces as GenericError(-6) in the encoder or (-9) in the decoder. The
        // flag is the proof that we asked for it, so it decides, whatever code came back.
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        outcome.map_err(|err| EngineError::Failed(err.to_string()))?;

        let mut segments = Vec::new();
        for segment in self.state.as_iter() {
            let text = segment
                .to_str_lossy()
                .map_err(|err| EngineError::Failed(err.to_string()))?
                .into_owned();
            segments.push(Segment {
                start_ms: centiseconds_to_ms(segment.start_timestamp()),
                end_ms: centiseconds_to_ms(segment.end_timestamp()),
                text,
            });
        }
        let text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>()
            .trim()
            .to_owned();
        let detected = match language {
            Some(code) => code.to_owned(),
            None => get_lang_str(self.state.full_lang_id_from_state())
                .unwrap_or("und")
                .to_owned(),
        };
        Ok(Transcript {
            text,
            language: detected,
            segments,
        })
    }
}

fn centiseconds_to_ms(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0).saturating_mul(10)
}
