//! Opt-in: the memory of one `hushpen-asr` process over many clips in a row.
//!
//! Set HUSHPEN_MEM_MODEL (a speech model of at most 1.5 GB) and HUSHPEN_MEM_MANIFEST (the
//! speech-model research manifest, with its `mine` array of WAVs), then run
//! `cargo test -p hushpen-engine --features heavy --test it memory:: -- --ignored --nocapture`.
//! HUSHPEN_MEM_CLIPS sets how many clips (30; the list repeats past its end),
//! HUSHPEN_MEM_GPU=0 keeps the model off Metal, and HUSHPEN_MEM_LONG names one more WAV
//! transcribed at the end, such as a long file. It prints the footprint (macOS
//! `phys_footprint`, the Activity Monitor Memory column; VmRSS on Linux) after the load and
//! after every 5 clips. The clips hold private speech: it prints no text.
//!
//! `engine_memory_across_model_loads` loads HUSHPEN_MEM_MODEL and HUSHPEN_MEM_SWITCH (another
//! model) in turn, HUSHPEN_MEM_LOADS times (8), as a model switch in Models does, with one clip
//! after each load, and prints the footprint after each.

use super::child::start_client;
use hushpen_core::idle::Idle;
use hushpen_engine::{EngineClient, JobOutcome, LoadSpec, Timing, TranscribeSpec};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The current and the highest footprint of `pid`, in bytes.
fn footprint(pid: u32) -> (u64, u64) {
    if cfg!(target_os = "macos") {
        let output = Command::new("footprint")
            .args(["-p", &pid.to_string(), "-f", "bytes", "--noCategories"])
            .output()
            .expect("run footprint");
        let text = String::from_utf8_lossy(&output.stdout);
        let field = |name: &str| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix(name))
                .and_then(|rest| rest.trim().trim_end_matches(" B").parse().ok())
                .unwrap_or_else(|| panic!("no {name} in {text}"))
        };
        (field("phys_footprint:"), field("phys_footprint_peak:"))
    } else {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
        let field = |name: &str| -> u64 {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .and_then(|rest| rest.trim().trim_end_matches(" kB").trim().parse().ok())
                .map_or(0, |kb: u64| kb * 1024)
        };
        (field("VmRSS:"), field("VmHWM:"))
    }
}

fn mb(bytes: u64) -> u64 {
    bytes.div_ceil(1 << 20)
}

fn transcribe(client: &EngineClient, wav: &Path) -> f64 {
    let spec = TranscribeSpec {
        wav_path: wav.to_path_buf(),
        language: None,
        prompt: None,
    };
    match client.transcribe(spec).wait() {
        JobOutcome::Done(_) | JobOutcome::Failed(_) => {}
        other => panic!("clip {}: {other:?}", wav.display()),
    }
    let reader = hound::WavReader::open(wav).expect("open wav");
    f64::from(reader.duration()) / f64::from(reader.spec().sample_rate)
}

fn load(client: &EngineClient, model: &Path, threads: usize) -> String {
    client
        .load_model(LoadSpec {
            path: model.to_path_buf(),
            gpu: std::env::var("HUSHPEN_MEM_GPU").map_or(true, |value| value != "0"),
            threads,
            idle: Idle::KEEP,
        })
        .wait()
        .expect("load")
        .model
}

