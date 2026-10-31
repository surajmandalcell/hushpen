//! The worker's request loop with a fake model: audio frames, ordering, cancel, idle unload.

use hushpen_core::error;
use hushpen_core::kokoro::{DEFAULT_VOICE, VOICES, find_voice};
use hushpen_core::llm_protocol::UnloadReason;
use hushpen_core::protocol::{FrameKind, write_frame, write_json};
use hushpen_core::tts_protocol::{
    MAX_AUDIO_SAMPLES, TTS_PROTOCOL_VERSION, TtsEngine, TtsEvent, TtsRequest, read_event,
};
use hushpen_tts::backend::{Backend, CancelFlag, JobError, LoadError, LoadSpec, Session};
use hushpen_tts::server::{EXIT_BAD_FRAME, EXIT_BAD_PROTOCOL, ServerConfig, serve};
use std::io::{PipeReader, PipeWriter};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const RATE: u32 = 24_000;
/// Every load of the fake takes this long.
const LOAD_MS: u64 = 20;

/// A model whose audio holds the speaker number: each sample is `speaker / 100`. It writes 100
/// samples for each byte of text, in three pieces over `job_ms`. A voice that is not a Kokoro
/// voice fails the job. A folder with `bad` in its name
/// fails to load. The text `explode` fails, `long` writes more than one frame holds at once.
struct Fake {
    job_ms: u64,
    loads: Arc<AtomicUsize>,
}

struct FakeSession {
    job_ms: u64,
}

impl Session for FakeSession {
    fn sample_rate(&self) -> u32 {
        RATE
    }

    fn speak(
        &mut self,
        text: &str,
        voice: Option<&str>,
        _speed: f32,
        cancel: &CancelFlag,
        sink: &mut dyn FnMut(&[f32]),
    ) -> Result<(), JobError> {
        if text == "explode" {
            return Err(JobError::Failed {
                code: error::SPEECH_FAILED,
                detail: "boom".into(),
            });
        }
        let voice = voice.unwrap_or(DEFAULT_VOICE);
        let Some(found) = find_voice(voice) else {
            return Err(JobError::Failed {
                code: error::SPEECH_VOICE_MISSING,
                detail: voice.to_owned(),
            });
        };
        let value = found.speaker as f32 / 100.0;
        if text == "long" {
            sink(&vec![value; MAX_AUDIO_SAMPLES + 10]);
            return Ok(());
        }
        let piece = vec![value; text.len() * 100 / 3];
        for _ in 0..3 {
            let started = Instant::now();
            while started.elapsed() < Duration::from_millis(self.job_ms / 3) {
                if cancel.is_cancelled() {
                    return Err(JobError::Cancelled);
                }
                thread::sleep(Duration::from_millis(2));
            }
            sink(&piece);
        }
        Ok(())
    }
}

impl Backend for Fake {
    fn load(&self, spec: &LoadSpec) -> Result<Box<dyn Session>, LoadError> {
        thread::sleep(Duration::from_millis(LOAD_MS));
        if spec.dir.to_string_lossy().contains("bad") {
            return Err(LoadError {
                detail: "not a model".into(),
            });
        }
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeSession {
            job_ms: self.job_ms,
        }))
    }
}

struct Worker {
    to_worker: PipeWriter,
    from_worker: PipeReader,
    exit: thread::JoinHandle<u8>,
    loads: Arc<AtomicUsize>,
}

/// What one job wrote: its audio and how it ended.
struct Spoken {
    samples: Vec<f32>,
    pieces: usize,
    end: TtsEvent,
}

fn config(job_ms: u64, loads: &Arc<AtomicUsize>) -> ServerConfig {
    ServerConfig {
        version: "test".into(),
        backend: Arc::new(Fake {
            job_ms,
            loads: Arc::clone(loads),
        }),
    }
}

impl Worker {
    fn start(job_ms: u64) -> Self {
        let (worker_in, to_worker) = std::io::pipe().unwrap();
        let (from_worker, worker_out) = std::io::pipe().unwrap();
        let loads = Arc::new(AtomicUsize::new(0));
        let config = config(job_ms, &loads);
        let exit = thread::spawn(move || serve(worker_in, worker_out, config));
        Self {
            to_worker,
            from_worker,
            exit,
            loads,
        }
    }

