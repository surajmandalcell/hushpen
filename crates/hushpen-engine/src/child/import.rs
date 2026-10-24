//! One imported file, decoded while it is transcribed in 120 s windows with 10 s of overlap.
//!
//! The windows are the ones every transcription uses (`windows.rs`, applied inside
//! `AsrEngine::transcribe_stream`), so the words at a window edge are kept once and only about
//! one window of samples is held. A decoder that panics on a hostile file costs this job and not
//! the child.

use crate::asr::{AsrEngine, CancelFlag, EngineError, PcmSource, TranscribeOptions};
use hushpen_audio::decode::{DecodeError, FileDecoder};
use hushpen_audio::resample::TARGET_RATE;
use hushpen_core::error;
use hushpen_core::protocol::Event;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::time::Instant;

/// Runs one import. Every `Progress` and `Segment` event goes to `emit`; the event that ends the
/// job is returned.
pub(crate) fn run(
    engine: Option<&mut (dyn AsrEngine + 'static)>,
    job: u64,
    path: &Path,
    options: &TranscribeOptions,
    cancel: &CancelFlag,
    emit: &mut dyn FnMut(Event),
) -> Event {
    let Some(engine) = engine else {
        return Event::error(Some(job), error::ENGINE_NO_MODEL, "no model is loaded");
    };
    let mut source = match guarded(|| FileDecoder::open(path)) {
        Ok(decoder) => FileSource {
            decoder,
            samples: 0,
        },
        Err(failure) => return failed(job, &decode_error(failure)),
    };
    emit(progress(job, 0.0));
    let started = Instant::now();
    let transcribed = engine.transcribe_stream(&mut source, options, cancel, &mut |fraction| {
        emit(progress(job, f64::from(fraction)));
    });
    match transcribed {
        Ok(transcript) => {
            for segment in &transcript.segments {
                emit(Event::Segment {
                    job,
                    segment: segment.clone(),
                });
            }
            Event::Result {
                job,
                text: transcript.text,
                language: transcript.language,
                segments: transcript.segments,
                audio_ms: source.samples * 1000 / u64::from(TARGET_RATE),
                ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }
        }
        Err(EngineError::Cancelled) => Event::Cancelled { job },
        Err(failure) => failed(job, &failure),
    }
}

/// The decoder of the file as the engine reads it, counting the samples it gave.
struct FileSource {
    decoder: FileDecoder,
    samples: u64,
}

impl PcmSource for FileSource {
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, EngineError> {
        let before = out.len();
        let more = guarded(|| self.decoder.read(out)).map_err(decode_error)?;
        self.samples += (out.len() - before) as u64;
        Ok(more)
    }

    fn fraction_read(&self) -> f32 {
        self.decoder.fraction_read()
    }
}

/// Runs one decoder step and turns a panic into a decode failure.
fn guarded<T>(step: impl FnOnce() -> Result<T, DecodeError>) -> Result<T, DecodeError> {
    panic::catch_unwind(AssertUnwindSafe(step)).unwrap_or_else(|_| {
        Err(DecodeError::Failed(
            "The file could not be decoded.".to_owned(),
        ))
    })
}

fn decode_error(failure: DecodeError) -> EngineError {
    match failure {
        DecodeError::Unsupported(text) => EngineError::Decode {
            code: error::IMPORT_UNSUPPORTED_CODEC,
            text,
        },
        DecodeError::Failed(text) => EngineError::Decode {
            code: error::IMPORT_DECODE_FAILED,
            text,
        },
    }
}

fn failed(job: u64, failure: &EngineError) -> Event {
    Event::error(
        Some(job),
        failure.code().unwrap_or(error::ENGINE_CRASHED),
        failure.to_string(),
    )
}

fn progress(job: u64, fraction: f64) -> Event {
    Event::Progress {
        job,
        fraction: fraction.clamp(0.0, 1.0),
    }
}
