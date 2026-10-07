//! The real `hushpen engine` child and the supervisor around it, with a real model.
//!
//! The child is the `hushpen` binary of the workspace; the tests build it once with cargo
//! (`HUSHPEN_BIN` names another build). Models and WAVs come from the helpers in `it.rs`.

use super::{assert_expected_words, model_path, short_wav, words};
use hushpen_core::error;
use hushpen_core::protocol::{
    Event, FrameKind, PROTOCOL_VERSION, Request, read_frame, write_frame, write_json,
};
use hushpen_engine::{
    ChildSpec, EngineClient, EngineLog, EngineState, Failure, JobOutcome, LoadSpec, Timing,
    TranscribeSpec,
};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

fn hushpen_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("HUSHPEN_BIN") {
        return path.into();
    }
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let exe = std::env::current_exe().expect("test exe");
            let profile_dir = exe
                .parent()
                .and_then(Path::parent)
                .expect("target profile dir")
                .to_path_buf();
            let mut build =
                Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
            build.args(["build", "--locked", "-p", "hushpen-app", "--bin", "hushpen"]);
            if profile_dir
                .file_name()
                .is_some_and(|name| name == "release")
            {
                build.arg("--release");
            }
            let status = build.status().expect("run cargo build");
            assert!(status.success(), "could not build the hushpen binary");
            profile_dir.join("hushpen")
        })
        .clone()
}

fn engine_spec() -> ChildSpec {
    ChildSpec {
        program: hushpen_binary(),
        args: vec!["engine".into()],
    }
}

fn fast_timing() -> Timing {
    Timing {
        backoff: vec![Duration::from_millis(200), Duration::from_millis(400)],
        ..Timing::default()
    }
}

fn load_spec(path: &Path) -> LoadSpec {
    LoadSpec {
        path: path.to_path_buf(),
        gpu: false,
        threads: 2,
    }
}

fn english(wav: &Path) -> TranscribeSpec {
    TranscribeSpec {
        wav_path: wav.to_path_buf(),
        language: Some("en".into()),
        prompt: None,
    }
}

struct Started {
    client: EngineClient,
    dir: tempfile::TempDir,
    log_path: PathBuf,
}

fn start_client(timing: Timing) -> Started {
    let dir = tempfile::tempdir().expect("temp dir");
    let log_path = dir.path().join("logs/engine.log");
    let client = EngineClient::start(engine_spec(), timing, EngineLog::open(&log_path).unwrap());
    assert!(
        client.wait_for(Duration::from_secs(20), |s| s.state == EngineState::Ready),
        "the engine did not become ready"
    );
    Started {
        client,
        dir,
        log_path,
    }
}

fn expect_done(outcome: JobOutcome) -> String {
    match outcome {
        JobOutcome::Done(done) => done.text,
        other => panic!("expected a transcript, got {other:?}"),
    }
}

fn job_lines(log_path: &Path) -> Vec<String> {
    std::fs::read_to_string(log_path)
        .unwrap()
        .lines()
        .filter(|line| line.contains(" JOB "))
        .map(str::to_owned)
        .collect()
}

fn signal(signal: &str, pid: u32) {
    let status = Command::new("kill")
        .args([signal, &pid.to_string()])
        .status()
        .expect("run kill");
    assert!(status.success(), "kill {signal} {pid}");
}

