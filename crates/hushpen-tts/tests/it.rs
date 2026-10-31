//! Heavy integration tests: the real `hushpen-tts` process with the real Kokoro and Chatterbox
//! models, sherpa-onnx, and ONNX Runtime.
//!
//! The Kokoro folder is `HUSHPEN_TEST_TTS_DIR`, or `$HUSHPEN_TEST_ASSETS/models/tts/<model id>`;
//! the Chatterbox folders are `$HUSHPEN_TEST_ASSETS/models/tts/chatterbox-turbo` and
//! `chatterbox-nano`. The library folder is `HUSHPEN_TTS_RUNTIME_DIR` (see
//! `cargo xtask tts-runtime`).
//! `HUSHPEN_TEST_THREADS` sets the thread count (default 2). With `HUSHPEN_TEST_WAV_DIR` the mix
//! test writes the audio of its two voices and their mix there.
#![cfg(feature = "heavy")]

use hushpen_core::chatterbox::{self, DEFAULT_VOICE_FILE, NANO_ID, TOKENIZER_FILE, TURBO_ID};
use hushpen_core::error;
use hushpen_core::kokoro::{
    FILES, LEXICON_FILE, MIX_DIR, MODEL_ID, SAMPLE_RATE, VOICES_BYTES, VoiceMix,
};
use hushpen_core::protocol::write_json;
use hushpen_core::tts_protocol::{
    TTS_PROTOCOL_VERSION, TtsEngine, TtsEvent, TtsRequest, read_event,
};
use hushpen_tts::chatterbox::tokenizer::Tokenizer;
use hushpen_tts::phonemes::Lexicon;
use std::io::BufRead;
use std::path::{Path, PathBuf};
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