    fn send(&mut self, request: &TtsRequest) {
        write_json(&mut self.to_worker, &request.to_json()).unwrap();
    }

    fn next(&mut self) -> TtsEvent {
        read_event(&mut self.from_worker)
            .unwrap()
            .expect("an event")
    }

    fn load_with(&mut self, name: &str, idle_secs: u64) -> TtsEvent {
        self.send(&TtsRequest::LoadModel {
            dir: format!("/models/tts/{name}"),
            engine: TtsEngine::Kokoro,
            threads: 2,
            idle_secs,
        });
        assert!(matches!(self.next(), TtsEvent::Loading { .. }));
        self.next()
    }

    fn load(&mut self) -> TtsEvent {
        self.load_with("kokoro-82m-int8", 300)
    }

    fn speak(&mut self, job: u64, text: &str, voice: Option<&str>) {
        self.send(&TtsRequest::Speak {
            job,
            text: text.into(),
            voice: voice.map(str::to_owned),
            speed: 1.0,
        });
    }

    /// Reads the audio of `job` up to its end.
    fn spoken(&mut self, job: u64) -> Spoken {
        let mut spoken = Spoken {
            samples: Vec::new(),
            pieces: 0,
            end: TtsEvent::Pong,
        };
        loop {
            match self.next() {
                TtsEvent::Audio {
                    job: from,
                    sample_rate,
                    samples,
                } if from == job => {
                    assert_eq!(sample_rate, RATE);
                    assert!(samples.len() <= MAX_AUDIO_SAMPLES);
                    spoken.samples.extend(samples);
                    spoken.pieces += 1;
                }
                end => {
                    spoken.end = end;
                    return spoken;
                }
            }
        }
    }

    fn finish(mut self) -> u8 {
        self.send(&TtsRequest::Shutdown);
        self.exit.join().unwrap()
    }
}

fn speaker_of(voice: &str) -> f32 {
    find_voice(voice).map(|voice| voice.speaker).unwrap() as f32 / 100.0
}

#[test]
fn hello_gets_ready_with_the_version_and_protocol() {
    let mut worker = Worker::start(0);
    worker.send(&TtsRequest::Hello {
        protocol: TTS_PROTOCOL_VERSION,
    });
    assert_eq!(
        worker.next(),
        TtsEvent::Ready {
            version: "test".into(),
            protocol: TTS_PROTOCOL_VERSION,
        }
    );
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_protocol_mismatch_is_answered_with_a_coded_error_and_an_exit() {
    let mut worker = Worker::start(0);
    worker.send(&TtsRequest::Hello { protocol: 99 });
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { code, .. } if code == error::TTS_PROTOCOL
    ));
    assert_eq!(worker.exit.join().unwrap(), EXIT_BAD_PROTOCOL);
}

#[test]
fn a_bad_request_is_answered_and_a_bad_frame_ends_the_worker() {
    let mut worker = Worker::start(0);
    write_frame(&mut worker.to_worker, FrameKind::Json, b"not json at all").unwrap();
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { code, .. } if code == error::TTS_PROTOCOL
    ));
    write_frame(&mut worker.to_worker, FrameKind::Binary, &[0, 0, 0, 0]).unwrap();
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { code, .. } if code == error::TTS_PROTOCOL
    ));
    worker.send(&TtsRequest::Ping);
    assert_eq!(worker.next(), TtsEvent::Pong);
    drop(worker.to_worker);
    assert_eq!(worker.exit.join().unwrap(), 0);

    let (worker_in, mut to_worker) = std::io::pipe().unwrap();
    let (_from_worker, worker_out) = std::io::pipe().unwrap();
    let config = config(0, &Arc::default());
    let exit = thread::spawn(move || serve(worker_in, worker_out, config));
    // A length over the limit cannot be read past.
    std::io::Write::write_all(&mut to_worker, &[0xff, 0xff, 0xff, 0xff, 0]).unwrap();
    assert_eq!(exit.join().unwrap(), EXIT_BAD_FRAME);
}