fn process_is_gone(pid: u32) -> bool {
    !Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

// ---- the raw frame protocol (VAL-ENG-007, VAL-ENG-008) ----

struct Raw {
    child: Child,
    to: ChildStdin,
    from: BufReader<ChildStdout>,
}

impl Raw {
    fn start() -> Self {
        let mut child = Command::new(hushpen_binary())
            .arg("engine")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start hushpen engine");
        let to = child.stdin.take().unwrap();
        let from = BufReader::new(child.stdout.take().unwrap());
        Self { child, to, from }
    }

    fn send(&mut self, request: &Request) {
        write_json(&mut self.to, &request.to_json()).expect("write request");
    }

    fn next(&mut self) -> Event {
        let frame = read_frame(&mut self.from)
            .expect("read frame")
            .expect("the engine closed its output");
        Event::from_frame(&frame).expect("a known event")
    }

    /// Reads events until a `Result`, `Error`, or `Cancelled` for `job`.
    fn finish_job(&mut self, job: u64) -> (Vec<hushpen_core::protocol::Segment>, Event) {
        let mut streamed = Vec::new();
        loop {
            match self.next() {
                Event::Segment { job: j, segment } if j == job => streamed.push(segment),
                event @ (Event::Result { .. } | Event::Error { .. } | Event::Cancelled { .. }) => {
                    return (streamed, event);
                }
                _ => {}
            }
        }
    }

    fn load(&mut self, model: &Path) {
        self.send(&Request::LoadModel {
            path: model.to_string_lossy().into_owned(),
            gpu: false,
            threads: 2,
        });
        assert!(matches!(self.next(), Event::Loading { .. }));
        assert!(matches!(self.next(), Event::Loaded { .. }));
    }

    fn transcribe(&mut self, job: u64, language: &str) {
        self.send(&Request::Transcribe {
            job,
            wav_path: short_wav().to_string_lossy().into_owned(),
            language: Some(language.into()),
            prompt: None,
        });
    }
}

#[test]
fn the_engine_child_speaks_the_frame_protocol() {
    let mut engine = Raw::start();
    engine.send(&Request::Hello {
        protocol: PROTOCOL_VERSION,
    });
    match engine.next() {
        Event::Ready {
            version, protocol, ..
        } => {
            assert!(!version.is_empty());
            assert_eq!(protocol, PROTOCOL_VERSION);
        }
        other => panic!("expected Ready, got {other:?}"),
    }

    engine.load(&model_path());
    engine.transcribe(1, "en");
    let (streamed, result) = engine.finish_job(1);
    let Event::Result {
        text,
        language,
        segments,
        ..
    } = result
    else {
        panic!("expected a result, got {result:?}");
    };
    assert_expected_words(&text);
    assert_eq!(language, "en");
    assert!(!segments.is_empty());
    assert_eq!(streamed, segments, "segments stream before the result");
    let mut last_start = 0;
    for segment in &segments {
        assert!(segment.start_ms < segment.end_ms, "{segment:?}");
        assert!(segment.start_ms >= last_start, "{segment:?}");
        last_start = segment.start_ms;
    }

    engine.send(&Request::Ping);
    assert_eq!(engine.next(), Event::Pong);

    write_json(&mut engine.to, &serde_json::json!({"type": "teleport"})).unwrap();
    match engine.next() {
        Event::Error {
            job: None, code, ..
        } => assert_eq!(code, error::ENGINE_PROTOCOL),
        other => panic!("expected a coded error, got {other:?}"),
    }
    write_frame(&mut engine.to, FrameKind::Json, b"{not json").unwrap();
    assert!(matches!(engine.next(), Event::Error { .. }));
    engine.send(&Request::Ping);
    assert_eq!(
        engine.next(),
        Event::Pong,
        "the engine survives bad requests"
    );

    let asked = Instant::now();
    engine.send(&Request::Shutdown);
    let status = engine.child.wait().expect("wait");
    assert_eq!(status.code(), Some(0));
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "{:?}",
        asked.elapsed()
    );
}

#[test]
fn a_wrong_language_code_is_a_coded_error_and_not_a_crash() {
    let mut engine = Raw::start();
    let pid = engine.child.id();
    engine.send(&Request::Hello {
        protocol: PROTOCOL_VERSION,
    });
    assert!(matches!(engine.next(), Event::Ready { .. }));
    engine.load(&model_path());

    engine.transcribe(1, "xx");
    match engine.finish_job(1).1 {
        Event::Error {
            job: Some(1), code, ..
        } => assert_eq!(code, error::ENGINE_BAD_LANGUAGE),
        other => panic!("expected ENGINE_BAD_LANGUAGE, got {other:?}"),
    }
    assert_eq!(engine.child.id(), pid);
    assert!(
        engine.child.try_wait().unwrap().is_none(),
        "the engine is alive"
    );

    engine.transcribe(2, "en");
    match engine.finish_job(2).1 {
        Event::Result { text, .. } => assert_expected_words(&text),
        other => panic!("expected a result, got {other:?}"),
    }
    engine.send(&Request::Shutdown);
    assert_eq!(engine.child.wait().unwrap().code(), Some(0));
}

// ---- the supervised client ----

fn asset_model(file: &str) -> Option<PathBuf> {
    let path = std::env::var_os("HUSHPEN_TEST_ASSETS")
        .map(|dir| PathBuf::from(dir).join("models/whisper").join(file))?;
    path.exists().then_some(path)
}

