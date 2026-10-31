//! Parakeet TDT through the Parakeet library in the vendored whisper.cpp tree.
//!
//! Parakeet always detects its language among its 25. It takes no prompt and has no translate
//! task. Its C segment spans the whole input, so the transcript is made of its timed words.
//!
//! parakeet.cpp checks the abort callback between decoder steps and after the encoder, but its
//! scheduled encoder graph never gets the callback. One encoder run over a 120 s window takes
//! about 5 s on an M4 Pro CPU and cannot be stopped. So only the speech the detector finds is
//! decoded, in spans of at most [`MAX_SPAN`] that end at pauses: one span runs well under a
//! second on that CPU, which bounds a cancel.

use std::ffi::{CStr, CString, c_void};
use std::ops::Range;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

use whisper_rs::{WhisperVadContext, WhisperVadContextParams};
use whisper_rs_sys as ffi;

use crate::asr::{
    AsrEngine, CancelFlag, EngineError, LoadOptions, PcmSource, Segment, TranscribeOptions,
    Transcript,
};
use crate::windows::{self, SAMPLE_RATE, Windows};

/// The longest audio one native call decodes.
const MAX_SPAN: usize = 20 * SAMPLE_RATE;

/// Audio decoded on each side of a cut inside speech, so that a word on the cut is heard whole.
const CUT_CONTEXT: usize = SAMPLE_RATE;

/// Silero v5 gives one speech probability for each 512 samples at 16 kHz.
const VAD_FRAME: usize = 512;

/// A part of a window to decode, and the part whose words it keeps, both in window samples.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    decode: Range<usize>,
    keep: Range<usize>,
}

extern "C" fn abort_requested(user_data: *mut c_void) -> bool {
    // SAFETY: `user_data` is the `AtomicBool` of a `CancelFlag` that outlives the native call.
    unsafe { (*user_data.cast::<AtomicBool>()).load(Ordering::Relaxed) }
}

/// One Parakeet model held in memory, with the speech detector that picks what it decodes.
pub struct ParakeetEngine {
    context: NonNull<ffi::parakeet_context>,
    state: NonNull<ffi::parakeet_state>,
    threads: i32,
    gpu: bool,
    vad: WhisperVadContext,
    /// The mappings that hold the weights, for a page-out. Empty where none is possible.
    weights: Vec<Range<usize>>,
}

impl ParakeetEngine {
    pub fn load(model: &Path, options: LoadOptions) -> Result<Self, EngineError> {
        if let Some(feature) = crate::cpu::missing_cpu_feature() {
            return Err(EngineError::UnsupportedCpu(feature));
        }
        let path = model
            .to_str()
            .ok_or_else(|| EngineError::LoadFailed("model path is not valid UTF-8".into()))?;
        let path =
            CString::new(path).map_err(|error| EngineError::LoadFailed(error.to_string()))?;
        let threads = i32::try_from(options.threads.max(1)).unwrap_or(i32::MAX);
        let mut vad_params = WhisperVadContextParams::default();
        vad_params.set_n_threads(threads);
        let vad = WhisperVadContext::new(&crate::vad::model_path()?, vad_params)
            .map_err(|error| EngineError::LoadFailed(error.to_string()))?;
        // SAFETY: the library returns a fully initialized value.
        let mut params = unsafe { ffi::parakeet_context_default_params() };
        params.use_gpu = options.gpu;
        // The state holds compute buffers sized for the whole audio context, more than the
        // weights. It is made outside `track`, or the page-out finds too much and advises nothing.
        let (context, weights) = crate::pageout::track(model, || {
            // SAFETY: `path` lives through the load. The context returned has one owner.
            unsafe { ffi::parakeet_init_from_file_with_params_no_state(path.as_ptr(), params) }
        });
        let context = NonNull::new(context)
            .ok_or_else(|| EngineError::LoadFailed("could not load the Parakeet model".into()))?;
        // SAFETY: `context` is a loaded model; the state returned has one owner.
        let state = unsafe { ffi::parakeet_init_state(context.as_ptr()) };
        let Some(state) = NonNull::new(state) else {
            // SAFETY: the context has no state and no other owner.
            unsafe { ffi::parakeet_free(context.as_ptr()) };
            return Err(EngineError::LoadFailed(
                "could not set up the Parakeet model".into(),
            ));
        };
        Ok(Self {
            context,
            state,
            threads,
            gpu: options.gpu && crate::whisper::gpu_backend_present(),
            vad,
            weights,
        })
    }

