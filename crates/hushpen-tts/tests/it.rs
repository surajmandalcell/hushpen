//! Heavy integration tests: the real `hushpen-tts` process with the real Kokoro model and
//! sherpa-onnx library.
//!
//! The model folder is `HUSHPEN_TEST_TTS_DIR`, or `$HUSHPEN_TEST_ASSETS/models/tts/<model id>`.
//! The library folder is `HUSHPEN_TTS_RUNTIME_DIR` (see `cargo xtask tts-runtime`).
//! `HUSHPEN_TEST_THREADS` sets the thread count (default 2).
#![cfg(feature = "heavy")]

use hushpen_core::kokoro::{LEXICON_FILE, MODEL_ID, SAMPLE_RATE};
use hushpen_core::protocol::write_json;
use hushpen_core::tts_protocol::{TTS_PROTOCOL_VERSION, TtsEvent, TtsRequest, read_event};
use hushpen_tts::phonemes::Lexicon;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const EVENT_WAIT: Duration = Duration::from_secs(60);
const NUMBERS: &str = "Hushpen 2.0 costs $45.99 on 12 March 2025 at 3:30. Dr. Smith's iPhone \
runs macOS (GitHub's API), 50% off! The café opens on the 21st.";

fn model_dir() -> PathBuf {
    let dir = std::env::var_os("HUSHPEN_TEST_TTS_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HUSHPEN_TEST_ASSETS")
                .map(|root| PathBuf::from(root).join("models/tts").join(MODEL_ID))
        })
        .unwrap_or_else(|| panic!("set HUSHPEN_TEST_TTS_DIR to the Kokoro model folder"));
    assert!(
        dir.join(LEXICON_FILE).is_file(),
        "{} has no model",
        dir.display()
    );
    dir
}

fn threads() -> u32 {
    std::env::var("HUSHPEN_TEST_THREADS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2)
}

struct Worker {
    child: Child,
    stdin: ChildStdin,
    events: Receiver<TtsEvent>,
    stderr: Receiver<String>,
}

impl Worker {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_hushpen-tts"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start hushpen-tts");
        let stdin = child.stdin.take().expect("stdin");
        let mut stdout = child.stdout.take().expect("stdout");
        let stderr_pipe = child.stderr.take().expect("stderr");
        let (event_tx, events) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(Some(event)) = read_event(&mut stdout) {
                if event_tx.send(event).is_err() {
                    return;
                }
            }
        });
        let (stderr_tx, stderr) = mpsc::channel();
        thread::spawn(move || {
            for line in std::io::BufReader::new(stderr_pipe)
                .lines()
                .map_while(Result::ok)
            {
                let _ = stderr_tx.send(line);
            }
        });
        let mut worker = Self {
            child,
            stdin,
            events,
            stderr,
        };
        worker.send(&TtsRequest::Hello {
            protocol: TTS_PROTOCOL_VERSION,
        });
        assert!(matches!(worker.next(), TtsEvent::Ready { .. }));
        worker.send(&TtsRequest::LoadModel {
            dir: model_dir().to_string_lossy().into_owned(),
            threads: threads(),
            idle_secs: 300,
        });
        assert!(matches!(worker.next(), TtsEvent::Loading { .. }));
        match worker.next() {
            TtsEvent::Loaded { model, ms } => {
                assert_eq!(Some(model.as_ref()), model_dir().file_name());
                eprintln!("load_ms={ms}");
            }
            other => panic!("expected loaded, got {other:?}"),
        }
        worker
    }

    fn send(&mut self, request: &TtsRequest) {
        write_json(&mut self.stdin, &request.to_json()).expect("write a request");
    }

    fn next(&mut self) -> TtsEvent {
        self.events
            .recv_timeout(EVENT_WAIT)
            .expect("an event from the worker")
    }

    fn speak(&mut self, job: u64, text: &str, voice: &str) {
        self.send(&TtsRequest::Speak {
            job,
            text: text.into(),
            voice: Some(voice.into()),
            speed: 1.0,
        });
    }

    /// What the library wrote to stderr, apart from the notice ONNX Runtime prints for an ARM
    /// CPU it does not know, such as a Linux VM on Apple silicon.
    fn stderr_lines(&self) -> Vec<String> {
        self.stderr
            .try_iter()
            .filter(|line| !line.starts_with("onnxruntime cpuid_info warning"))
            .collect()
    }

    fn finish(mut self) {
        self.send(&TtsRequest::Shutdown);
        let status = self.child.wait().expect("the worker exits");
        assert!(status.success(), "{status}");
    }
}

