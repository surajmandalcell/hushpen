//! Repo tasks, run as `cargo xtask <task>`.

mod gate;
mod net_audit;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <gate|net-audit>";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repo root")
        .to_path_buf()
}

fn main() -> ExitCode {
    let root = repo_root();
    let result = match std::env::args().nth(1).as_deref() {
        Some("gate") => gate::run(&root),
        Some("net-audit") => net_audit::run(&root),
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
