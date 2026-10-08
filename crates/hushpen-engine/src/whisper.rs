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
use crate::windows;

extern "C" fn abort_requested(user_data: *mut c_void) -> bool {
    // SAFETY: `user_data` is the `AtomicBool` of a `CancelFlag` that outlives the `full()` call.
    unsafe { (*(user_data as *const AtomicBool)).load(Ordering::Relaxed) }
}

extern "C" fn native_log(level: u32, text: *const std::ffi::c_char, _user_data: *mut c_void) {
    // Debug builds of whisper.cpp print every decoded token at DEBUG level, which is transcript
    // text. Only warnings and errors may reach stderr, and so engine.log.
    if text.is_null()
        || !(level == whisper_rs_sys::ggml_log_level_GGML_LOG_LEVEL_WARN
            || level == whisper_rs_sys::ggml_log_level_GGML_LOG_LEVEL_ERROR)
    {
        return;
    }
    // SAFETY: whisper.cpp passes a NUL-terminated string that lives for this call.
    let text = unsafe { std::ffi::CStr::from_ptr(text) };
    eprint!("{}", text.to_string_lossy());
}

/// Routes whisper.cpp and ggml logging through a filter that drops everything below a warning.
/// Call it once in the engine child before any model loads.
pub fn quiet_native_logs() {
    // SAFETY: `native_log` is a plain function that ignores its user data.
    unsafe {
        whisper_rs_sys::whisper_log_set(Some(native_log), std::ptr::null_mut());
        whisper_rs_sys::ggml_log_set(Some(native_log), std::ptr::null_mut());
    }
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
        if let Some(feature) = crate::cpu::missing_cpu_feature() {
            return Err(EngineError::UnsupportedCpu(feature));
        }
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

        let mut segments = Vec::new();
        let mut detected = language.map(str::to_owned);
        let padded = windows::with_tail_pad(pcm);
        for window in windows::plan(padded.len()) {
            if cancel.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            let (found, language_used) = self.run_window(
                &padded[window.start..window.end],
                detected.as_deref(),
                options.prompt.as_deref(),
                cancel,
            )?;
            // Later windows reuse the language of the first one instead of detecting again.
            detected.get_or_insert(language_used);
            segments.extend(windows::own_segments(&window, found));
        }
        let segments = windows::clip_to_audio(segments, pcm.len());
        let text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>()
            .trim()
            .to_owned();
        Ok(Transcript {
            text,
            language: detected.unwrap_or_else(|| "und".to_owned()),
            segments,
        })
    }
}

impl WhisperEngine {
    /// One whisper call. Returns the segments in window time and the language used.
    fn run_window(
        &mut self,
        pcm: &[f32],
        language: Option<&str>,
        prompt: Option<&str>,
        cancel: &CancelFlag,
    ) -> Result<(Vec<Segment>, String), EngineError> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(i32::try_from(self.threads).unwrap_or(i32::MAX));
        params.set_language(language);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        if let Some(prompt) = prompt.filter(|p| !p.is_empty()) {
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
        let used = match language {
            Some(code) => code.to_owned(),
            None => get_lang_str(self.state.full_lang_id_from_state())
                .unwrap_or("und")
                .to_owned(),
        };
        Ok((segments, used))
    }
}

fn centiseconds_to_ms(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0).saturating_mul(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_language_picker_list_is_the_list_of_the_linked_whisper() {
        let linked = usize::try_from(whisper_rs::get_lang_max_id() + 1).unwrap();
        assert_eq!(hushpen_core::language::LANGUAGES.len(), linked);
        for (code, _) in hushpen_core::language::LANGUAGES {
            let id = get_lang_id(code).unwrap_or_else(|| panic!("whisper lacks {code}"));
            assert_eq!(get_lang_str(id), Some(code));
        }
    }
}
