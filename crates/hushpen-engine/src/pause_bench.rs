//! Opt-in, private-corpus comparison of whole dictation and causal pause cuts.
//!
//! Set HUSHPEN_PAUSE_MANIFEST, HUSHPEN_PAUSE_MODEL, and HUSHPEN_PAUSE_OUT, then run
//! `cargo test -p hushpen-engine --lib pause_bench::compare -- --ignored --nocapture`.
//! The manifest is the speech-model research manifest, with its `mine` array.
//! Results contain speech text: keep the output outside the repository.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use whisper_rs::{WhisperVadContext, WhisperVadContextParams};

use crate::{AsrEngine, CancelFlag, LoadOptions, ParakeetEngine, TranscribeOptions, WhisperEngine};

const RATE: usize = 16_000;
const FRAME: usize = 512;

/// The end of every non-final range is also its release time. Decisions consume only complete
/// causal Silero frames; no future probabilities or offline segment padding choose a cut.
fn parts(probs: &[f32], samples: usize, silence_ms: usize, minimum_ms: usize) -> Vec<Range<usize>> {
    let silence_frames = (silence_ms * RATE).div_ceil(1000 * FRAME);
    let minimum = minimum_ms * RATE / 1000;
    let mut result = Vec::new();
    let mut start = 0;
    let mut quiet = 0;
    let mut speech = false;
    for (frame, &probability) in probs.iter().enumerate() {
        let end = (frame + 1) * FRAME;
        if end > samples {
            break;
        }
        if probability >= 0.5 {
            speech = true;
            quiet = 0;
        } else if probability < 0.35 {
            quiet += 1;
        } else {
            quiet = 0;
        }
        if speech && quiet >= silence_frames && end - start >= minimum {
            result.push(start..end);
            start = end;
            quiet = 0;
            speech = false;
        }
    }
    // A partly received last frame can contain the final word.
    if speech
        || result.is_empty()
        || !samples.is_multiple_of(FRAME) && probs.last().is_some_and(|p| *p >= 0.5)
    {
        result.push(start..samples);
    }
    result
}

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

fn write_record(output: &mut BufWriter<File>, value: Value) {
    serde_json::to_writer(&mut *output, &value).expect("write result");
    writeln!(output).unwrap();
    output.flush().unwrap();
}

