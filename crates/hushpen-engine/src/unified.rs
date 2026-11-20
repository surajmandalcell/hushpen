//! Nemotron Labs Unified ASR through parakeet.cpp and Hushpen's ggml backend.

use crate::asr::{
    AsrEngine, CancelFlag, EngineError, LoadOptions, PcmSource, Segment, TranscribeOptions,
    Transcript,
};
use crate::windows::{self, Windows};
use std::ffi::{CStr, CString, c_char};
use std::ops::Range;
use std::path::Path;
use std::ptr::NonNull;
use whisper_rs_sys as ffi;

/// Windows up to 40 s keep the full attention the model scores best with. Past that, words
/// near the end of a window can be lost, such as the last 2.5 s of a 1 minute file.
const WINDOW: usize = 40 * windows::SAMPLE_RATE;
/// Each side keeps 3 s past the handover, so a word cut at a window edge is whole in one.
const OVERLAP: usize = 6 * windows::SAMPLE_RATE;

pub(crate) struct Unified {
    model: NonNull<ffi::hushpen_unified_model>,
    weights: Vec<Range<usize>>,
}

impl Unified {
    pub fn load(path: &Path, options: LoadOptions) -> Result<Self, EngineError> {
        if let Some(feature) = crate::cpu::missing_cpu_feature() {
            return Err(EngineError::UnsupportedCpu(feature));
        }
        let name = path
            .to_str()
            .and_then(|s| CString::new(s).ok())
            .ok_or_else(|| EngineError::LoadFailed("invalid model path".into()))?;
        let threads = i32::try_from(options.threads.max(1)).unwrap_or(i32::MAX);
        // SAFETY: the path lives through load. Only this struct owns the returned model.
        let (ptr, weights) = crate::pageout::track(path, || unsafe {
            ffi::hushpen_unified_load(name.as_ptr(), options.gpu, threads)
        });
        let model = NonNull::new(ptr).ok_or_else(|| EngineError::LoadFailed(last_error()))?;
        Ok(Self { model, weights })
    }
}

impl Drop for Unified {
    fn drop(&mut self) {
        // SAFETY: all calls and streaming sessions finish before this owner is dropped.
        unsafe { ffi::hushpen_unified_free(self.model.as_ptr()) };
    }
}

struct Stream(NonNull<ffi::hushpen_unified_stream>);

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: the stream belongs to the live call, whose model still exists.
        unsafe { ffi::hushpen_unified_stream_free(self.0.as_ptr()) }
    }
}