    /// The words of one window, in window time. A window without speech is not decoded.
    fn run_window(
        &mut self,
        pcm: &[f32],
        cancel: &CancelFlag,
    ) -> Result<Vec<Segment>, EngineError> {
        let speech = self
            .vad
            .segments_from_samples(crate::vad::params(), pcm)
            .map_err(|error| EngineError::Failed(error.to_string()))?;
        let found: Vec<_> = (0..speech.num_segments())
            .filter_map(|index| speech.get_segment(index))
            .map(|segment| {
                (
                    centiseconds_to_samples(segment.start),
                    centiseconds_to_samples(segment.end),
                )
            })
            .collect();
        let spans = spans(&found, self.vad.probabilities(), pcm.len(), MAX_SPAN);
        let mut words = Vec::new();
        for span in spans {
            if cancel.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            let offset_ms = (span.decode.start * 1000 / SAMPLE_RATE) as u64;
            let found = self.decode(&pcm[span.decode.clone()], cancel)?;
            words.extend(
                found
                    .into_iter()
                    .map(|word| Segment {
                        start_ms: word.start_ms + offset_ms,
                        end_ms: word.end_ms + offset_ms,
                        text: word.text,
                    })
                    .filter(|word| {
                        let middle = (word.start_ms + word.end_ms) / 2;
                        span.keep.contains(&(middle as usize * SAMPLE_RATE / 1000))
                    }),
            );
        }
        Ok(words)
    }

    /// One native call over `pcm`. Returns its words in `pcm` time.
    fn decode(&mut self, pcm: &[f32], cancel: &CancelFlag) -> Result<Vec<Segment>, EngineError> {
        let context = self.context.as_ptr();
        let state = self.state.as_ptr();
        // SAFETY: the library initializes every field and leaves the callbacks null.
        let mut params = unsafe {
            ffi::parakeet_full_default_params(
                ffi::parakeet_sampling_strategy_PARAKEET_SAMPLING_GREEDY,
            )
        };
        params.n_threads = self.threads;
        params.no_context = true;
        params.abort_callback = Some(abort_requested);
        params.abort_callback_user_data =
            std::sync::Arc::as_ptr(cancel.as_atomic()).cast_mut().cast();
        let samples = i32::try_from(pcm.len())
            .map_err(|_| EngineError::Failed("audio span is too long".into()))?;
        // SAFETY: `self` holds the context and the state exclusively; `pcm` and `cancel` outlive
        // the call.
        let code =
            unsafe { ffi::parakeet_full_with_state(context, state, params, pcm.as_ptr(), samples) };
        // An abort returns -6 after the encoder or -7 in the decoder. The flag decides.
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        if code != 0 {
            return Err(EngineError::Failed(format!("Parakeet returned {code}")));
        }
        let mut words = Vec::new();
        // SAFETY: the call succeeded, so the state holds its result; every index is in range.
        unsafe {
            for segment in 0..ffi::parakeet_full_n_segments_from_state(state) {
                for token in 0..ffi::parakeet_full_n_tokens_from_state(state, segment) {
                    let data = ffi::parakeet_full_get_token_data_from_state(state, segment, token);
                    let text = ffi::parakeet_full_get_token_text_from_state(
                        context, state, segment, token,
                    );
                    if text.is_null() {
                        return Err(EngineError::Failed("Parakeet returned a null token".into()));
                    }
                    let text = CStr::from_ptr(text).to_string_lossy();
                    append_piece(&mut words, &text, data.is_word_start, data.t0, data.t1);
                }
            }
        }
        Ok(words)
    }
}