#[test]
#[ignore = "requires the private speech corpus, a model, and Metal"]
fn compare() {
    crate::whisper::quiet_native_logs();
    let model = required("HUSHPEN_PAUSE_MODEL");
    let size = std::fs::metadata(&model).unwrap().len();
    assert!(size <= 1_500_000_000, "model exceeds the measurement limit");
    let manifest: Value =
        serde_json::from_reader(File::open(required("HUSHPEN_PAUSE_MANIFEST")).unwrap()).unwrap();
    let clips = manifest["mine"].as_array().expect("mine clips");
    let mut output = BufWriter::new(File::create(required("HUSHPEN_PAUSE_OUT")).unwrap());
    let options = LoadOptions {
        gpu: true,
        threads: 8,
    };
    let parakeet = Path::new(&model)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("ggml-parakeet");
    let mut engine: Box<dyn AsrEngine> = if parakeet {
        Box::new(ParakeetEngine::load(Path::new(&model), options).expect("load Parakeet"))
    } else {
        Box::new(WhisperEngine::load(Path::new(&model), options).expect("load Whisper"))
    };
    assert!(engine.gpu_in_use(), "Metal must be active");
    let mut vad_params = WhisperVadContextParams::default();
    vad_params.set_n_threads(1);
    let mut vad = WhisperVadContext::new(&crate::vad::model_path().unwrap(), vad_params).unwrap();
    // Exclude graph and shader initialization, as with the app's warm, loaded model.
    let warm = crate::wav::read_mono_16k(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/speech-short.wav"),
    )
    .unwrap();
    engine
        .transcribe(&warm, &TranscribeOptions::default(), &CancelFlag::new())
        .unwrap();
    vad.detect_speech(&warm).unwrap();
    write_record(
        &mut output,
        json!({
            "type": "run", "model": model, "model_bytes": size, "gpu": engine.gpu_in_use(),
            "threads": options.threads, "clips": clips.len(), "frame_ms": 32,
            "variants": [{"silence_ms": 300, "minimum_ms": 5000},
                         {"silence_ms": 600, "minimum_ms": 10000}],
        }),
    );
    for (index, clip) in clips.iter().enumerate() {
        let pcm = crate::wav::read_mono_16k(Path::new(clip["wav"].as_str().unwrap())).unwrap();
        let audio_s = pcm.len() as f64 / RATE as f64;
        let started = Instant::now();
        vad.detect_speech(&pcm).unwrap();
        let vad_s = started.elapsed().as_secs_f64();
        let probabilities = vad.probabilities();
        // VAD is causal and runs while recording. Charge all its measured work against each
        // release as a conservative bound, rather than granting it zero compute time.
        let mut variants = Vec::new();
        for (silence, minimum) in [(300, 5000), (600, 10000)] {
            let ranges = parts(probabilities, pcm.len(), silence, minimum);
            let mut text = String::new();
            let mut language = None;
            let mut completed_s = 0.0_f64;
            let mut measurements = Vec::new();
            for (part, range) in ranges.iter().enumerate() {
                let final_part = part + 1 == ranges.len() && range.end == pcm.len();
                let release_s = if final_part {
                    audio_s
                } else {
                    range.end as f64 / RATE as f64
                };
                let started = Instant::now();
                let result = engine
                    .transcribe(
                        &pcm[range.clone()],
                        &TranscribeOptions {
                            language: language.clone(),
                            prompt: (!parakeet && !text.is_empty()).then(|| text.clone()),
                            translate: false,
                        },
                        &CancelFlag::new(),
                    )
                    .expect("transcribe part");
                let decode_s = started.elapsed().as_secs_f64();
                completed_s = completed_s.max(release_s + vad_s) + decode_s;
                if !result.text.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&result.text);
                    if result.language != "und" {
                        language.get_or_insert(result.language);
                    }
                }
                measurements.push(json!({
                    "start_sample": range.start, "end_sample": range.end,
                    "release_s": release_s, "decode_s": decode_s, "completed_s": completed_s,
                }));
            }
            variants.push(json!({
                "silence_ms": silence, "minimum_ms": minimum, "text": text,
                "stop_to_text_s": (completed_s - audio_s).max(0.0),
                "parts": measurements,
            }));
        }
        let started = Instant::now();
        let whole = engine
            .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
            .expect("transcribe whole");
        let whole_s = started.elapsed().as_secs_f64();
        write_record(
            &mut output,
            json!({
                "type": "clip", "id": clip["id"], "audio_s": audio_s, "vad_s": vad_s,
                "whole": {"text": whole.text, "stop_to_text_s": whole_s}, "variants": variants,
            }),
        );
        eprintln!("pause benchmark: {}/{} complete", index + 1, clips.len());
    }
}

#[test]
#[allow(clippy::single_range_in_vec_init)]
fn cuts_wait_for_complete_silence_frames_and_keep_a_short_dictation_whole() {
    let mut probs = vec![0.9; 40];
    probs.extend([0.1; 10]);
    probs.extend([0.9; 20]);
    assert_eq!(
        parts(&probs, 70 * FRAME, 300, 0),
        [0..50 * FRAME, 50 * FRAME..70 * FRAME]
    );
    assert_eq!(parts(&probs, 70 * FRAME, 300, 5000), [0..70 * FRAME]);
    assert_eq!(parts(&probs[..49], 49 * FRAME, 300, 0), [0..49 * FRAME]);
}

#[test]
#[allow(clippy::single_range_in_vec_init)]
fn no_speech_does_not_create_intermediate_parts_and_a_silent_tail_needs_no_decode() {
    assert_eq!(parts(&[0.1; 100], 100 * FRAME, 300, 0), [0..100 * FRAME]);
    let mut probs = vec![0.9; 40];
    probs.extend([0.1; 60]);
    assert_eq!(parts(&probs, 100 * FRAME, 300, 0), [0..50 * FRAME]);
}