fn last_error() -> String {
    // SAFETY: the runtime owns this thread's nul-terminated error until the next native call.
    let ptr = unsafe { ffi::hushpen_unified_error() };
    if ptr.is_null() {
        return "Nemotron Unified returned no error detail".into();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn language(options: &TranscribeOptions) -> Result<CString, EngineError> {
    if options.translate {
        return Err(EngineError::TranslateUnsupported);
    }
    let code = options.language.as_deref().unwrap_or("auto");
    if code != "auto" && !hushpen_core::catalog::PARAKEET_LANGUAGES.contains(&code) {
        return Err(EngineError::BadLanguage(code.into()));
    }
    CString::new(code).map_err(|_| EngineError::BadLanguage(code.into()))
}

// Copies the runtime's borrowed result before another call can replace it.
unsafe fn native_text(ptr: *const c_char) -> Result<String, EngineError> {
    if ptr.is_null() {
        return Err(EngineError::Failed(last_error()));
    }
    // SAFETY: callers pass a nul-terminated result from the live model or stream.
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map(str::to_owned)
        .map_err(|error| EngineError::Failed(error.to_string()))
}

fn transcript(json: &str, options: &TranscribeOptions) -> Result<Transcript, EngineError> {
    let data: serde_json::Value =
        serde_json::from_str(json).map_err(|e| EngineError::Failed(e.to_string()))?;
    let malformed = || EngineError::Failed("invalid transcript from Nemotron Unified".into());
    let text = data["text"].as_str().ok_or_else(malformed)?.to_owned();
    let mut segments = Vec::new();
    for word in data["words"].as_array().ok_or_else(malformed)? {
        let start = word["start"].as_f64().ok_or_else(malformed)?;
        let end = word["end"].as_f64().ok_or_else(malformed)?;
        let word = word["word"].as_str().ok_or_else(malformed)?;
        segments.push(Segment {
            start_ms: (start * 1000.0).round() as u64,
            end_ms: (end * 1000.0).round() as u64,
            text: format!(" {}", word.trim()),
        });
    }
    let language = options
        .language
        .as_deref()
        .filter(|s| *s != "auto")
        .unwrap_or("und")
        .to_owned();
    Ok(Transcript {
        text,
        segments,
        language,
    })
}

impl AsrEngine for Unified {
    fn transcribe_live(
        &mut self,
        source: &mut dyn PcmSource,
        options: &TranscribeOptions,
        cancel: &CancelFlag,
        partial: &mut dyn FnMut(&str),
    ) -> Result<Transcript, EngineError> {
        let lang = language(options)?;
        // SAFETY: the exclusive model outlives the stream and the native call copies its language.
        let stream = unsafe { ffi::hushpen_unified_begin(self.model.as_ptr(), lang.as_ptr()) };
        let stream = Stream(NonNull::new(stream).ok_or_else(|| EngineError::Failed(last_error()))?);
        let mut pcm = Vec::new();
        let mut text = String::new();
        let mut heard_audio = false;
        loop {
            if cancel.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            pcm.clear();
            let more = source.read(&mut pcm)?;
            for chunk in pcm.chunks(4096) {
                if cancel.is_cancelled() {
                    return Err(EngineError::Cancelled);
                }
                heard_audio = true;
                // SAFETY: the stream and bounded sample slice live through the call.
                let delta = unsafe {
                    native_text(ffi::hushpen_unified_feed(
                        stream.0.as_ptr(),
                        chunk.as_ptr(),
                        chunk.len() as i32,
                    ))?
                };
                if !delta.is_empty() {
                    text.push_str(&delta);
                    partial(&text);
                }
            }
            if !more {
                break;
            }
        }
        if !heard_audio {
            return Err(EngineError::NoAudio);
        }
        // SAFETY: the stream is live and this is its only final flush.
        let json = unsafe { native_text(ffi::hushpen_unified_finish(stream.0.as_ptr()))? };
        transcript(&json, options)
    }

    fn gpu_in_use(&self) -> bool {
        // SAFETY: the handle is live for the duration of this call.
        unsafe { ffi::hushpen_unified_gpu(self.model.as_ptr()) }
    }

    fn page_out(&mut self) -> std::io::Result<u64> {
        crate::pageout::page_out(&self.weights)
    }

    fn transcribe_stream(
        &mut self,
        source: &mut dyn PcmSource,
        options: &TranscribeOptions,
        cancel: &CancelFlag,
        progress: &mut dyn FnMut(f32),
    ) -> Result<Transcript, EngineError> {
        let lang = language(options)?;
        let mut input = Windows::sized(source, WINDOW, OVERLAP);
        let mut segments = Vec::new();
        let mut resume = false;
        while let Some((window, pcm)) = input.next(cancel)? {
            // Save the decoder state where the next window starts: a cold decoder that starts
            // mid-sentence can write nothing for a whole window.
            let save_at = if window.keep_to == usize::MAX {
                -1
            } else {
                (pcm.len() - OVERLAP) as i32
            };
            // SAFETY: the bounded window, language, and exclusive model are live throughout.
            let json = unsafe {
                native_text(ffi::hushpen_unified_batch(
                    self.model.as_ptr(),
                    pcm.as_ptr(),
                    pcm.len() as i32,
                    lang.as_ptr(),
                    resume,
                    save_at,
                ))?
            };
            resume = save_at >= 0;
            if cancel.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            segments.extend(windows::own_segments(
                &window,
                transcript(&json, options)?.segments,
            ));
            progress(input.fraction_read());
        }
        let segments = windows::clip_to_audio(segments, input.audio_len());
        let text = segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>()
            .trim()
            .to_owned();
        Ok(Transcript {
            text,
            language: options
                .language
                .clone()
                .filter(|s| s != "auto")
                .unwrap_or_else(|| "und".into()),
            segments,
        })
    }
}