fn chatterbox_dir(id: &str) -> PathBuf {
    let root = std::env::var_os("HUSHPEN_TEST_ASSETS")
        .unwrap_or_else(|| panic!("set HUSHPEN_TEST_ASSETS to the test assets folder"));
    let dir = PathBuf::from(root).join("models/tts").join(id);
    assert!(
        chatterbox::FILES
            .iter()
            .all(|file| dir.join(file).is_file()),
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
        Self::start_with(TtsEngine::Kokoro, &model_dir())
    }

    fn start_with(engine: TtsEngine, dir: &Path) -> Self {
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
            dir: dir.to_string_lossy().into_owned(),
            engine,
            threads: threads(),
            idle_secs: 300,
        });
        assert!(matches!(worker.next(), TtsEvent::Loading { .. }));
        match worker.next() {
            TtsEvent::Loaded { model, ms } => {
                assert_eq!(Some(model.as_ref()), dir.file_name());
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
        self.speak_with(job, text, Some(voice));
    }

    fn speak_with(&mut self, job: u64, text: &str, voice: Option<&str>) {
        self.send(&TtsRequest::Speak {
            job,
            text: text.into(),
            voice: voice.map(str::to_owned),
            speed: 1.0,
        });
    }

    /// The audio of `job` up to its end, and the end.
    fn spoken(&mut self, job: u64) -> (Vec<f32>, TtsEvent) {
        let mut samples = Vec::new();
        loop {
            match self.next() {
                TtsEvent::Audio {
                    job: from,
                    samples: piece,
                    ..
                } if from == job => samples.extend(piece),
                end => return (samples, end),
            }
        }
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
fn kokoro_short_texts_stay_finite_at_every_speed() {
    let mut worker = Worker::start();
    let mut job = 100;
    for voice in ["af_heart", "bm_george"] {
        for speed in [0.5, 1.0, 1.5, 2.0] {
            for text in ["Hello.", "Good morning."] {
                job += 1;
                worker.send(&TtsRequest::Speak {
                    job,
                    text: text.to_owned(),
                    voice: Some(voice.to_owned()),
                    speed,
                });
                let (samples, end) = worker.spoken(job);
                assert!(matches!(end, TtsEvent::Done { .. }), "{end:?}");
                assert!(
                    !samples.is_empty() && samples.iter().all(|sample| sample.is_finite()),
                    "{voice} at {speed}: audio must contain only finite samples"
                );
                assert!(
                    samples.iter().any(|sample| sample.abs() > 0.01),
                    "{voice} at {speed}: audio must not be silent"
                );
            }
        }
    }
    worker.finish();
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
        "sherpa-onnx 2.0.0\nonnxruntime 1.28.2"
    );
}

/// A model folder of links to the real files, so a mix file goes in a scratch `mixes` folder
/// next to it and never in the test assets.
fn linked_model(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("hushpen-tts-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join(MODEL_ID);
    std::fs::create_dir_all(&dir).unwrap();
    for file in FILES {
        std::os::unix::fs::symlink(model_dir().join(file), dir.join(file)).unwrap();
    }
    dir
}

#[test]
fn a_mix_of_two_voices_speaks_and_the_plain_voices_still_do() {
    let dir = linked_model("mix");
    let mut worker = Worker::start_with(TtsEngine::Kokoro, &dir);
    let mix = VoiceMix::new("af_heart", "am_adam", 50).unwrap();
    let text = "The quiet river carried seven paper boats past the old mill.";
    let mut spoken = Vec::new();
    for (job, voice) in [
        (1, "af_heart"),
        (2, "am_adam"),
        (3, mix.id().as_str()),
        (4, "af_heart"),
    ] {
        worker.speak(job, text, voice);
        let (samples, end) = worker.spoken(job);
        assert!(matches!(end, TtsEvent::Done { .. }), "{voice}: {end:?}");
        assert!(samples.len() > SAMPLE_RATE as usize, "{voice}: too short");
        assert!(samples.iter().all(|sample| sample.is_finite()), "{voice}");
        assert!(samples.iter().any(|sample| sample.abs() > 0.05), "{voice}");
        spoken.push(samples);
    }
    if let Some(out) = std::env::var_os("HUSHPEN_TEST_WAV_DIR") {
        let out = PathBuf::from(out);
        for (name, samples) in ["a-af_heart", "b-am_adam", "mix-50"].iter().zip(&spoken) {
            write_wav(&out.join(format!("{name}.wav")), samples);
        }
    }
    let file = dir.parent().unwrap().join(MIX_DIR).join(mix.file_name());
    assert_eq!(std::fs::metadata(&file).unwrap().len(), VOICES_BYTES as u64);
    // The vocoder adds noise, so two runs of one voice differ in their samples; the length
    // comes from the style vectors alone.
    assert_eq!(
        spoken[3].len(),
        spoken[0].len(),
        "a plain voice keeps its timing in the mix file"
    );
    worker.send(&TtsRequest::Speak {
        job: 5,
        text: text.into(),
        voice: Some("af_heart:50+af_heart:50".into()),
        speed: 1.0,
    });
    match worker.next() {
        TtsEvent::Error { job, code, .. } => {
            assert_eq!((job, code.as_str()), (Some(5), error::SPEECH_VOICE_MISSING));
        }
        other => panic!("expected the refusal, got {other:?}"),
    }
    worker.finish();
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(path, spec).expect("create the WAV");
    for sample in samples {
        wav.write_sample(*sample).expect("write a sample");
    }
    wav.finalize().expect("finish the WAV");
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

/// Ids from the Hugging Face tokenizer of the models (`tokenizers` 0.22) for the same texts.
const REFERENCE_IDS: [(&str, &[u32]); 10] = [
    ("Hello world.", &[15496, 995, 13, 50256, 50256]),
    (
        "Oh, that's hilarious! [chuckle] Um anyway, we do have a new model in store.",
        &[
            5812, 11, 326, 338, 20105, 0, 220, 50274, 21039, 6949, 11, 356, 466, 423, 257, 649,
            2746, 287, 3650, 13, 50256, 50256,
        ],
    ),
    (
        "It costs $5.20 \u{2014} really?  Yes\u{2026}",
        &[
            1026, 3484, 720, 20, 13, 1238, 851, 1107, 30, 220, 3363, 1399, 50256, 50256,
        ],
    ),
    (
        "Don't stop; we'll see you're right, they've said I'm sure it'd work.",
        &[
            3987, 470, 2245, 26, 356, 1183, 766, 345, 821, 826, 11, 484, 1053, 531, 314, 1101,
            1654, 340, 1549, 670, 13, 50256, 50256,
        ],
    ),
    (
        "Line one\nLine two\n\n  indented   spaces",
        &[
            13949, 530, 198, 13949, 734, 628, 220, 773, 4714, 220, 220, 9029, 50256, 50256,
        ],
    ),
    (
        "Caf\u{e9} na\u{ef}ve fa\u{e7}ade r\u{e9}sum\u{e9} \u{65e5}\u{672c}\u{8a9e} emoji \u{1f642} done",
        &[
            34, 1878, 2634, 41492, 24685, 16175, 671, 40560, 16345, 2634, 10545, 245, 98, 17312,
            105, 45739, 252, 44805, 32485, 1760, 50256, 50256,
        ],
    ),
    (
        "[laugh][sigh] tags back to back [clear throat] and [unknown] tag",
        &[
            50275, 50268, 15940, 736, 284, 736, 220, 50267, 290, 685, 34680, 60, 7621, 50256, 50256,
        ],
    ),
    (
        "Numbers 1234567 and 3.14159, mixed abc123def.",
        &[
            49601, 17031, 2231, 3134, 290, 513, 13, 1415, 19707, 11, 7668, 450, 66, 10163, 4299,
            13, 50256, 50256,
        ],
    ),
    (
        "  leading and trailing  ",
        &[220, 3756, 290, 25462, 220, 220, 50256, 50256],
    ),
    ("", &[50256, 50256]),
];

#[test]
fn the_chatterbox_tokenizer_gives_the_reference_ids() {
    let path = chatterbox_dir(TURBO_ID).join(TOKENIZER_FILE);
    let mut tokenizer = Tokenizer::read(&path).expect("the tokenizer");
    for (text, ids) in REFERENCE_IDS {
        assert_eq!(tokenizer.encode(text), ids, "{text:?}");
    }
}

/// A copy of the model's own clip as 16 kHz 16-bit stereo, so the worker reads, resamples,
/// and mixes down a clip that is not in its own format.
fn foreign_clip(model: &Path, dir: &Path) -> PathBuf {
    let mut reader = hound::WavReader::open(model.join(DEFAULT_VOICE_FILE)).expect("the clip");
    let samples: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
    let path = dir.join("reference.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).expect("a clip file");
    for index in 0..samples.len() * 2 / 3 {
        let value = (samples[index * 3 / 2].clamp(-1.0, 1.0) * 32_000.0) as i16;
        writer.write_sample(value).unwrap();
        writer.write_sample(value).unwrap();
    }
    writer.finalize().unwrap();
    path
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hushpen-tts-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

const CHATTERBOX_TEXT: &str = "Hushpen reads your notes aloud on this computer. The meeting \
starts at 9:30, so please bring the report and don't be late.";

#[test]
fn chatterbox_reads_with_its_own_voice_and_with_a_cloned_one() {
    for id in [TURBO_ID, NANO_ID] {
        let dir = chatterbox_dir(id);
        let clips = scratch(id);
        let clip = foreign_clip(&dir, &clips);
        let mut worker = Worker::start_with(TtsEngine::Chatterbox, &dir);
        for (job, voice) in [(1, None), (2, Some(clip.to_string_lossy().into_owned()))] {
            let started = Instant::now();
            worker.speak_with(job, CHATTERBOX_TEXT, voice.as_deref());
            let (samples, end) = worker.spoken(job);
            let TtsEvent::Done { stats, .. } = end else {
                panic!("{id}: expected done, got {end:?}");
            };
            let seconds = samples.len() as f64 / 24_000.0;
            let peak = samples.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
            let rtf = started.elapsed().as_secs_f64() / seconds;
            eprintln!("{id} {voice:?}: {seconds:.1} s, rtf={rtf:.3}, {stats:?}");
            // 22 words read at 2 to 4 words a second, with the pauses.
            assert!((5.0..14.0).contains(&seconds), "{id}: {seconds} s");
            assert!(peak > 0.05 && peak <= 1.0, "{id}: peak {peak}");
            assert!(stats.first_audio_ms < stats.ms);
        }
        let cached = chatterbox::conditioning_path(&clip, id);
        assert!(
            cached.is_file(),
            "{id}: the clone is kept at {}",
            cached.display()
        );
        let logged = worker.stderr_lines();
        assert!(logged.is_empty(), "{id}: the runtime logged: {logged:?}");
        worker.finish();
        std::fs::remove_dir_all(&clips).unwrap();
    }
}

#[test]
fn chatterbox_refuses_other_languages_and_clips_it_cannot_clone() {
    let dir = chatterbox_dir(NANO_ID);
    let clips = scratch("bad-clips");
    let short = clips.join("short.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 24_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&short, spec).unwrap();
    for at in 0..12_000 {
        let value = (f32::sin(at as f32 * 0.05) * 9_000.0) as i16;
        writer.write_sample(value).unwrap();
    }
    writer.finalize().unwrap();
    let missing = clips.join("missing.wav");
    let mut worker = Worker::start_with(TtsEngine::Chatterbox, &dir);
    worker.speak_with(1, "Das ist nicht der Weg, und ich weiß es nicht.", None);
    let (_, end) = worker.spoken(1);
    assert!(
        matches!(&end, TtsEvent::Error { job: Some(1), code, .. } if code == error::SPEECH_NOT_ENGLISH),
        "{end:?}"
    );
    for (job, clip) in [(2, &short), (3, &missing)] {
        worker.speak_with(job, "Hello there.", Some(&clip.to_string_lossy()));
        let (_, end) = worker.spoken(job);
        assert!(
            matches!(&end, TtsEvent::Error { code, .. } if code == error::SPEECH_VOICE_CLIP),
            "{end:?}"
        );
    }
    worker.finish();
    std::fs::remove_dir_all(&clips).unwrap();
}

#[test]
fn cancel_stops_a_chatterbox_job_within_a_sentence() {
    let mut worker = Worker::start_with(TtsEngine::Chatterbox, &chatterbox_dir(TURBO_ID));
    let text = "This is one sentence of a long text that goes on and on for a while. ".repeat(8);
    worker.speak_with(4, &text, None);
    assert!(matches!(worker.next(), TtsEvent::Audio { job: 4, .. }));
    let started = Instant::now();
    worker.send(&TtsRequest::Cancel { job: 4 });
    let (_, end) = worker.spoken(4);
    assert_eq!(end, TtsEvent::Cancelled { job: 4 });
    // One decoder run at most, never the rest of the text.
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "cancel took {:?}",
        started.elapsed()
    );
    worker.send(&TtsRequest::Ping);
    assert_eq!(worker.next(), TtsEvent::Pong);
    worker.finish();
}

#[test]
fn one_worker_switches_from_kokoro_to_chatterbox_and_back() {
    let mut worker = Worker::start();
    let models = [
        (TtsEngine::Chatterbox, chatterbox_dir(NANO_ID), None),
        (TtsEngine::Kokoro, model_dir(), Some("af_heart")),
    ];
    for (job, (engine, dir, voice)) in (1..).zip(models) {
        worker.send(&TtsRequest::LoadModel {
            dir: dir.to_string_lossy().into_owned(),
            engine,
            threads: threads(),
            idle_secs: 300,
        });
        assert!(matches!(worker.next(), TtsEvent::Loading { .. }));
        assert!(
            matches!(worker.next(), TtsEvent::Loaded { .. }),
            "{engine:?}"
        );
        worker.speak_with(job, "Hello there, this is a test.", voice);
        let (samples, end) = worker.spoken(job);
        assert!(matches!(end, TtsEvent::Done { .. }), "{engine:?}: {end:?}");
        assert!(
            samples.len() > 12_000,
            "{engine:?}: {} samples",
            samples.len()
        );
    }
    worker.finish();
}