#[test]
fn a_model_change_takes_effect_in_the_same_session_and_a_bad_model_keeps_the_old_one() {
    let Started {
        client,
        dir,
        log_path,
    } = start_client(Timing::default());
    let tiny = model_path();
    // Another model where the assets have one, else the same weights under another name.
    let other = asset_model("ggml-base.en.bin").unwrap_or_else(|| {
        let copy = dir.path().join("ggml-tiny-copy.en.bin");
        std::fs::copy(&tiny, &copy).unwrap();
        copy
    });
    let pid = client.status().pid;

    // The state reads loading while a load runs, and ready afterwards.
    let seen_loading = Arc::new(Mutex::new(false));
    let watcher = {
        let (client, seen) = (client.clone(), Arc::clone(&seen_loading));
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            while Instant::now() < deadline {
                let status = client.status();
                if status.state == EngineState::Loading {
                    *seen.lock().unwrap() = true;
                }
                if status.model.is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        })
    };
    let loaded = client
        .load_model(load_spec(&tiny))
        .wait()
        .expect("load tiny");
    watcher.join().unwrap();
    assert!(*seen_loading.lock().unwrap(), "no loading state was seen");
    assert_eq!(client.status().state, EngineState::Ready);
    assert_eq!(
        client.status().model.as_deref(),
        Some(loaded.model.as_str())
    );
    assert_expected_words(&expect_done(
        client.transcribe(english(&short_wav())).wait(),
    ));

    let second = client
        .load_model(load_spec(&other))
        .wait()
        .expect("load second");
    assert_ne!(second.model, loaded.model);
    assert_expected_words(&expect_done(
        client.transcribe(english(&short_wav())).wait(),
    ));
    assert_eq!(client.status().pid, pid, "the engine was not restarted");

    let truncated = dir.path().join("ggml-base.en.bin");
    let bytes = std::fs::read(&other).unwrap();
    std::fs::write(&truncated, &bytes[..bytes.len() / 4]).unwrap();
    let failure = client.load_model(load_spec(&truncated)).wait().unwrap_err();
    assert_eq!(failure.code, error::ENGINE_LOAD_FAILED);
    assert_eq!(client.status().state, EngineState::Ready);
    assert_eq!(
        client.status().model.as_deref(),
        Some(second.model.as_str())
    );
    assert_expected_words(&expect_done(
        client.transcribe(english(&short_wav())).wait(),
    ));
    assert_eq!(
        client.status().pid,
        pid,
        "a bad model does not kill the engine"
    );
    client.shutdown();

    let lines = job_lines(&log_path);
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert!(
        lines[0].contains(&format!("model={}", loaded.model)),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(&format!("model={}", second.model)),
        "{}",
        lines[1]
    );
    assert!(
        lines[2].contains(&format!("model={}", second.model)),
        "{}",
        lines[2]
    );
}