#[test]
#[ignore = "needs the private speech corpus and two speech models"]
fn engine_memory_across_model_loads() {
    let first =
        PathBuf::from(std::env::var_os("HUSHPEN_MEM_MODEL").expect("set HUSHPEN_MEM_MODEL"));
    let second =
        PathBuf::from(std::env::var_os("HUSHPEN_MEM_SWITCH").expect("set HUSHPEN_MEM_SWITCH"));
    for model in [&first, &second] {
        assert!(std::fs::metadata(model).unwrap().len() <= 1_500_000_000);
    }
    let manifest: Value = serde_json::from_reader(
        std::fs::File::open(
            std::env::var_os("HUSHPEN_MEM_MANIFEST").expect("set HUSHPEN_MEM_MANIFEST"),
        )
        .unwrap(),
    )
    .unwrap();
    let clip = PathBuf::from(manifest["mine"][0]["wav"].as_str().unwrap());
    let loads: usize = std::env::var("HUSHPEN_MEM_LOADS").map_or(8, |n| n.parse().unwrap());
    let cpus = std::thread::available_parallelism().map_or(2, usize::from);
    let threads = hushpen_core::threads::auto_threads(cpus);
    let started = start_client(Timing::default());
    let client = started.client;
    let pid = client.status().pid.expect("engine pid");
    for index in 0..loads {
        let model = if index % 2 == 0 { &first } else { &second };
        let name = load(&client, model, threads);
        let (loaded, peak) = footprint(pid);
        transcribe(&client, &clip);
        let (after_clip, _) = footprint(pid);
        println!(
            "memory: load={} model={name} footprint_mb={} after_clip_mb={} peak_mb={}",
            index + 1,
            mb(loaded),
            mb(after_clip),
            mb(peak)
        );
    }
    assert_eq!(client.status().pid, Some(pid), "the engine restarted");
    client.shutdown();
}

#[test]
#[ignore = "needs the private speech corpus and a speech model"]
fn engine_memory_over_many_clips() {
    let model =
        PathBuf::from(std::env::var_os("HUSHPEN_MEM_MODEL").expect("set HUSHPEN_MEM_MODEL"));
    let size = std::fs::metadata(&model).unwrap().len();
    assert!(size <= 1_500_000_000, "a model over 1.5 GB is not loaded");
    let manifest: Value = serde_json::from_reader(
        std::fs::File::open(
            std::env::var_os("HUSHPEN_MEM_MANIFEST").expect("set HUSHPEN_MEM_MANIFEST"),
        )
        .unwrap(),
    )
    .unwrap();
    let count: usize = std::env::var("HUSHPEN_MEM_CLIPS").map_or(30, |n| n.parse().unwrap());
    let clips: Vec<PathBuf> = manifest["mine"]
        .as_array()
        .expect("mine clips")
        .iter()
        .cycle()
        .take(count)
        .map(|clip| PathBuf::from(clip["wav"].as_str().unwrap()))
        .collect();
    let gpu = std::env::var("HUSHPEN_MEM_GPU").map_or(true, |value| value != "0");
    let cpus = std::thread::available_parallelism().map_or(2, usize::from);

    let started = start_client(Timing::default());
    let client = started.client;
    let pid = client.status().pid.expect("engine pid");
    let (idle, _) = footprint(pid);
    let loaded = client
        .load_model(LoadSpec {
            path: model.clone(),
            gpu,
            threads: hushpen_core::threads::auto_threads(cpus),
            idle: Idle::KEEP,
        })
        .wait()
        .expect("load");
    let (after_load, _) = footprint(pid);
    println!(
        "memory: model {} ({} MB file) gpu={} pid {pid}",
        loaded.model,
        mb(size),
        loaded.gpu
    );
    println!(
        "memory: clips=0 audio_s=0 footprint_mb={} (before load {})",
        mb(after_load),
        mb(idle)
    );
    let mut audio = 0.0;
    for (index, clip) in clips.iter().enumerate() {
        audio += transcribe(&client, clip);
        if (index + 1) % 5 == 0 || index + 1 == clips.len() {
            let (now, peak) = footprint(pid);
            println!(
                "memory: clips={} audio_s={audio:.0} footprint_mb={} peak_mb={}",
                index + 1,
                mb(now),
                mb(peak)
            );
        }
    }
    if let Some(long) = std::env::var_os("HUSHPEN_MEM_LONG") {
        let seconds = transcribe(&client, Path::new(&long));
        let (now, peak) = footprint(pid);
        println!(
            "memory: long file {seconds:.0} s footprint_mb={} peak_mb={}",
            mb(now),
            mb(peak)
        );
    }
    assert_eq!(client.status().pid, Some(pid), "the engine restarted");
    if cfg!(target_os = "macos") {
        let output = Command::new("footprint")
            .args(["-p", &pid.to_string()])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines().skip(5).take(8) {
            println!("memory: category {}", line.trim());
        }
    }
    client.shutdown();
}
