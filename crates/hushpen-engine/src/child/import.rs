//! One imported file: decode it, then transcribe it in 120 s windows with 10 s of overlap.
//!
//! The windows are the ones every transcription uses (`windows.rs`, applied inside
//! `AsrEngine::transcribe_with_progress`), so the words at a window edge are kept once. A decoder
//! that panics on a hostile file costs this job and not the child.

use crate::asr::{AsrEngine, CancelFlag, EngineError, TranscribeOptions};
use hushpen_audio::decode::{self, DecodeError};
use hushpen_audio::resample::TARGET_RATE;
use hushpen_core::error;
use hushpen_core::protocol::Event;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::time::Instant;

/// How much of the progress bar the decode takes. The rest belongs to the windows.
const DECODE_SHARE: f64 = 0.05;

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
    let decoded = panic::catch_unwind(AssertUnwindSafe(|| {
        decode::decode_file(path, &|| cancel.is_cancelled(), &mut |fraction| {
            emit(progress(job, f64::from(fraction) * DECODE_SHARE));
        })
    }))
    .unwrap_or_else(|_| {
        Err(DecodeError::Failed(
            "The file could not be decoded.".to_owned(),
        ))
    });
    let pcm = match decoded {
        Ok(pcm) => pcm,
        Err(DecodeError::Cancelled) => return Event::Cancelled { job },
        Err(DecodeError::Unsupported(text)) => {
            return Event::error(Some(job), error::IMPORT_UNSUPPORTED_CODEC, text);
        }
        Err(DecodeError::Failed(text)) => {
            return Event::error(Some(job), error::IMPORT_DECODE_FAILED, text);
        }
    };
    let audio_ms = pcm.len() as u64 * 1000 / u64::from(TARGET_RATE);
    let started = Instant::now();
    let transcribed = engine.transcribe_with_progress(&pcm, options, cancel, &mut |fraction| {
        emit(progress(
            job,
            DECODE_SHARE + (1.0 - DECODE_SHARE) * f64::from(fraction),
        ));
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
                audio_ms,
                ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }
        }
        Err(EngineError::Cancelled) => Event::Cancelled { job },
        Err(failure) => Event::error(
            Some(job),
            failure.code().unwrap_or(error::ENGINE_CRASHED),
            failure.to_string(),
        ),
    }
}

fn progress(job: u64, fraction: f64) -> Event {
    Event::Progress {
        job,
        fraction: fraction.clamp(0.0, 1.0),
    }
}