/// A 10 minute WAV: the shared fixture, else the short clip repeated.
fn long_wav(dir: &Path) -> PathBuf {
    if let Some(shared) = std::env::var_os("HUSHPEN_TEST_ASSETS")
        .map(|assets| PathBuf::from(assets).join("fixtures/long-600s.wav"))
        .filter(|path| path.exists())
    {
        return shared;
    }
    let mut reader = hound::WavReader::open(short_wav()).unwrap();
    let spec = reader.spec();
    let clip: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
    let path = dir.join("long-600s.wav");
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for sample in clip.iter().copied().cycle().take(16_000 * 600) {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();
    path
}

/// A small deterministic generator, so a failing run can be repeated.
struct Lcg(u64);

impl Lcg {
    fn next_below(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

#[test]
fn cancel_settles_in_under_a_second_also_when_the_engine_hangs() {
    let Started { client, dir, .. } = start_client(Timing::default());
    client
        .load_model(load_spec(&model_path()))
        .wait()
        .expect("load");
    let long = long_wav(dir.path());
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos() as u64);
    eprintln!("cancel timing seed {seed}");
    let mut random = Lcg(seed);

    let mut latencies = Vec::new();
    for round in 0..20 {
        let job = client.transcribe(english(&long));
        std::thread::sleep(Duration::from_millis(random.next_below(3000)));
        let asked = Instant::now();
        job.cancel();
        let outcome = job.wait();
        let latency = asked.elapsed();
        assert_eq!(outcome, JobOutcome::Cancelled, "round {round}");
        latencies.push(latency);
    }
    let worst = latencies.iter().max().copied().unwrap_or_default();
    eprintln!("20 cancel latencies: {latencies:?}; max {worst:?}");
    assert!(
        worst < Duration::from_secs(1),
        "slowest cancel took {worst:?}"
    );

    // A stopped engine cannot answer a cancel; the supervisor kills it and starts a new one.
    let stopped_pid = client.status().pid.expect("a running engine");
    let job = client.transcribe(english(&long));
    std::thread::sleep(Duration::from_millis(300));
    signal("-STOP", stopped_pid);
    let asked = Instant::now();
    job.cancel();
    assert_eq!(job.wait(), JobOutcome::Cancelled);
    let latency = asked.elapsed();
    eprintln!("cancel of a stopped engine: {latency:?}");
    assert!(latency < Duration::from_secs(1), "took {latency:?}");
    assert!(
        client.wait_for(Duration::from_secs(20), |s| s.state == EngineState::Ready
            && s.pid.is_some_and(|pid| pid != stopped_pid)
            && s.model.is_some()),
        "no new engine came up"
    );
    assert!(
        process_is_gone(stopped_pid),
        "the stopped engine is still alive"
    );
    assert_expected_words(&expect_done(
        client.transcribe(english(&short_wav())).wait(),
    ));
    client.shutdown();
}

#[test]
fn killing_the_engine_mid_job_fails_the_job_keeps_the_wav_and_the_next_job_works() {
    let Started {
        client,
        dir,
        log_path,
    } = start_client(fast_timing());
    client
        .load_model(load_spec(&model_path()))
        .wait()
        .expect("load");
    let wav = long_wav(dir.path());
    let before = std::fs::metadata(&wav).unwrap().len();
    let first_pid = client.status().pid.expect("pid");

    let job = client.transcribe(english(&wav));
    std::thread::sleep(Duration::from_millis(500));
    signal("-KILL", first_pid);
    match job.wait() {
        JobOutcome::Failed(Failure { code, .. }) => assert_eq!(code, error::ENGINE_CRASHED),
        other => panic!("expected ENGINE_CRASHED, got {other:?}"),
    }
    assert_eq!(
        std::fs::metadata(&wav).unwrap().len(),
        before,
        "the WAV is kept"
    );

    // A job submitted while the engine restarts waits for it instead of failing.
    let next = client.transcribe(english(&short_wav()));
    assert_expected_words(&expect_done(next.wait()));
    let status = client.status();
    assert_ne!(status.pid, Some(first_pid));
    assert_eq!(status.restarts, 1);
    assert!(status.model.is_some(), "the model was loaded again");
    client.shutdown();

    let text = std::fs::read_to_string(log_path).unwrap();
    assert!(text.contains(" CRASH "), "{text}");
    assert!(
        text.contains(&format!("code={}", error::ENGINE_CRASHED)),
        "{text}"
    );
}

#[test]
fn engine_log_has_one_line_per_job_with_model_threads_timings_and_code_and_no_transcript() {
    let Started {
        client,
        log_path,
        dir: _dir,
    } = start_client(Timing::default());
    client
        .load_model(load_spec(&model_path()))
        .wait()
        .expect("load");
    let text = expect_done(client.transcribe(english(&short_wav())).wait());
    assert_expected_words(&text);
    let wrong = client.transcribe(TranscribeSpec {
        language: Some("xx".into()),
        ..english(&short_wav())
    });
    assert!(matches!(wrong.wait(), JobOutcome::Failed(_)));
    client
        .load_model(LoadSpec {
            threads: 3,
            ..load_spec(&model_path())
        })
        .wait()
        .expect("reload");
    expect_done(client.transcribe(english(&short_wav())).wait());
    client.shutdown();

    let lines = job_lines(&log_path);
    assert_eq!(lines.len(), 3, "{lines:#?}");
    assert!(
        lines[0].contains("threads=2") && lines[0].contains("code=OK"),
        "{}",
        lines[0]
    );
    assert!(
        lines[0].contains("decode_ms=") && lines[0].contains("total_ms="),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(error::ENGINE_BAD_LANGUAGE),
        "{}",
        lines[1]
    );
    assert!(lines[2].contains("threads=3"), "{}", lines[2]);
    let log = std::fs::read_to_string(&log_path).unwrap().to_lowercase();
    let in_log: std::collections::HashSet<&str> = log
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect();
    for word in words(&text).split(' ').filter(|word| word.len() > 3) {
        assert!(
            !in_log.contains(word),
            "the log holds the transcript word '{word}'"
        );
    }
}
