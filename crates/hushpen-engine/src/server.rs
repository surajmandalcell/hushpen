//! The engine child: reads requests from one pipe, writes events to another.
//!
//! The reading thread never blocks on a model. It answers `Ping`, handles `Cancel`, and queues
//! `LoadModel` and `Transcribe` for one worker thread that owns the loaded model. That is why a
//! ping still gets its pong during a load or a long job, and why a cancel can settle a job that
//! has not started yet without waiting for the job in front of it.

use crate::asr::{AsrEngine, CancelFlag, EngineError, LoadOptions, TranscribeOptions};
use crate::wav;
use hushpen_core::error;
use hushpen_core::protocol::{
    DecodeError, Event, Frame, FrameError, FrameKind, PROTOCOL_VERSION, Request, read_frame,
    write_json,
};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// Exit code after a frame the child cannot read past. The supervisor restarts it.
pub const EXIT_BAD_FRAME: u8 = 3;
/// Exit code after a `Hello` with a protocol version this child does not speak.
pub const EXIT_BAD_PROTOCOL: u8 = 4;

/// How long a shutdown waits for a running job to notice its cancel flag.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(700);

pub type Loader =
    dyn Fn(&Path, LoadOptions) -> Result<Box<dyn AsrEngine>, EngineError> + Send + Sync;

pub struct ServerConfig {
    /// Reported in `Ready`.
    pub version: String,
    /// True when a GPU backend is built in and not switched off.
    pub gpu_available: bool,
    pub loader: Arc<Loader>,
}

type Output = Arc<Mutex<Box<dyn Write + Send>>>;

struct JobEntry {
    cancel: CancelFlag,
    started: bool,
}

type Jobs = Arc<Mutex<HashMap<u64, JobEntry>>>;

enum Work {
    Load {
        path: PathBuf,
        gpu: bool,
        threads: usize,
    },
    Transcribe {
        job: u64,
        wav_path: PathBuf,
        options: TranscribeOptions,
    },
}