#[test]
fn the_real_lexicon_reads_numbers_names_and_endings() {
    let lexicon = Lexicon::read(&model_dir().join(LEXICON_FILE)).expect("the lexicon");
    assert_eq!(
        lexicon.sentences(NUMBERS),
        [
            "hˈʌʃpˈɛn tˈu pˈYnt zˈɪɹO kˈɔsts fˈɔɹɾi fˈIv dˈɑləɹz ænd nˈIndi nˈIn sˈɛnts ˌɔn \
             twˈɛlv mˈɑɹʧ twˈɛnti twˈɛnti fˈIv æt θɹˈi θˈɜɹɾi.",
            "dˈɑktəɹ. smˈɪθs ˈI fˈOn ɹˈʌnz mˈæk ˈO ˈɛs ɡˈɪt hˈʌbz A pˈi ˈI, fˈɪfti pəɹsˈɛnt ˈɔf!",
            "ðə kæfˈA ˈOpᵊnz ˌɔn ðə twˈɛnti fˈɜɹst.",
        ]
    );
    assert_eq!(
        lexicon
            .sentences("The 70th and 100th runners opened 3 boxes, waiting for €2 and £1,000,000."),
        [
            "ðə sˈɛvəntiəθ ænd wˈʌn hˈʌndɹədθ ɹˈʌnəɹz ˈOpᵊnd θɹˈi bˈɑksᵻz, wˈAɾɪŋ fɔɹ tˈu jˈʊɹOz \
             ænd wˈʌn mˈɪljᵊn pˈWndz."
        ]
    );
}

#[test]
fn the_worker_speaks_every_word_and_streams_the_audio() {
    let mut worker = Worker::start();
    for (job, voice) in [(1, "af_heart"), (2, "bm_george")] {
        let started = Instant::now();
        worker.speak(job, NUMBERS, voice);
        let mut samples = Vec::new();
        let mut pieces = 0;
        let stats = loop {
            match worker.next() {
                TtsEvent::Audio {
                    job: from,
                    sample_rate,
                    samples: piece,
                } => {
                    assert_eq!((from, sample_rate), (job, SAMPLE_RATE));
                    samples.extend(piece);
                    pieces += 1;
                }
                TtsEvent::Done { job: from, stats } => {
                    assert_eq!(from, job);
                    break stats;
                }
                other => panic!("expected audio, got {other:?}"),
            }
        };
        let seconds = samples.len() as f64 / f64::from(SAMPLE_RATE);
        let peak = samples
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        let rtf = started.elapsed().as_secs_f64() / seconds;
        eprintln!("{voice}: {seconds:.1} s of audio in {pieces} pieces, rtf={rtf:.3}, {stats:?}");
        assert!(seconds > 5.0, "{seconds} s");
        assert!(peak > 0.05, "peak {peak}");
        assert!(pieces > 1, "the two sentences come as two pieces");
        assert!(stats.first_audio_ms < stats.ms);
    }
    let logged = worker.stderr_lines();
    assert!(logged.is_empty(), "the library logged: {logged:?}");
    worker.finish();
}

#[test]
fn the_runtime_check_loads_the_bundled_library() {
    let output = Command::new(env!("CARGO_BIN_EXE_hushpen-tts"))
        .arg("--check-runtime")
        .output()
        .expect("run hushpen-tts --check-runtime");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "sherpa-onnx 2.0.0"
    );
}

#[test]
fn cancel_stops_a_long_job_after_its_first_audio() {
    let mut worker = Worker::start();
    let text = "This is the first sentence of a long text. ".repeat(12);
    worker.speak(7, &text, "af_heart");
    assert!(matches!(worker.next(), TtsEvent::Audio { job: 7, .. }));
    let started = Instant::now();
    worker.send(&TtsRequest::Cancel { job: 7 });
    loop {
        match worker.next() {
            TtsEvent::Audio { job: 7, .. } => {}
            TtsEvent::Cancelled { job: 7 } => break,
            other => panic!("expected the cancel, got {other:?}"),
        }
    }
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "cancel took {:?}",
        started.elapsed()
    );
    worker.send(&TtsRequest::Ping);
    assert_eq!(worker.next(), TtsEvent::Pong);
    worker.finish();
}
