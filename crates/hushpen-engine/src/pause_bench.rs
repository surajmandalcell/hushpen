//! Opt-in, private-corpus comparison of whole dictation and causal pause cuts.
//!
//! Set HUSHPEN_PAUSE_MANIFEST, HUSHPEN_PAUSE_MODEL, and HUSHPEN_PAUSE_OUT, then run
//! `cargo test -p hushpen-engine --lib pause_bench::compare -- --ignored --nocapture`.
//! The manifest is the speech-model research manifest, with its `mine` array.
//! Results contain speech text: keep the output outside the repository.
//! An existing output resumes after its last complete clip. On macOS, wait for games
//! to leave the front before loading Metal and before each clip (at most three hours).

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
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

fn completed_clips(path: &Path, header: &Value, clips: &[Value]) -> usize {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return 0,
        Err(error) => panic!("read existing results: {error}"),
    };
    let mut lines = BufReader::new(file).lines();
    let existing: Value = serde_json::from_str(&lines.next().expect("missing run header").unwrap())
        .expect("invalid run header");
    assert_eq!(&existing, header, "cannot resume a different measurement");
    let mut count = 0;
    for line in lines {
        let record: Value =
            serde_json::from_str(&line.unwrap()).expect("incomplete existing clip record");
        assert!(count < clips.len(), "too many existing clips");
        assert_eq!(record["type"], "clip");
        assert_eq!(record["id"], clips[count]["id"], "clip order changed");
        count += 1;
    }
    count
}

#[cfg(target_os = "macos")]
fn game_in_front(info: &str) -> bool {
    let info = info.to_lowercase();
    [
        "league of legends",
        "zelda",
        "ryujinx",
        "steam",
        "steamapps",
    ]
    .iter()
    .any(|name| info.contains(name))
}

#[cfg(target_os = "macos")]
fn wait_for_games(deadline: Instant) {
    loop {
        assert!(Instant::now() < deadline, "game wait deadline reached");
        let output = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                r#"front=$(lsappinfo front) && lsappinfo info -only name "$front" && lsappinfo info -only bundlepath -only executablepath "$front""#,
            ])
            .output()
            .expect("check front app");
        assert!(output.status.success(), "front app check failed");
        let info = String::from_utf8(output.stdout).expect("front app is UTF-8");
        assert!(!info.trim().is_empty(), "front app check returned no app");
        if !game_in_front(&info) {
            return;
        }
        eprintln!("pause benchmark: game in front, waiting 60 seconds");
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[cfg(not(target_os = "macos"))]
fn wait_for_games(_deadline: Instant) {}

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
    let output_path = required("HUSHPEN_PAUSE_OUT");
    let options = LoadOptions {
        gpu: true,
        threads: 8,
    };
    let header = json!({
        "type": "run", "model": model, "model_bytes": size, "gpu": true,
        "threads": options.threads, "clips": clips.len(), "frame_ms": 32,
        "variants": [{"silence_ms": 300, "minimum_ms": 5000},
                     {"silence_ms": 600, "minimum_ms": 10000}],
    });
    let completed = completed_clips(Path::new(&output_path), &header, clips);
    eprintln!("pause benchmark: {completed}/{} reused", clips.len());
    if completed == clips.len() {
        return;
    }
    let deadline = Instant::now() + std::time::Duration::from_secs(3 * 60 * 60);
    wait_for_games(deadline);
    let new_output = !Path::new(&output_path).exists();
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
    let mut output = BufWriter::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(output_path)
            .unwrap(),
    );
    if new_output {
        write_record(&mut output, header);
    }
    for (index, clip) in clips.iter().enumerate().skip(completed) {
        wait_for_games(deadline);
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

#[test]
fn resume_keeps_completed_clips_and_rejects_a_different_run() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("results.jsonl");
    let header = json!({"type": "run", "model": "turbo"});
    let clips = [json!({"id": "first"}), json!({"id": "second"})];
    assert_eq!(completed_clips(&path, &header, &clips), 0);
    let mut output = BufWriter::new(File::create(&path).unwrap());
    write_record(&mut output, header.clone());
    write_record(&mut output, json!({"type": "clip", "id": "first"}));
    assert_eq!(completed_clips(&path, &header, &clips), 1);
    assert!(std::panic::catch_unwind(|| completed_clips(&path, &json!({}), &clips)).is_err());
    assert!(
        std::panic::catch_unwind(|| {
            completed_clips(&path, &header, &[json!({"id": "second"})])
        })
        .is_err()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn games_and_steam_installs_block_metal_but_work_apps_do_not() {
    for info in [
        "\"League Of Legends\"",
        "\"Zelda\"",
        "\"Ryujinx\"",
        "\"Steam\"",
        "/Library/Application Support/Steam/steamapps/common/Portal 2",
    ] {
        assert!(game_in_front(info));
    }
    assert!(!game_in_front("\"Terminal\""));
    assert!(!game_in_front("\"Orca\""));
}
