//! `hushpen engine`: the entry point of the engine child.
//!
//! No arguments serves the frame protocol on stdin and stdout. `--smoke <wav> --model <ggml file>`
//! transcribes one file, prints the result, and exits; it opens no window and no data folder.

use crate::asr::{AsrEngine, CancelFlag, LoadOptions, TranscribeOptions};
use crate::server::{Loader, ServerConfig, serve};
use crate::wav;
use crate::whisper::WhisperEngine;
use hushpen_core::threads::auto_threads;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

/// True when a GPU backend is built in and `HUSHPEN_DISABLE_GPU` does not switch it off.
pub fn gpu_available() -> bool {
    cfg!(target_os = "macos") && !gpu_disabled_by_env()
}

/// `HUSHPEN_DISABLE_GPU` set to anything but empty or `0` forces the CPU.
pub fn gpu_disabled_by_env() -> bool {
    std::env::var_os("HUSHPEN_DISABLE_GPU").is_some_and(|value| !value.is_empty() && value != "0")
}

fn whisper_loader() -> Arc<Loader> {
    Arc::new(|path, options| {
        WhisperEngine::load(path, options).map(|engine| Box::new(engine) as Box<dyn AsrEngine>)
    })
}

/// Runs the engine child. `args` are the arguments after `engine`.
pub fn engine_main(args: &[String]) -> ExitCode {
    crate::whisper::quiet_native_logs();
    if args.is_empty() {
        let config = ServerConfig {
            version: hushpen_core::BUILD_VERSION.to_owned(),
            gpu_available: gpu_available(),
            loader: whisper_loader(),
        };
        return ExitCode::from(serve(std::io::stdin().lock(), std::io::stdout(), config));
    }
    match parse_smoke(args) {
        Ok(smoke) => run_smoke(&smoke),
        Err(problem) => {
            eprintln!("hushpen engine: {problem}");
            ExitCode::from(2)
        }
    }
}

struct Smoke {
    wav: PathBuf,
    model: PathBuf,
}

fn parse_smoke(args: &[String]) -> Result<Smoke, String> {
    let mut wav = None;
    let mut model = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--smoke" => wav = iter.next().map(PathBuf::from),
            "--model" => model = iter.next().map(PathBuf::from),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    match (wav, model) {
        (Some(wav), Some(model)) => Ok(Smoke { wav, model }),
        _ => Err("usage: hushpen engine --smoke <file.wav> --model <ggml file>".into()),
    }
}

fn run_smoke(smoke: &Smoke) -> ExitCode {
    let cpus = std::thread::available_parallelism().map_or(2, usize::from);
    let options = LoadOptions {
        gpu: gpu_available(),
        threads: auto_threads(cpus),
    };
    let pcm = match wav::read_mono_16k(&smoke.wav) {
        Ok(pcm) => pcm,
        Err(problem) => {
            eprintln!("hushpen engine: {}: {problem}", smoke.wav.display());
            return ExitCode::FAILURE;
        }
    };
    let mut engine = match WhisperEngine::load(&smoke.model, options) {
        Ok(engine) => engine,
        Err(problem) => {
            eprintln!("hushpen engine: {problem}");
            return ExitCode::FAILURE;
        }
    };
    let transcript =
        match engine.transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new()) {
            Ok(transcript) => transcript,
            Err(problem) => {
                eprintln!("hushpen engine: {problem}");
                return ExitCode::FAILURE;
            }
        };
    println!("gpu: {}", if engine.gpu_in_use() { "on" } else { "off" });
    println!("threads: {}", options.threads);
    println!("language: {}", transcript.language);
    println!("text: {}", transcript.text);
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn smoke_needs_a_file_and_a_model() {
        let smoke = parse_smoke(&strings(&["--smoke", "a.wav", "--model", "m.bin"])).unwrap();
        assert_eq!(smoke.wav, PathBuf::from("a.wav"));
        assert_eq!(smoke.model, PathBuf::from("m.bin"));
        assert!(parse_smoke(&strings(&["--smoke", "a.wav"])).is_err());
        assert!(parse_smoke(&strings(&["--model", "m.bin"])).is_err());
        assert!(parse_smoke(&strings(&["--bogus"])).is_err());
    }
}