#[test]
fn the_voice_list_holds_the_english_voices() {
    let mut worker = Worker::start(0);
    worker.send(&TtsRequest::ListVoices);
    let TtsEvent::Voices { voices } = worker.next() else {
        panic!("a voice list");
    };
    assert_eq!(voices.len(), VOICES.len());
    let heart = voices
        .iter()
        .find(|voice| voice.id == DEFAULT_VOICE)
        .unwrap();
    assert_eq!(
        (heart.name.as_str(), heart.language.as_str()),
        ("Heart", "en-US")
    );
    assert!(voices.iter().any(|voice| voice.language == "en-GB"));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_job_streams_its_audio_then_ends_with_its_times() {
    let mut worker = Worker::start(30);
    let loaded = worker.load();
    let TtsEvent::Loaded { model, ms } = loaded else {
        panic!("loaded, got {loaded:?}");
    };
    assert_eq!(model, "kokoro-82m-int8");
    assert!(ms >= LOAD_MS, "{ms}");
    worker.speak(1, "Hello there.", Some("bm_george"));
    let spoken = worker.spoken(1);
    assert_eq!(spoken.pieces, 3, "one audio event for each piece");
    assert_eq!(spoken.samples.len(), 3 * (12 * 100 / 3));
    assert!(
        spoken
            .samples
            .iter()
            .all(|&sample| sample == speaker_of("bm_george"))
    );
    let TtsEvent::Done { job: 1, stats } = spoken.end else {
        panic!("done, got {:?}", spoken.end);
    };
    assert_eq!(stats.audio_ms, 1200 * 1000 / u64::from(RATE));
    assert_eq!(stats.load_ms, 0, "the model was warm");
    assert!(stats.first_audio_ms > 0 && stats.first_audio_ms <= stats.ms);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn no_voice_means_the_default_voice_and_long_audio_is_split_into_frames() {
    let mut worker = Worker::start(0);
    worker.load();
    worker.speak(1, "long", None);
    let spoken = worker.spoken(1);
    assert_eq!(spoken.pieces, 2);
    assert_eq!(spoken.samples.len(), MAX_AUDIO_SAMPLES + 10);
    assert_eq!(spoken.samples[0], speaker_of(DEFAULT_VOICE));
    assert!(matches!(spoken.end, TtsEvent::Done { job: 1, .. }));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn empty_text_an_unknown_voice_and_no_model_get_coded_errors() {
    let mut worker = Worker::start(0);
    worker.speak(1, "hello", None);
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { job: Some(1), code, .. } if code == error::TTS_NO_MODEL
    ));
    worker.speak(2, "  \n", None);
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { job: Some(2), code, .. } if code == error::SPEECH_NO_TEXT
    ));
    assert_eq!(worker.loads.load(Ordering::SeqCst), 0);
    worker.load();
    worker.speak(3, "hello", Some("zf_xiaobei"));
    assert!(matches!(
        worker.spoken(3).end,
        TtsEvent::Error { job: Some(3), code, .. } if code == error::SPEECH_VOICE_MISSING
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_failed_load_reports_the_load_code_and_fails_the_waiting_job() {
    let mut worker = Worker::start(0);
    assert!(matches!(
        worker.load_with("bad-model", 300),
        TtsEvent::Error { job: None, code, .. } if code == error::TTS_LOAD_FAILED
    ));
    worker.speak(3, "hi", None);
    assert!(matches!(worker.next(), TtsEvent::Loading { .. }));
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { job: None, code, .. } if code == error::TTS_LOAD_FAILED
    ));
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { job: Some(3), code, .. } if code == error::TTS_LOAD_FAILED
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_failing_job_gets_its_own_code_and_the_next_job_still_runs() {
    let mut worker = Worker::start(0);
    worker.load();
    worker.speak(1, "explode", None);
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { job: Some(1), code, .. } if code == error::SPEECH_FAILED
    ));
    worker.speak(2, "fine", None);
    assert!(matches!(
        worker.spoken(2).end,
        TtsEvent::Done { job: 2, .. }
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn ping_and_the_voice_list_are_answered_while_a_job_runs() {
    let mut worker = Worker::start(600);
    worker.load();
    worker.speak(1, "hello", None);
    thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    worker.send(&TtsRequest::Ping);
    worker.send(&TtsRequest::ListVoices);
    assert_eq!(worker.next(), TtsEvent::Pong);
    assert!(matches!(worker.next(), TtsEvent::Voices { .. }));
    assert!(
        started.elapsed() < Duration::from_millis(150),
        "the answers waited for the job"
    );
    assert!(matches!(
        worker.spoken(1).end,
        TtsEvent::Done { job: 1, .. }
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn cancel_stops_a_running_job_and_settles_a_queued_one_at_once() {
    let mut worker = Worker::start(3000);
    worker.load();
    worker.speak(1, "running", None);
    worker.speak(2, "queued", None);
    thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    worker.send(&TtsRequest::Cancel { job: 2 });
    assert_eq!(worker.next(), TtsEvent::Cancelled { job: 2 });
    worker.send(&TtsRequest::Cancel { job: 1 });
    assert_eq!(worker.spoken(1).end, TtsEvent::Cancelled { job: 1 });
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "cancel took {:?}",
        started.elapsed()
    );
    worker.send(&TtsRequest::Cancel { job: 9 });
    worker.send(&TtsRequest::Ping);
    assert_eq!(
        worker.next(),
        TtsEvent::Pong,
        "a cancel of no job is ignored"
    );
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_reused_job_id_is_refused() {
    let mut worker = Worker::start(300);
    worker.load();
    worker.speak(1, "first", None);
    worker.speak(1, "again", None);
    assert!(matches!(
        worker.next(),
        TtsEvent::Error { job: Some(1), code, .. } if code == error::TTS_PROTOCOL
    ));
    assert!(matches!(
        worker.spoken(1).end,
        TtsEvent::Done { job: 1, .. }
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn an_idle_model_is_dropped_and_the_next_job_loads_it_again_by_itself() {
    let mut worker = Worker::start(0);
    worker.load_with("kokoro-82m-int8", 1);
    worker.speak(1, "before", None);
    assert!(matches!(
        worker.spoken(1).end,
        TtsEvent::Done { job: 1, .. }
    ));
    assert_eq!(
        worker.next(),
        TtsEvent::Unloaded {
            reason: UnloadReason::Idle
        }
    );
    assert_eq!(worker.loads.load(Ordering::SeqCst), 1);

    worker.speak(2, "after", None);
    assert!(matches!(worker.next(), TtsEvent::Loading { .. }));
    assert!(matches!(worker.next(), TtsEvent::Loaded { .. }));
    let TtsEvent::Done { job: 2, stats } = worker.spoken(2).end else {
        panic!("done");
    };
    assert!(stats.load_ms >= LOAD_MS, "the reload counts in the job");
    assert!(stats.ms >= stats.load_ms);
    assert_eq!(worker.loads.load(Ordering::SeqCst), 2);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn unload_drops_the_model_and_a_second_load_of_it_answers_at_once() {
    let mut worker = Worker::start(0);
    worker.load();
    worker.send(&TtsRequest::LoadModel {
        dir: "/models/tts/kokoro-82m-int8".into(),
        engine: TtsEngine::Kokoro,
        threads: 2,
        idle_secs: 300,
    });
    assert!(matches!(worker.next(), TtsEvent::Loaded { ms: 0, .. }));
    assert_eq!(worker.loads.load(Ordering::SeqCst), 1);
    worker.send(&TtsRequest::Unload);
    assert_eq!(
        worker.next(),
        TtsEvent::Unloaded {
            reason: UnloadReason::Requested
        }
    );
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_new_idle_time_for_the_loaded_model_keeps_it_and_applies_at_once() {
    let mut worker = Worker::start(0);
    worker.load_with("kokoro-82m-int8", 0);
    worker.send(&TtsRequest::LoadModel {
        dir: "/models/tts/kokoro-82m-int8".into(),
        engine: TtsEngine::Kokoro,
        threads: 2,
        idle_secs: 1,
    });
    assert!(matches!(worker.next(), TtsEvent::Loaded { ms: 0, .. }));
    assert_eq!(
        worker.next(),
        TtsEvent::Unloaded {
            reason: UnloadReason::Idle
        }
    );
    assert_eq!(worker.loads.load(Ordering::SeqCst), 1);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn the_end_of_the_input_ends_the_worker_even_during_a_job() {
    let mut worker = Worker::start(5000);
    worker.load();
    worker.speak(1, "long text", None);
    thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    drop(worker.to_worker);
    assert_eq!(worker.exit.join().unwrap(), 0);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the worker lingered {:?}",
        started.elapsed()
    );
}
