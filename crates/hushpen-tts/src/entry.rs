//! `hushpen-tts`: the entry point of the worker.
//!
//! No arguments serves the frame protocol on stdin and stdout. `--smoke <model folder>` speaks
//! a fixed text, prints the timings and the real-time factor (synthesis time over audio time,
//! under 1 is faster than real time), and exits; it opens no data folder and no socket.

use crate::backend::{Backend, CancelFlag, LoadSpec, TtsEngine};
use crate::kokoro::SherpaBackend;
use crate::runtime;
use crate::server::{ServerConfig, serve};
use hushpen_core::kokoro::{DEFAULT_VOICE, find_voice};
use hushpen_core::threads::auto_threads;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

/// Prints the version of the sherpa-onnx library the worker opens, so a package check can prove
/// the bundled libraries load, without a model.
fn check_runtime() -> ExitCode {
    match SherpaBackend::new(runtime::library_path()).api() {
        Ok(api) => {
            println!("sherpa-onnx {}", api.version());
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("hushpen-tts: {problem}");
            ExitCode::FAILURE
        }
    }
}

/// The text of the smoke run: three sentences, so the first audio comes before the end.
pub const SMOKE_TEXT: &str = "Hushpen reads your notes aloud on this computer. Nothing you \
write leaves it. The voice runs in a worker of its own, so a fault there never takes the app \
down.";

/// Runs the worker. `args` are the arguments after the program name.
pub fn worker_main(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("--version") => {
            println!("hushpen-tts {}", hushpen_core::VERSION);
            return ExitCode::SUCCESS;
        }
        Some("--check-runtime") => return check_runtime(),
        _ => {}
    }
    let smoke = match parse_args(args) {
        Ok(smoke) => smoke,
        Err(problem) => {
            eprintln!("hushpen-tts: {problem}");
            return ExitCode::from(2);
        }
    };
    let backend = SherpaBackend::new(runtime::library_path());
    match smoke {
        None => {
            let config = ServerConfig {
                version: hushpen_core::BUILD_VERSION.to_owned(),
                backend: Arc::new(backend),
            };
            ExitCode::from(serve(std::io::stdin().lock(), std::io::stdout(), config))
        }
        Some(smoke) => run_smoke(&backend, &smoke),
    }
}

#[derive(Debug, PartialEq)]
struct Smoke {
    dir: PathBuf,
    threads: Option<usize>,
    voice: String,
    text: String,
    runs: usize,
    out: Option<PathBuf>,
}

const USAGE: &str = "usage: hushpen-tts [--version] [--check-runtime] \
[--smoke <model folder> [--threads N] [--voice ID] [--text TEXT] [--runs N] [--out FILE.wav]]";

fn parse_args(args: &[String]) -> Result<Option<Smoke>, String> {
    if args.is_empty() {
        return Ok(None);
    }
    let mut dir = None;
    let mut smoke = Smoke {
        dir: PathBuf::new(),
        threads: None,
        voice: DEFAULT_VOICE.to_owned(),
        text: SMOKE_TEXT.to_owned(),
        runs: 1,
        out: None,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--smoke" => dir = Some(PathBuf::from(value()?)),
            "--threads" => smoke.threads = Some(number(value()?, arg)?),
            "--voice" => smoke.voice = value()?.clone(),
            "--text" => smoke.text = value()?.clone(),
            "--runs" => smoke.runs = number(value()?, arg)?,
            "--out" => smoke.out = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument '{other}'\n{USAGE}")),
        }
    }
    smoke.dir = dir.ok_or_else(|| USAGE.to_owned())?;
    Ok(Some(smoke))
}

fn number(text: &str, arg: &str) -> Result<usize, String> {
    text.parse()
        .ok()
        .filter(|count| *count > 0)
        .ok_or_else(|| format!("{arg} needs a number above 0"))
}