/// Runs the request loop until `Shutdown`, the end of the input, or a bad frame. Returns the
/// exit code of the child.
pub fn serve(input: impl Read, output: impl Write + Send + 'static, config: ServerConfig) -> u8 {
    let output: Output = Arc::new(Mutex::new(Box::new(output)));
    let jobs: Jobs = Arc::default();
    let (work_tx, work_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = Worker {
        output: Arc::clone(&output),
        jobs: Arc::clone(&jobs),
        loader: Arc::clone(&config.loader),
        gpu_available: config.gpu_available,
        held_model: Mutex::new(None),
    };
    thread::spawn(move || {
        worker.run(&work_rx);
        let _ = done_tx.send(());
    });

    let code = read_requests(input, &output, &jobs, &work_tx, &config);

    for entry in lock(&jobs).values() {
        entry.cancel.cancel();
    }
    drop(work_tx);
    // A job that ignores its flag must not keep the child alive: the process exits anyway.
    let _ = done_rx.recv_timeout(SHUTDOWN_GRACE);
    code
}

fn read_requests(
    mut input: impl Read,
    output: &Output,
    jobs: &Jobs,
    work: &Sender<Work>,
    config: &ServerConfig,
) -> u8 {
    loop {
        let frame = match read_frame(&mut input) {
            Ok(Some(frame)) => frame,
            Ok(None) => return 0,
            Err(error) => {
                eprintln!("engine: {error}");
                if !matches!(error, FrameError::Io(_)) {
                    send(
                        output,
                        &Event::error(None, error::ENGINE_PROTOCOL, error.to_string()),
                    );
                }
                return EXIT_BAD_FRAME;
            }
        };
        let request = match decode(&frame) {
            Ok(request) => request,
            Err(problem) => {
                send(
                    output,
                    &Event::error(None, error::ENGINE_PROTOCOL, problem.to_string()),
                );
                continue;
            }
        };
        match request {
            Request::Hello { protocol } if protocol == PROTOCOL_VERSION => send(
                output,
                &Event::Ready {
                    version: config.version.clone(),
                    protocol: PROTOCOL_VERSION,
                    gpu: config.gpu_available,
                },
            ),
            Request::Hello { protocol } => {
                send(
                    output,
                    &Event::error(
                        None,
                        error::ENGINE_PROTOCOL,
                        format!(
                            "protocol {protocol} is not supported, this engine speaks {PROTOCOL_VERSION}"
                        ),
                    ),
                );
                return EXIT_BAD_PROTOCOL;
            }
            Request::LoadModel { path, gpu, threads } => {
                let _ = work.send(Work::Load {
                    path: PathBuf::from(path),
                    gpu,
                    threads: threads as usize,
                });
            }
            Request::Transcribe {
                job,
                wav_path,
                language,
                prompt,
            } => {
                let fresh = match lock(jobs).entry(job) {
                    Entry::Occupied(_) => false,
                    Entry::Vacant(slot) => {
                        slot.insert(JobEntry {
                            cancel: CancelFlag::new(),
                            started: false,
                        });
                        true
                    }
                };
                if fresh {
                    let _ = work.send(Work::Transcribe {
                        job,
                        wav_path: PathBuf::from(wav_path),
                        options: TranscribeOptions { language, prompt },
                    });
                } else {
                    send(
                        output,
                        &Event::error(Some(job), error::ENGINE_PROTOCOL, "job id is in use"),
                    );
                }
            }
            Request::Cancel { job } => cancel(job, jobs, output),
            Request::Ping => send(output, &Event::Pong),
            Request::Shutdown => return 0,
        }
    }
}

fn decode(frame: &Frame) -> Result<Request, DecodeError> {
    if frame.kind == FrameKind::Binary {
        return Err(DecodeError::Invalid(
            "the engine takes no binary frames".into(),
        ));
    }
    Request::from_frame(frame)
}

/// A job that is running stops through its flag. A job that has not started is settled here, so
/// a cancel does not wait for the load or the job in front of it.
fn cancel(job: u64, jobs: &Jobs, output: &Output) {
    let mut jobs = lock(jobs);
    match jobs.get(&job) {
        Some(entry) if entry.started => entry.cancel.cancel(),
        Some(_) => {
            jobs.remove(&job);
            send(output, &Event::Cancelled { job });
        }
        None => {}
    }
}

struct Worker {
    output: Output,
    jobs: Jobs,
    loader: Arc<Loader>,
    gpu_available: bool,
    /// whisper.cpp reads the model and closes the file. Holding it open keeps the model file
    /// visible in `/proc/<pid>/fd` and `lsof`, which shows which file this process serves.
    held_model: Mutex<Option<File>>,
}

impl Worker {
    fn run(&self, work: &Receiver<Work>) {
        let mut engine: Option<Box<dyn AsrEngine>> = None;
        while let Ok(item) = work.recv() {
            match item {
                Work::Load { path, gpu, threads } => self.load(&mut engine, &path, gpu, threads),
                Work::Transcribe {
                    job,
                    wav_path,
                    options,
                } => self.transcribe(engine.as_deref_mut(), job, &wav_path, &options),
            }
        }
    }

    /// A failed load leaves the old model in place.
    fn load(
        &self,
        engine: &mut Option<Box<dyn AsrEngine>>,
        path: &Path,
        gpu: bool,
        threads: usize,
    ) {
        let model = model_name(path);
        send(
            &self.output,
            &Event::Loading {
                model: model.clone(),
            },
        );
        let started = Instant::now();
        let options = LoadOptions {
            gpu: gpu && self.gpu_available,
            threads: threads.max(1),
        };
        match (self.loader)(path, options) {
            Ok(loaded) => {
                let gpu = loaded.gpu_in_use();
                *engine = Some(loaded);
                *lock(&self.held_model) = File::open(path).ok();
                send(
                    &self.output,
                    &Event::Loaded {
                        model,
                        ms: elapsed_ms(started),
                        gpu,
                    },
                );
            }
            Err(failure) => {
                let code = failure.code().unwrap_or(error::ENGINE_LOAD_FAILED);
                let event = match failure.reason() {
                    Some(reason) => {
                        Event::error_with_reason(None, code, failure.to_string(), reason)
                    }
                    None => Event::error(None, code, failure.to_string()),
                };
                send(&self.output, &event);
            }
        }
    }

    fn transcribe(
        &self,
        engine: Option<&mut (dyn AsrEngine + 'static)>,
        job: u64,
        wav_path: &Path,
        options: &TranscribeOptions,
    ) {
        let Some(cancel) = self.start(job) else {
            return;
        };
        let event = self.run_job(engine, job, wav_path, options, &cancel);
        lock(&self.jobs).remove(&job);
        send(&self.output, &event);
    }

    /// Marks the job as running. `None` when a cancel already settled it.
    fn start(&self, job: u64) -> Option<CancelFlag> {
        let mut jobs = lock(&self.jobs);
        let entry = jobs.get_mut(&job)?;
        entry.started = true;
        Some(entry.cancel.clone())
    }

    fn run_job(
        &self,
        engine: Option<&mut (dyn AsrEngine + 'static)>,
        job: u64,
        wav_path: &Path,
        options: &TranscribeOptions,
        cancel: &CancelFlag,
    ) -> Event {
        let Some(engine) = engine else {
            return Event::error(Some(job), error::ENGINE_NO_MODEL, "no model is loaded");
        };
        let pcm = match wav::read_mono_16k(wav_path) {
            Ok(pcm) => pcm,
            Err(detail) => return Event::error(Some(job), error::ENGINE_BAD_AUDIO, detail),
        };
        let audio_ms = pcm.len() as u64 * 1000 / u64::from(wav::SAMPLE_RATE);
        let started = Instant::now();
        match engine.transcribe(&pcm, options, cancel) {
            Ok(transcript) => {
                for segment in &transcript.segments {
                    send(
                        &self.output,
                        &Event::Segment {
                            job,
                            segment: segment.clone(),
                        },
                    );
                }
                Event::Result {
                    job,
                    text: transcript.text,
                    language: transcript.language,
                    segments: transcript.segments,
                    audio_ms,
                    ms: elapsed_ms(started),
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
}

/// `ggml-tiny.en.bin` is the model `tiny.en`.
pub fn model_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    stem.strip_prefix("ggml-").unwrap_or(&stem).to_owned()
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn send(output: &Output, event: &Event) {
    // A broken pipe means the app is gone; the reader sees the same and ends the loop.
    let _ = write_json(&mut *lock(output), &event.to_json());
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::Transcript;
    use hushpen_core::protocol::{Segment, write_frame};
    use serde_json::json;
    use std::io::PipeReader;
    use std::io::PipeWriter;

    struct Fake {
        /// How long a job takes if nobody cancels it.
        job_ms: u64,
    }

    impl AsrEngine for Fake {
        fn gpu_in_use(&self) -> bool {
            false
        }

        fn transcribe(
            &mut self,
            pcm: &[f32],
            options: &TranscribeOptions,
            cancel: &CancelFlag,
        ) -> Result<Transcript, EngineError> {
            if options.language.as_deref() == Some("xx") {
                return Err(EngineError::BadLanguage("xx".into()));
            }
            let started = Instant::now();
            while started.elapsed() < Duration::from_millis(self.job_ms) {
                if cancel.is_cancelled() {
                    return Err(EngineError::Cancelled);
                }
                thread::sleep(Duration::from_millis(2));
            }
            Ok(Transcript {
                text: format!("{} samples", pcm.len()),
                language: "en".into(),
                segments: vec![
                    Segment {
                        start_ms: 0,
                        end_ms: 400,
                        text: "one".into(),
                    },
                    Segment {
                        start_ms: 400,
                        end_ms: 900,
                        text: "two".into(),
                    },
                ],
            })
        }
    }

    /// A loader where a path with `bad` fails and a path with `slow` takes 300 ms.
    fn fake_loader(job_ms: u64) -> Arc<Loader> {
        Arc::new(move |path, _options| {
            let name = path.to_string_lossy();
            if name.contains("slow") {
                thread::sleep(Duration::from_millis(300));
            }
            if name.contains("bad") {
                return Err(EngineError::LoadFailed("not a model".into()));
            }
            if name.contains("oldcpu") {
                return Err(EngineError::UnsupportedCpu("AVX2"));
            }
            Ok(Box::new(Fake { job_ms }) as Box<dyn AsrEngine>)
        })
    }

    struct Session {
        to_engine: PipeWriter,
        from_engine: PipeReader,
        exit: thread::JoinHandle<u8>,
        dir: tempfile::TempDir,
    }

    impl Session {
        fn start(job_ms: u64) -> Self {
            let (engine_in, to_engine) = std::io::pipe().unwrap();
            let (from_engine, engine_out) = std::io::pipe().unwrap();
            let config = ServerConfig {
                version: "test".into(),
                gpu_available: false,
                loader: fake_loader(job_ms),
            };
            let exit = thread::spawn(move || serve(engine_in, engine_out, config));
            Self {
                to_engine,
                from_engine,
                exit,
                dir: tempfile::tempdir().unwrap(),
            }
        }

        fn send(&mut self, request: &Request) {
            write_json(&mut self.to_engine, &request.to_json()).unwrap();
        }

        fn next(&mut self) -> Event {
            let frame = read_frame(&mut self.from_engine).unwrap().expect("a frame");
            Event::from_frame(&frame).unwrap()
        }

        fn wav(&self) -> String {
            let path = self.dir.path().join("a.wav");
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            for _ in 0..1600 {
                writer.write_sample(0_i16).unwrap();
            }
            writer.finalize().unwrap();
            path.to_string_lossy().into_owned()
        }

        fn load(&mut self, name: &str) -> Event {
            self.send(&Request::LoadModel {
                path: format!("/models/{name}"),
                gpu: false,
                threads: 2,
            });
            assert!(matches!(self.next(), Event::Loading { .. }));
            self.next()
        }

        fn transcribe(&mut self, job: u64, language: Option<&str>) {
            let wav_path = self.wav();
            self.send(&Request::Transcribe {
                job,
                wav_path,
                language: language.map(str::to_owned),
                prompt: None,
            });
        }

        fn finish(mut self) -> u8 {
            self.send(&Request::Shutdown);
            self.exit.join().unwrap()
        }
    }

    #[test]
    fn hello_gets_ready_with_the_version_and_protocol() {
        let mut session = Session::start(0);
        session.send(&Request::Hello {
            protocol: PROTOCOL_VERSION,
        });
        assert_eq!(
            session.next(),
            Event::Ready {
                version: "test".into(),
                protocol: PROTOCOL_VERSION,
                gpu: false
            }
        );
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_protocol_mismatch_is_answered_with_a_coded_error_and_an_exit() {
        let mut session = Session::start(0);
        session.send(&Request::Hello { protocol: 99 });
        assert!(matches!(
            session.next(),
            Event::Error { code, .. } if code == error::ENGINE_PROTOCOL
        ));
        assert_eq!(session.exit.join().unwrap(), EXIT_BAD_PROTOCOL);
    }

    #[test]
    fn load_then_transcribe_gives_segments_then_a_result() {
        let mut session = Session::start(0);
        let loaded = session.load("ggml-tiny.en.bin");
        assert!(matches!(&loaded, Event::Loaded { model, .. } if model == "tiny.en"));
        session.transcribe(1, Some("en"));
        for expected in [(0, 400, "one"), (400, 900, "two")] {
            match session.next() {
                Event::Segment { job: 1, segment } => assert_eq!(
                    (segment.start_ms, segment.end_ms, segment.text.as_str()),
                    expected
                ),
                other => panic!("expected a segment, got {other:?}"),
            }
        }
        match session.next() {
            Event::Result {
                job,
                text,
                language,
                audio_ms,
                ..
            } => {
                assert_eq!(
                    (job, text.as_str(), language.as_str()),
                    (1, "1600 samples", "en")
                );
                assert_eq!(audio_ms, 100);
            }
            other => panic!("expected a result, got {other:?}"),
        }
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_job_before_any_model_gets_a_coded_error() {
        let mut session = Session::start(0);
        session.transcribe(5, None);
        assert!(matches!(
            session.next(),
            Event::Error { job: Some(5), code, .. } if code == error::ENGINE_NO_MODEL
        ));
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_wrong_language_is_a_coded_error_and_the_next_job_works() {
        let mut session = Session::start(0);
        session.load("ggml-tiny.en.bin");
        session.transcribe(1, Some("xx"));
        assert!(matches!(
            session.next(),
            Event::Error { job: Some(1), code, .. } if code == error::ENGINE_BAD_LANGUAGE
        ));
        session.transcribe(2, Some("en"));
        loop {
            match session.next() {
                Event::Segment { .. } => {}
                Event::Result { job: 2, .. } => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn an_unreadable_wav_is_a_coded_error() {
        let mut session = Session::start(0);
        session.load("ggml-tiny.en.bin");
        session.send(&Request::Transcribe {
            job: 1,
            wav_path: "/does/not/exist.wav".into(),
            language: None,
            prompt: None,
        });
        assert!(matches!(
            session.next(),
            Event::Error { job: Some(1), code, .. } if code == error::ENGINE_BAD_AUDIO
        ));
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_failed_load_is_coded_and_the_old_model_keeps_working() {
        let mut session = Session::start(0);
        session.load("ggml-tiny.en.bin");
        assert!(matches!(session.load("ggml-bad.bin"), Event::Error {
            job: None, code, ..
        } if code == error::ENGINE_LOAD_FAILED));
        session.transcribe(1, None);
        loop {
            match session.next() {
                Event::Segment { .. } => {}
                Event::Result { job: 1, .. } => break,
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_cpu_that_lacks_an_instruction_set_is_a_load_failure_with_the_cpu_reason() {
        let mut session = Session::start(0);
        let Event::Error {
            job: None,
            code,
            params,
        } = session.load("ggml-oldcpu.bin")
        else {
            panic!("expected a load error");
        };
        assert_eq!(code, error::ENGINE_LOAD_FAILED);
        assert_eq!(params.get("reason"), Some(&json!("cpu")));
        session.send(&Request::Ping);
        assert_eq!(session.next(), Event::Pong);
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn an_unknown_request_is_a_coded_error_and_the_engine_keeps_answering() {
        let mut session = Session::start(0);
        write_json(&mut session.to_engine, &json!({"type": "dance"})).unwrap();
        assert!(matches!(
            session.next(),
            Event::Error { job: None, code, .. } if code == error::ENGINE_PROTOCOL
        ));
        write_frame(&mut session.to_engine, FrameKind::Binary, &[1, 2]).unwrap();
        assert!(matches!(session.next(), Event::Error { .. }));
        session.send(&Request::Ping);
        assert_eq!(session.next(), Event::Pong);
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_ping_is_answered_while_a_job_runs_and_a_cancel_stops_it() {
        let mut session = Session::start(10_000);
        session.load("ggml-tiny.en.bin");
        session.transcribe(1, None);
        thread::sleep(Duration::from_millis(50));
        session.send(&Request::Ping);
        assert_eq!(session.next(), Event::Pong);
        let asked = Instant::now();
        session.send(&Request::Cancel { job: 1 });
        assert_eq!(session.next(), Event::Cancelled { job: 1 });
        assert!(asked.elapsed() < Duration::from_millis(500));
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_cancel_settles_a_queued_job_without_waiting_for_the_load_in_front() {
        let mut session = Session::start(0);
        session.send(&Request::LoadModel {
            path: "/models/ggml-slow.bin".into(),
            gpu: false,
            threads: 2,
        });
        assert!(matches!(session.next(), Event::Loading { .. }));
        session.transcribe(1, None);
        let asked = Instant::now();
        session.send(&Request::Cancel { job: 1 });
        assert_eq!(session.next(), Event::Cancelled { job: 1 });
        assert!(
            asked.elapsed() < Duration::from_millis(200),
            "the cancel waited for the load"
        );
        assert!(matches!(session.next(), Event::Loaded { .. }));
        session.send(&Request::Ping);
        assert_eq!(
            session.next(),
            Event::Pong,
            "the cancelled job must not answer a second time"
        );
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn a_cancel_for_an_unknown_job_is_ignored() {
        let mut session = Session::start(0);
        session.send(&Request::Cancel { job: 404 });
        session.send(&Request::Ping);
        assert_eq!(session.next(), Event::Pong);
        assert_eq!(session.finish(), 0);
    }

    #[test]
    fn the_end_of_the_input_is_a_clean_exit() {
        let session = Session::start(0);
        drop(session.to_engine);
        assert_eq!(session.exit.join().unwrap(), 0);
    }

    #[test]
    fn an_oversized_frame_ends_the_child_with_the_bad_frame_code() {
        let mut session = Session::start(0);
        let mut header = u32::MAX.to_le_bytes().to_vec();
        header.push(0);
        session.to_engine.write_all(&header).unwrap();
        assert_eq!(session.exit.join().unwrap(), EXIT_BAD_FRAME);
    }

    #[test]
    fn model_names_drop_the_ggml_prefix_and_the_extension() {
        assert_eq!(model_name(Path::new("/m/ggml-tiny.en.bin")), "tiny.en");
        assert_eq!(
            model_name(Path::new("/m/ggml-large-v3-turbo.bin")),
            "large-v3-turbo"
        );
        assert_eq!(model_name(Path::new("/m/custom.bin")), "custom");
    }
}
