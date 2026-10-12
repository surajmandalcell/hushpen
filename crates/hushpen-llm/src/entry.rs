//! `hushpen-llm`: the entry point of the worker.
//!
//! No arguments serves the frame protocol on stdin and stdout. `--smoke <gguf>` cleans one fixed
//! sentence, prints the result, and exits; it opens no data folder and no socket.

use crate::backend::{Backend, CancelFlag, JobSpec, LoadSpec, Session, SessionEnd};
use crate::llama::LlamaRuntime;
use crate::server::{ServerConfig, serve};
use hushpen_core::llm_prompt::{CLEANUP_SYSTEM, PromptFamily};
use hushpen_core::threads::auto_threads;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

/// The sentence of the smoke run: fillers, a repeat, and a spoken self-correction.
pub const SMOKE_SENTENCE: &str = "um so I think we should uh meet on tuesday no wait wednesday at \
like three pm to go over the the budget you know";

/// Words the smoke run must find gone from the output.
pub const SMOKE_FILLERS: [&str; 3] = ["um", "uh", "er"];

/// Runs the worker. `args` are the arguments after the program name.
pub fn worker_main(args: &[String]) -> ExitCode {
    if args.first().map(String::as_str) == Some("--version") {
        println!("hushpen-llm {}", hushpen_core::VERSION);
        return ExitCode::SUCCESS;
    }
    let smoke = match parse_args(args) {
        Ok(smoke) => smoke,
        Err(problem) => {
            eprintln!("hushpen-llm: {problem}");
            return ExitCode::from(2);
        }
    };
    let runtime = match LlamaRuntime::init() {
        Ok(runtime) => runtime,
        Err(problem) => {
            eprintln!("hushpen-llm: {problem}");
            return ExitCode::FAILURE;
        }
    };
    match smoke {
        None => {
            let config = ServerConfig {
                version: hushpen_core::BUILD_VERSION.to_owned(),
                backend: Arc::new(runtime),
            };
            ExitCode::from(serve(std::io::stdin().lock(), std::io::stdout(), config))
        }
        Some(smoke) => run_smoke(&runtime, &smoke),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Smoke {
    model: PathBuf,
    gpu: bool,
    threads: Option<usize>,
}

fn parse_args(args: &[String]) -> Result<Option<Smoke>, String> {
    if args.is_empty() {
        return Ok(None);
    }
    let mut model = None;
    let mut gpu = false;
    let mut threads = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--smoke" => model = iter.next().map(PathBuf::from),
            "--gpu" => gpu = true,
            "--threads" => {
                threads = Some(
                    iter.next()
                        .and_then(|value| value.parse().ok())
                        .ok_or("--threads needs a number")?,
                );
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    model
        .map(|model| {
            Some(Smoke {
                model,
                gpu,
                threads,
            })
        })
        .ok_or_else(|| "usage: hushpen-llm [--smoke <model.gguf> [--gpu] [--threads N]]".into())
}

fn run_smoke(runtime: &LlamaRuntime, smoke: &Smoke) -> ExitCode {
    let file = smoke
        .model
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(family) = PromptFamily::from_model_name(&file) else {
        eprintln!("hushpen-llm: the model family is not known from the file name");
        return ExitCode::FAILURE;
    };
    let cpus = std::thread::available_parallelism().map_or(2, usize::from);
    let spec = LoadSpec {
        path: smoke.model.clone(),
        family,
        gpu: smoke.gpu,
        threads: smoke.threads.unwrap_or_else(|| auto_threads(cpus)),
        idle: None,
    };
    let mut outcome = None;
    let loaded = runtime.with_session(&spec, &mut |session: &mut dyn Session, info| {
        let job = JobSpec {
            system: CLEANUP_SYSTEM.to_owned(),
            user: SMOKE_SENTENCE.to_owned(),
            max_tokens: 128,
        };
        let cancel = CancelFlag::new();
        let first = session.run(&job, &cancel);
        let second = session.run(&job, &cancel);
        outcome = Some((info, first, second));
        SessionEnd::Quit
    });
    if let Err(problem) = loaded {
        eprintln!("hushpen-llm: {}", problem.detail);
        return ExitCode::FAILURE;
    }
    let Some((info, Ok(first), Ok(second))) = outcome else {
        eprintln!("hushpen-llm: the cleanup job failed");
        return ExitCode::FAILURE;
    };
    println!("backend: {}", runtime.backend_name());
    println!("gpu: {}", if info.gpu { "on" } else { "off" });
    println!("threads: {}", spec.threads);
    println!("load_ms: {}", info.ms);
    println!("prompt_tokens: {}", first.stats.prompt_tokens);
    println!(
        "first_new_tokens: {} second_new_tokens: {} second_reused_tokens: {}",
        first.stats.new_tokens, second.stats.new_tokens, second.stats.reused_tokens
    );
    println!("text: {}", first.text);
    let fillers = SMOKE_FILLERS.iter().any(|filler| {
        first
            .text
            .to_lowercase()
            .split_whitespace()
            .any(|word| word == *filler)
    });
    if first.text.is_empty() || fillers || first.text.contains("<think>") {
        eprintln!("hushpen-llm: the cleaned text still has fillers or reasoning");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
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
    fn smoke_takes_a_model_and_optional_gpu_and_threads() {
        assert_eq!(
            parse_args(&strings(&["--smoke", "m.gguf", "--gpu", "--threads", "3"])),
            Ok(Some(Smoke {
                model: PathBuf::from("m.gguf"),
                gpu: true,
                threads: Some(3)
            }))
        );
        assert!(parse_args(&strings(&["--smoke"])).is_err());
        assert!(parse_args(&strings(&["--gpu"])).is_err());
        assert!(parse_args(&strings(&["--threads", "x", "--smoke", "m"])).is_err());
        assert!(parse_args(&strings(&["--bogus"])).is_err());
    }
}