fn run_smoke(backend: &SherpaBackend, smoke: &Smoke) -> ExitCode {
    match smoke_runs(backend, smoke) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("hushpen-tts: {problem}");
            ExitCode::FAILURE
        }
    }
}

fn smoke_runs(backend: &SherpaBackend, smoke: &Smoke) -> Result<(), String> {
    if find_voice(&smoke.voice).is_none() {
        return Err(format!("unknown voice '{}'", smoke.voice));
    }
    let cpus = std::thread::available_parallelism().map_or(2, usize::from);
    let spec = LoadSpec {
        dir: smoke.dir.clone(),
        engine: TtsEngine::Kokoro,
        threads: smoke.threads.unwrap_or_else(|| auto_threads(cpus)),
        idle: None,
    };
    println!("runtime: sherpa-onnx {}", backend.api()?.version());
    let loading = Instant::now();
    let mut session = backend.load(&spec).map_err(|failure| failure.detail)?;
    println!("threads: {}", spec.threads);
    println!("load_ms: {}", loading.elapsed().as_millis());
    let rate = session.sample_rate();
    let mut audio = Vec::new();
    for run in 1..=smoke.runs {
        audio.clear();
        let started = Instant::now();
        let mut first_audio = None;
        session
            .speak(
                &smoke.text,
                Some(&smoke.voice),
                1.0,
                &CancelFlag::new(),
                &mut |samples| {
                    first_audio.get_or_insert_with(|| started.elapsed());
                    audio.extend_from_slice(samples);
                },
            )
            .map_err(|failure| format!("{failure:?}"))?;
        let synth = started.elapsed().as_secs_f64();
        let seconds = audio.len() as f64 / f64::from(rate);
        let peak = audio
            .iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        if seconds < 1.0 || peak < 0.01 {
            return Err(format!(
                "run {run} wrote {seconds:.2} s of audio with a peak of {peak:.4}"
            ));
        }
        println!(
            "run {run}: audio_ms={:.0} synth_ms={:.0} first_audio_ms={:.0} rtf={:.3} peak={peak:.3}",
            seconds * 1000.0,
            synth * 1000.0,
            first_audio.map_or(0.0, |first| first.as_secs_f64() * 1000.0),
            synth / seconds
        );
    }
    if let Some(out) = &smoke.out {
        write_wav(out, rate, &audio)?;
        println!("wav: {}", out.display());
    }
    Ok(())
}

fn write_wav(path: &Path, rate: u32, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let failed = |error: hound::Error| format!("cannot write {}: {error}", path.display());
    let mut writer = hound::WavWriter::create(path, spec).map_err(failed)?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        writer.write_sample(value).map_err(failed)?;
    }
    writer.finalize().map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn no_arguments_serve_the_pipe_protocol() {
        assert_eq!(parse_args(&[]), Ok(None));
    }

    #[test]
    fn smoke_takes_a_folder_and_optional_settings() {
        let Ok(Some(smoke)) = parse_args(&strings(&[
            "--smoke",
            "/m/kokoro",
            "--threads",
            "3",
            "--voice",
            "bm_george",
            "--runs",
            "2",
            "--out",
            "/tmp/a.wav",
        ])) else {
            panic!("a smoke run");
        };
        assert_eq!(smoke.dir, PathBuf::from("/m/kokoro"));
        assert_eq!((smoke.threads, smoke.runs), (Some(3), 2));
        assert_eq!(smoke.voice, "bm_george");
        assert_eq!(smoke.text, SMOKE_TEXT);
        assert_eq!(smoke.out, Some(PathBuf::from("/tmp/a.wav")));
    }

    #[test]
    fn bad_smoke_arguments_are_refused() {
        for args in [
            &["--smoke"][..],
            &["--threads", "2"],
            &["--smoke", "/m", "--runs", "0"],
            &["--smoke", "/m", "--threads", "x"],
            &["--bogus"],
        ] {
            assert!(parse_args(&strings(args)).is_err(), "{args:?}");
        }
    }
}
