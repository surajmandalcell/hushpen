//! The real Parakeet engine with the Parakeet Q4_K model.
//!
//! Input: `HUSHPEN_TEST_PARAKEET_MODEL`, or `HUSHPEN_TEST_ASSETS`
//! (`models/whisper/ggml-parakeet-tdt-0.6b-v3-q4_k.bin`).

use super::child::{Started, load_spec, start_client};
use super::{assert_expected_words, assets_dir, read_wav, short_wav, words};
use hushpen_core::export::{self, Format, Item};
use hushpen_engine::{
    AsrEngine, CancelFlag, EngineError, ImportSpec, JobOutcome, LoadOptions, ParakeetEngine,
    Timing, TranscribeOptions, TranscribeSpec,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn model() -> PathBuf {
    if let Some(path) = std::env::var_os("HUSHPEN_TEST_PARAKEET_MODEL") {
        return path.into();
    }
    let path = assets_dir()
        .expect("set HUSHPEN_TEST_PARAKEET_MODEL or HUSHPEN_TEST_ASSETS")
        .join("models/whisper/ggml-parakeet-tdt-0.6b-v3-q4_k.bin");
    assert!(path.exists(), "missing model {}", path.display());
    path
}

fn load(gpu: bool) -> ParakeetEngine {
    ParakeetEngine::load(&model(), LoadOptions { gpu, threads: 4 }).expect("load Parakeet")
}

fn speech() -> Vec<f32> {
    read_wav(&short_wav())
}

#[test]
fn parakeet_transcribes_and_ignores_the_dictionary_prompt() {
    let mut engine = load(cfg!(target_os = "macos"));
    assert_eq!(engine.gpu_in_use(), cfg!(target_os = "macos"));
    let pcm = speech();
    let first = engine
        .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
        .expect("transcribe");
    assert_expected_words(&first.text);
    assert_eq!(
        first.language, "und",
        "the C API names no detected language"
    );
    let biased = engine
        .transcribe(
            &pcm,
            &TranscribeOptions {
                language: Some("en".into()),
                prompt: Some("Zebrafinch. Quokka.".into()),
                translate: false,
            },
            &CancelFlag::new(),
        )
        .expect("transcribe with a prompt");
    assert_eq!(biased, first, "Parakeet must not read a Whisper prompt");
}

#[test]
fn parakeet_words_export_on_the_audio_clock() {
    let mut engine = load(false);
    let clip = speech();
    let mut pcm = vec![0.0; 8 * 16_000];
    pcm.extend(&clip);
    pcm.resize(pcm.len() + 10 * 16_000, 0.0);
    let second_ms = pcm.len() as u64 / 16;
    pcm.extend(&clip);
    let audio_ms = pcm.len() as u64 / 16;
    let result = engine
        .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
        .expect("transcribe");
    assert_eq!(
        words(&result.text).matches("the quick brown fox").count(),
        2
    );
    assert!(result.segments.len() >= 20, "one segment per word expected");
    assert!(
        result.segments[0].start_ms >= 7_000,
        "{:?}",
        result.segments[0]
    );
    let last = result.segments.last().unwrap();
    assert!(last.end_ms > second_ms + 3_000, "{last:?}");
    let mut previous = 0;
    for segment in &result.segments {
        assert!(segment.start_ms < segment.end_ms, "{segment:?}");
        assert!(segment.start_ms >= previous, "{segment:?}");
        assert!(segment.end_ms <= audio_ms, "{segment:?}");
        assert!(segment.text.starts_with(' ') && !segment.text.contains('\u{2581}'));
        previous = segment.start_ms;
    }
    let item = Item {
        final_text: Some(result.text.clone()),
        duration_ms: audio_ms as i64,
        segments: result
            .segments
            .iter()
            .map(|s| export::Segment {
                start_ms: s.start_ms as i64,
                end_ms: s.end_ms as i64,
                text: s.text.clone(),
            })
            .collect(),
        ..Item::default()
    };
    for format in [Format::Srt, Format::Vtt] {
        let output = export::render(format, std::slice::from_ref(&item));
        assert!(output.matches("-->").count() >= 2, "{format:?}: {output}");
        assert!(output.contains("quick"), "{format:?}: {output}");
        assert!(!output.contains('\u{2581}'), "{format:?}: {output}");
    }
}

#[test]
fn parakeet_refuses_what_it_cannot_do_and_finds_no_words_in_silence() {
    let mut engine = load(false);
    let pcm = speech();
    for (options, expected) in [
        (
            TranscribeOptions {
                translate: true,
                ..TranscribeOptions::default()
            },
            EngineError::TranslateUnsupported,
        ),
        (
            TranscribeOptions {
                language: Some("hi".into()),
                ..TranscribeOptions::default()
            },
            EngineError::BadLanguage("hi".into()),
        ),
    ] {
        assert_eq!(
            engine.transcribe(&pcm, &options, &CancelFlag::new()),
            Err(expected)
        );
    }
    let set = CancelFlag::new();
    set.cancel();
    assert_eq!(
        engine.transcribe(&pcm, &TranscribeOptions::default(), &set),
        Err(EngineError::Cancelled)
    );
    assert_eq!(
        engine.transcribe(&[], &TranscribeOptions::default(), &CancelFlag::new()),
        Err(EngineError::NoAudio)
    );
    for samples in [1, 800, 16_000, 30 * 16_000] {
        let result = engine
            .transcribe(
                &vec![0.0; samples],
                &TranscribeOptions::default(),
                &CancelFlag::new(),
            )
            .expect("transcribe silence");
        assert_eq!(result.text, "", "{samples} samples of silence");
        assert!(result.segments.is_empty());
    }
    let missing = ParakeetEngine::load(
        std::path::Path::new("/nonexistent/ggml-parakeet.bin"),
        LoadOptions {
            gpu: false,
            threads: 2,
        },
    );
    assert_eq!(
        missing.err().and_then(|error| error.code()),
        Some("ENGINE_LOAD_FAILED")
    );
}

/// The encoder cannot be stopped inside a native call, so this bounds one decoded span.
#[test]
fn parakeet_stops_a_long_cpu_job_within_one_second() {
    let mut engine = load(false);
    let clip = speech();
    let pcm: Vec<f32> = clip.iter().copied().cycle().take(120 * 16_000).collect();
    for delay_ms in [200_u64, 3_000] {
        let cancel = CancelFlag::new();
        let trigger = cancel.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let timer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(delay_ms));
            trigger.cancel();
            tx.send(Instant::now()).expect("send");
        });
        let result = engine.transcribe(&pcm, &TranscribeOptions::default(), &cancel);
        let returned = Instant::now();
        timer.join().expect("timer");
        let latency = returned.saturating_duration_since(rx.recv().expect("set time"));
        eprintln!("Parakeet abort at {delay_ms} ms: returned {latency:?} after the flag was set");
        assert_eq!(
            result,
            Err(EngineError::Cancelled),
            "abort at {delay_ms} ms"
        );
        assert!(latency < Duration::from_secs(1), "abort took {latency:?}");
    }
}

