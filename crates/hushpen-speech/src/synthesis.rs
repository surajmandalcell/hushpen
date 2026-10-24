//! Streams speech from an engine on a thread of its own, one chunk of sentences at a time.

use crate::text::chunks;
use crate::{MAX_SPEED, MIN_SPEED, Pcm, SAMPLE_RATE, SpeechEngine, SpeechError};
use hushpen_audio::resample::MonoResampler;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Debug, Clone, PartialEq)]
pub enum SpeechEvent {
    /// The audio of the next chunk at `SAMPLE_RATE`, after which `done` of `total` chunks are
    /// spoken.
    Audio {
        samples: Vec<f32>,
        done: usize,
        total: usize,
    },
    Done,
    Cancelled,
    Failed(SpeechError),
}

/// A running synthesis. Every run ends with one `Done`, `Cancelled`, or `Failed` event.
/// Dropping it cancels the run.
pub struct Synthesis {
    events: Receiver<SpeechEvent>,
    cancelled: Arc<AtomicBool>,
}

impl Synthesis {
    pub fn events(&self) -> &Receiver<SpeechEvent> {
        &self.events
    }

    /// Stops after the chunk that is being spoken.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Drop for Synthesis {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Speaks prepared text (see `text::prepare`) with `engine`, off the calling thread.
pub fn synthesize(
    engine: Arc<dyn SpeechEngine>,
    text: String,
    voice: Option<String>,
    speed: f32,
) -> Synthesis {
    let (sender, events) = mpsc::channel();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancelled);
    let failure = sender.clone();
    let spawned = thread::Builder::new()
        .name("hushpen-speech".into())
        .spawn(move || run(&*engine, &text, voice.as_deref(), speed, &flag, &sender));
    if let Err(error) = spawned {
        let error = SpeechError::failed(format!("the speech thread did not start: {error}"));
        let _ = failure.send(SpeechEvent::Failed(error));
    }
    Synthesis { events, cancelled }
}

fn run(
    engine: &dyn SpeechEngine,
    text: &str,
    voice: Option<&str>,
    speed: f32,
    cancelled: &AtomicBool,
    events: &Sender<SpeechEvent>,
) {
    let speed = if speed.is_finite() {
        speed.clamp(MIN_SPEED, MAX_SPEED)
    } else {
        1.0
    };
    let chunks = chunks(text);
    let total = chunks.len();
    for (index, chunk) in chunks.into_iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            let _ = events.send(SpeechEvent::Cancelled);
            return;
        }
        let event = match engine.speak(chunk, voice, speed).and_then(to_output_rate) {
            Ok(samples) => SpeechEvent::Audio {
                samples,
                done: index + 1,
                total,
            },
            Err(error) => {
                let _ = events.send(SpeechEvent::Failed(error));
                return;
            }
        };
        if events.send(event).is_err() {
            return;
        }
    }
    let _ = events.send(SpeechEvent::Done);
}

fn to_output_rate(pcm: Pcm) -> Result<Vec<f32>, SpeechError> {
    let mut resampler = MonoResampler::to_rate(pcm.rate, SAMPLE_RATE)
        .map_err(|error| SpeechError::failed(error.detail))?;
    let mut out = Vec::new();
    resampler.process(&pcm.samples, &mut out);
    resampler.finish(&mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Voice;
    use hushpen_core::error::SPEECH_VOICE_MISSING;
    use std::sync::Mutex;
    use std::time::Duration;

    /// Speaks one sample per byte at half the output rate. Each chunk waits for a token when
    /// it has a gate.
    struct FakeEngine {
        gate: Option<Mutex<Receiver<()>>>,
        spoken: Mutex<Vec<(String, f32)>>,
    }

    impl FakeEngine {
        fn new(gate: Option<Receiver<()>>) -> Arc<Self> {
            Arc::new(Self {
                gate: gate.map(Mutex::new),
                spoken: Mutex::new(Vec::new()),
            })
        }
    }

    impl SpeechEngine for FakeEngine {
        fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
            Ok(Vec::new())
        }

        fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
            if let Some(gate) = &self.gate {
                let _ = gate.lock().unwrap().recv();
            }
            if voice == Some("gone") {
                return Err(SpeechError::new(SPEECH_VOICE_MISSING, "gone"));
            }
            self.spoken.lock().unwrap().push((text.to_owned(), speed));
            Ok(Pcm {
                rate: SAMPLE_RATE / 2,
                samples: vec![0.25; text.len()],
            })
        }
    }

    /// Five sentences that are one chunk each.
    fn five_chunks() -> String {
        (1..=5)
            .map(|n| format!("Sentence {n} {}.", "word ".repeat(40)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn collect(synthesis: &Synthesis) -> Vec<SpeechEvent> {
        let mut events = Vec::new();
        while let Ok(event) = synthesis.events().recv_timeout(Duration::from_secs(10)) {
            let last = !matches!(event, SpeechEvent::Audio { .. });
            events.push(event);
            if last {
                break;
            }
        }
        events
    }

    #[test]
    fn every_chunk_streams_at_the_output_rate_with_progress() {
        let engine = FakeEngine::new(None);
        let text = five_chunks();
        let synthesis = synthesize(engine.clone(), text.clone(), None, 9.0);
        let events = collect(&synthesis);
        assert_eq!(events.len(), 6);
        assert_eq!(events[5], SpeechEvent::Done);
        let spoken = engine.spoken.lock().unwrap().clone();
        for (index, event) in events[..5].iter().enumerate() {
            let SpeechEvent::Audio {
                samples,
                done,
                total,
            } = event
            else {
                panic!("not audio: {event:?}");
            };
            assert_eq!((*done, *total), (index + 1, 5));
            assert_eq!(samples.len(), spoken[index].0.len() * 2);
            assert_eq!(spoken[index].1, MAX_SPEED);
        }
    }

    #[test]
    fn cancel_stops_after_the_current_chunk() {
        let (tokens, gate) = mpsc::channel();
        let synthesis = synthesize(FakeEngine::new(Some(gate)), five_chunks(), None, 1.0);
        tokens.send(()).unwrap();
        let first = synthesis.events().recv_timeout(Duration::from_secs(10));
        assert!(matches!(first, Ok(SpeechEvent::Audio { done: 1, .. })));
        synthesis.cancel();
        // Lets a chunk that already started finish.
        tokens.send(()).unwrap();
        let rest = collect(&synthesis);
        assert_eq!(rest.last(), Some(&SpeechEvent::Cancelled));
        assert!(rest.len() <= 2);
    }

    #[test]
    fn an_engine_error_ends_the_run() {
        let synthesis = synthesize(
            FakeEngine::new(None),
            five_chunks(),
            Some("gone".into()),
            1.0,
        );
        let events = collect(&synthesis);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], SpeechEvent::Failed(error) if error.code == SPEECH_VOICE_MISSING)
        );
    }

    #[test]
    fn dropping_the_synthesis_stops_the_thread() {
        let (tokens, gate) = mpsc::channel();
        let engine = FakeEngine::new(Some(gate));
        drop(synthesize(engine.clone(), five_chunks(), None, f32::NAN));
        for _ in 0..5 {
            let _ = tokens.send(());
        }
        thread::sleep(Duration::from_millis(200));
        let spoken = engine.spoken.lock().unwrap();
        assert!(spoken.len() <= 1);
        assert!(spoken.iter().all(|(_, speed)| *speed == 1.0));
    }
}