impl Drop for ParakeetEngine {
    fn drop(&mut self) {
        // SAFETY: this value owns the state and the context, and no native call can still use
        // them. The context has no state of its own to free.
        unsafe {
            ffi::parakeet_free_state(self.state.as_ptr());
            ffi::parakeet_free(self.context.as_ptr());
        }
    }
}

impl AsrEngine for ParakeetEngine {
    fn gpu_in_use(&self) -> bool {
        self.gpu
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
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        if let Some(code) = options.language.as_deref()
            && code != "auto"
            && !hushpen_core::catalog::PARAKEET_LANGUAGES.contains(&code)
        {
            return Err(EngineError::BadLanguage(code.to_owned()));
        }
        if options.translate {
            return Err(EngineError::TranslateUnsupported);
        }
        let mut input = Windows::new(source);
        let mut segments = Vec::new();
        while let Some((window, pcm)) = input.next(cancel)? {
            let words = self.run_window(pcm, cancel)?;
            segments.extend(windows::own_segments(&window, words));
            let last = window.keep_to == usize::MAX;
            progress(if last { 1.0 } else { input.fraction_read() });
        }
        let segments = windows::clip_to_audio(segments, input.audio_len());
        let text = segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<String>()
            .trim()
            .to_owned();
        Ok(Transcript {
            text,
            // The model detects among its languages, but the C API does not say which it heard.
            language: "und".into(),
            segments,
        })
    }
}

fn centiseconds_to_samples(value: f32) -> usize {
    (value.max(0.0) * (SAMPLE_RATE / 100) as f32) as usize
}

/// The parts of a window of `len` samples to decode, each at most `max` samples long: the speech
/// `found`, in time order, joined while a part stays within `max`. The silence between parts is
/// not decoded. Speech that runs on longer is cut at its quietest frame in `probs`, and each side
/// of a cut also decodes [`CUT_CONTEXT`] past it but keeps only the words on its own side.
fn spans(found: &[(usize, usize)], probs: &[f32], len: usize, max: usize) -> Vec<Span> {
    let mut joined: Vec<Range<usize>> = Vec::new();
    for &(start, end) in found {
        let (start, end) = (start.min(len), end.min(len));
        if start >= end {
            continue;
        }
        match joined.last_mut() {
            Some(last) if end.saturating_sub(last.start) <= max => last.end = last.end.max(end),
            _ => joined.push(start..end),
        }
    }
    let room = max - 2 * CUT_CONTEXT;
    let mut spans = Vec::new();
    for part in joined {
        let mut start = part.start;
        loop {
            let from = if start == part.start {
                start
            } else {
                start - CUT_CONTEXT
            };
            let keep_from = if start == part.start { 0 } else { start };
            if part.end - from <= max {
                spans.push(Span {
                    decode: from..part.end,
                    keep: keep_from..usize::MAX,
                });
                break;
            }
            let cut = quietest(probs, start + room / 2..start + room);
            spans.push(Span {
                decode: from..cut + CUT_CONTEXT,
                keep: keep_from..cut,
            });
            start = cut;
        }
    }
    spans
}