#[test]
fn a_paged_out_parakeet_model_gives_the_same_transcript() {
    let mut engine = load(false);
    let pcm = speech();
    let first = engine
        .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
        .expect("transcribe");
    let advised = engine.page_out().expect("page out");
    if cfg!(target_os = "linux") {
        let file = std::fs::metadata(model()).unwrap().len();
        assert!(
            advised >= file / 2,
            "advised {advised} of a {file} byte model"
        );
    } else {
        assert_eq!(advised, 0, "only Linux has MADV_PAGEOUT");
    }
    let again = engine
        .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
        .expect("transcribe after the page-out");
    assert_eq!(again.text, first.text);
}

/// The 45 s dictation is at least three spans; its last word must survive the last cut.
#[test]
fn parakeet_keeps_the_end_of_a_long_dictation() {
    let Some(path) = assets_dir().map(|dir| dir.join("fixtures/dictation-45s.wav")) else {
        eprintln!("skipped: HUSHPEN_TEST_ASSETS is not set");
        return;
    };
    let mut engine = load(cfg!(target_os = "macos"));
    let pcm = read_wav(&path);
    let last = pcm.iter().rposition(|s| s.abs() > 0.01).expect("speech");
    let result = engine
        .transcribe(
            &pcm[..=last],
            &TranscribeOptions::default(),
            &CancelFlag::new(),
        )
        .expect("transcribe");
    let text = words(&result.text);
    assert!(
        text.ends_with("the final word of this dictation is lighthouse"),
        "unexpected transcript tail: {text}"
    );
}

#[test]
fn parakeet_dictation_and_files_work_through_the_child() {
    let Started {
        client,
        log_path,
        dir: _dir,
    } = start_client(Timing::default());
    let pid = client.status().pid;
    let loaded = client
        .load_model(hushpen_engine::LoadSpec {
            gpu: cfg!(target_os = "macos"),
            ..load_spec(&model())
        })
        .wait()
        .expect("load Parakeet in the speech child");
    assert_eq!(loaded.model, hushpen_core::catalog::SPEECH_DEFAULT);
    assert_eq!(loaded.gpu, cfg!(target_os = "macos"));

    let dictation = client.transcribe(TranscribeSpec {
        wav_path: short_wav(),
        language: None,
        prompt: Some("Zebrafinch. Quokka.".into()),
    });
    let Some(JobOutcome::Done(dictation)) = dictation.wait_timeout(Duration::from_secs(30)) else {
        panic!("the dictation did not finish");
    };
    assert_expected_words(&dictation.text);
    assert_eq!(dictation.language, "und");
    assert!(!dictation.segments.is_empty());

    let file = client.import(
        ImportSpec {
            path: short_wav(),
            language: Some("en".into()),
            prompt: None,
            translate: false,
        },
        None,
    );
    let Some(JobOutcome::Done(file)) = file.wait_timeout(Duration::from_secs(30)) else {
        panic!("the file did not finish");
    };
    assert_eq!(file.text, dictation.text);
    assert_eq!(file.segments, dictation.segments);
    assert_eq!(file.language, "und");
    assert!((4_500..5_700).contains(&file.audio_ms));

    let whisper = assets_dir()
        .expect("HUSHPEN_TEST_ASSETS")
        .join("models/whisper/ggml-base-q5_1.bin");
    let loaded = client
        .load_model(load_spec(&whisper))
        .wait()
        .expect("switch to Whisper Base Q5");
    assert_eq!(loaded.model, "base-q5_1");
    let whisper = client.transcribe(TranscribeSpec {
        wav_path: short_wav(),
        language: Some("en".into()),
        prompt: None,
    });
    let Some(JobOutcome::Done(whisper)) = whisper.wait_timeout(Duration::from_secs(30)) else {
        panic!("Whisper did not finish");
    };
    assert_expected_words(&whisper.text);
    assert_eq!(whisper.language, "en");
    assert_eq!(client.status().pid, pid, "switching keeps the same child");
    client.shutdown();

    let log = std::fs::read_to_string(log_path).expect("engine log");
    assert_eq!(log.matches(" JOB ").count(), 3);
    for word in ["quick", "brown", "zebrafinch", "quokka"] {
        assert!(
            !log.to_lowercase().contains(word),
            "private text in the log"
        );
    }
}
