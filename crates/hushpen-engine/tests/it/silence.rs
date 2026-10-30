//! Generated silence around the public speech fixture, never private recordings.

use super::{assert_expected_words, english, load, read_wav, short_wav};
use hushpen_core::export::{self, Format, Item, Segment};
use hushpen_engine::{AsrEngine, CancelFlag};

#[test]
fn silence_after_speech_has_no_text() {
    let mut engine = load(false);
    let spoken = engine
        .transcribe(&read_wav(&short_wav()), &english(), &CancelFlag::new())
        .unwrap();
    assert_expected_words(&spoken.text);
    for samples in [1, 16_000 / 20, 16_000, 16_000 * 30] {
        let result = engine
            .transcribe(&vec![0.0; samples], &Default::default(), &CancelFlag::new())
            .unwrap();
        assert!(result.text.is_empty(), "silence produced text");
        assert!(result.segments.is_empty());
    }
}

#[test]
fn silence_gaps_keep_words_and_original_timestamps() {
    let mut engine = load(false);
    let speech = read_wav(&short_wav());
    let mut pcm = vec![0.0; 16_000 * 8];
    pcm.extend_from_slice(&speech);
    pcm.resize(pcm.len() + 16_000 * 10, 0.0);
    let second_start_ms = pcm.len() as u64 / 16;
    pcm.extend_from_slice(&speech);
    pcm.resize(pcm.len() + 16_000 * 7, 0.0);
    let result = engine
        .transcribe(&pcm, &english(), &CancelFlag::new())
        .unwrap();
    let phrase = "the quick brown fox jumps over the lazy dog numbers";
    assert!(super::words(&result.text).starts_with(phrase));
    assert_eq!(super::words(&result.text).matches(phrase).count(), 2);
    for part in super::words(&result.text).split(phrase).skip(1) {
        assert!(
            part.trim() == "1 2 3 4 5" || part.trim() == "one two three four five",
            "a word was lost or added around a silence gap"
        );
    }
    assert!(result.segments.first().unwrap().start_ms >= 7_000);
    assert!(
        result
            .segments
            .iter()
            .any(|segment| segment.end_ms >= second_start_ms + 3_000),
        "timestamps used the shortened audio clock"
    );
    assert!(result.segments.last().unwrap().end_ms <= pcm.len() as u64 / 16);

    let item = Item {
        final_text: Some(result.text),
        duration_ms: (pcm.len() / 16) as i64,
        segments: result
            .segments
            .iter()
            .map(|segment| Segment {
                start_ms: segment.start_ms as i64,
                end_ms: segment.end_ms as i64,
                text: segment.text.clone(),
            })
            .collect(),
        ..Item::default()
    };
    for (format, separator) in [(Format::Srt, ','), (Format::Vtt, '.')] {
        let output = export::render(format, std::slice::from_ref(&item));
        for segment in &item.segments {
            let clock = |ms: i64| {
                format!(
                    "{:02}:{:02}:{:02}{separator}{:03}",
                    ms / 3_600_000,
                    ms / 60_000 % 60,
                    ms / 1_000 % 60,
                    ms % 1_000,
                )
            };
            assert!(output.contains(&format!(
                "{} --> {}",
                clock(segment.start_ms),
                clock(segment.end_ms)
            )));
        }
    }
}

#[test]
fn imported_speech_after_silence_keeps_subtitle_times() {
    use super::child::{load_spec, start_client};
    use hushpen_engine::{ImportSpec, JobOutcome, Timing};

    let started = start_client(Timing::default());
    started
        .client
        .load_model(load_spec(&super::model_path()))
        .wait()
        .unwrap();
    let speech = read_wav(&short_wav());
    let mut pcm = vec![0.0; 16_000 * 125];
    pcm.extend_from_slice(&speech);
    pcm.resize(pcm.len() + 16_000 * 8, 0.0);
    let path = started.dir.path().join("silence-gaps.wav");
    let mut wav = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for sample in pcm {
        wav.write_sample((sample * 32768.0) as i16).unwrap();
    }
    wav.finalize().unwrap();
    let job = started.client.import(
        ImportSpec {
            path,
            language: Some("en".into()),
            prompt: None,
            translate: false,
        },
        None,
    );
    let result = job.wait_timeout(std::time::Duration::from_secs(30));
    started.client.shutdown();
    let Some(JobOutcome::Done(result)) = result else {
        panic!("the generated import did not finish");
    };
    assert_expected_words(&result.text);
    assert!(result.segments.first().unwrap().start_ms >= 124_000);
    assert!(result.segments.last().unwrap().end_ms >= 128_000);
}

#[test]
fn an_empty_first_window_does_not_choose_the_language() {
    use hushpen_engine::{LoadOptions, TranscribeOptions, WhisperEngine};

    let Some(assets) = super::assets_dir() else {
        eprintln!("skipped: HUSHPEN_TEST_ASSETS is not set");
        return;
    };
    let model = assets.join("models/whisper/ggml-base.bin");
    let speech = assets.join("fixtures/speech-es.wav");
    if !model.is_file() || !speech.is_file() {
        eprintln!("skipped: the multilingual model or Spanish fixture is missing");
        return;
    }
    let mut engine = WhisperEngine::load(
        &model,
        LoadOptions {
            gpu: false,
            threads: 2,
        },
    )
    .unwrap();
    let mut pcm = vec![0.0; 16_000 * 125];
    pcm.extend(read_wav(&speech));
    let result = engine
        .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
        .unwrap();
    assert_eq!(result.language, "es");
    assert!(!result.text.is_empty());
    assert!(result.segments.first().unwrap().start_ms >= 124_000);
}