/// The sample in `range` at the middle of the frame least likely to hold speech, or the end of
/// `range` when `probs` does not cover it.
fn quietest(probs: &[f32], range: Range<usize>) -> usize {
    (range.start / VAD_FRAME..range.end / VAD_FRAME)
        .filter_map(|frame| probs.get(frame).map(|prob| (frame, *prob)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(range.end, |(frame, _)| {
            (frame * VAD_FRAME + VAD_FRAME / 2).clamp(range.start, range.end)
        })
}

/// Adds one timed subword. Times are in centiseconds. A piece that starts no word joins the
/// word before it, and every word starts with a space, so the words join into the text.
fn append_piece(words: &mut Vec<Segment>, piece: &str, word_start: bool, t0: i64, t1: i64) {
    if matches!(piece, "<unk>" | "<s>" | "</s>" | "[BLANK]") {
        return;
    }
    let text = piece.replace('\u{2581}', " ");
    let start_ms = u64::try_from(t0).unwrap_or(0).saturating_mul(10);
    let end_ms = u64::try_from(t1).unwrap_or(0).saturating_mul(10);
    match words.last_mut() {
        Some(word) if !word_start => {
            word.text.push_str(&text);
            word.end_ms = word.end_ms.max(end_ms);
        }
        _ => words.push(Segment {
            start_ms,
            end_ms: end_ms.max(start_ms.saturating_add(10)),
            text: if text.starts_with(' ') {
                text
            } else {
                format!(" {text}")
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(start_ms: u64, end_ms: u64, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    #[test]
    fn timed_pieces_form_words_and_attach_punctuation() {
        let mut words = Vec::new();
        append_piece(&mut words, "\u{2581}Hello", true, 8, 16);
        append_piece(&mut words, ",", false, 16, 16);
        append_piece(&mut words, "\u{2581}trans", true, 24, 32);
        append_piece(&mut words, "cription", false, 32, 40);
        append_piece(&mut words, ".", false, 40, 40);
        append_piece(&mut words, "<unk>", true, 40, 48);
        assert_eq!(
            words,
            [word(80, 160, " Hello,"), word(240, 400, " transcription.")]
        );
    }

    #[test]
    fn every_word_has_a_valid_span_and_a_leading_space() {
        let mut words = Vec::new();
        append_piece(&mut words, "A", false, 0, 0);
        append_piece(&mut words, "B", true, 5, 3);
        assert_eq!(words, [word(0, 10, " A"), word(50, 60, " B")]);
    }

    fn whole(decode: Range<usize>) -> Span {
        Span {
            decode,
            keep: 0..usize::MAX,
        }
    }

    #[test]
    fn speech_is_joined_into_spans_that_stay_within_the_limit() {
        let second = SAMPLE_RATE;
        let found = [
            (0, 3 * second),
            (5 * second, 12 * second),
            (13 * second, 21 * second),
            (30 * second, 31 * second),
        ];
        assert_eq!(
            spans(&found, &[], 60 * second, MAX_SPAN),
            [whole(0..12 * second), whole(13 * second..31 * second)]
        );
    }

    #[test]
    fn speech_without_a_long_pause_is_cut_at_its_quietest_frames() {
        let second = SAMPLE_RATE;
        let end = 50 * second;
        let mut probs = vec![0.9; end / VAD_FRAME];
        probs[15 * second / VAD_FRAME] = 0.1;
        probs[32 * second / VAD_FRAME] = 0.2;
        let a = 15 * second / VAD_FRAME * VAD_FRAME + VAD_FRAME / 2;
        let b = 32 * second / VAD_FRAME * VAD_FRAME + VAD_FRAME / 2;
        let kept = spans(&[(0, end)], &probs, end, MAX_SPAN);
        let context = CUT_CONTEXT;
        assert_eq!(
            kept,
            [
                Span {
                    decode: 0..a + context,
                    keep: 0..a,
                },
                Span {
                    decode: a - context..b + context,
                    keep: a..b,
                },
                Span {
                    decode: b - context..end,
                    keep: b..usize::MAX,
                },
            ]
        );
        let blind = spans(&[(0, end)], &[], end, MAX_SPAN);
        assert_eq!(blind.len(), 3);
        for span in kept.iter().chain(&blind) {
            assert!(span.decode.len() <= MAX_SPAN, "{span:?}");
        }
        for pair in blind.windows(2) {
            assert_eq!(pair[0].keep.end, pair[1].keep.start);
        }
    }

    #[test]
    fn spans_stay_inside_the_window_and_silence_has_none() {
        let len = 4 * SAMPLE_RATE;
        assert!(spans(&[], &[], len, MAX_SPAN).is_empty());
        let found = [(len, len + 10), (2, 1), (len - 100, len + 500)];
        assert_eq!(spans(&found, &[], len, MAX_SPAN), [whole(len - 100..len)]);
    }

    #[test]
    fn detector_times_are_centiseconds() {
        assert_eq!(centiseconds_to_samples(150.0), 24_000);
        assert_eq!(centiseconds_to_samples(-1.0), 0);
    }
}
