//! Repo tasks, run as `cargo xtask <task>`.

mod gate;
mod net_audit;
mod release_check;
mod root;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <gate|net-audit|release-check [--binary <path>] [--guard]>";

fn main() -> ExitCode {
    let root = match root::repo_root() {
        Ok(root) => root,
        Err(message) => {
            eprintln!("xtask: {message}");
            return ExitCode::FAILURE;
        }
    };
    let result = match std::env::args().nth(1).as_deref() {
        Some("gate") => gate::run(&root),
        Some("net-audit") => net_audit::run(&root),
        Some("release-check") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            release_check::run(&root, &rest)
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}
