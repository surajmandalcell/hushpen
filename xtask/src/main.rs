//! Repo tasks, run as `cargo xtask <task>`.

mod gate;
mod licenses;
mod net_audit;
mod package;
mod release_check;
mod root;
mod two_binary;
mod update_feed;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask <gate|net-audit|licenses|two-binary|release-check [--binary <path>] [--features <list>] [--guard]|package [...]|package-verify [...]|update-test-keygen --out <dir> [--name <name>]|update-feed --out <dir> --key <file> --version <v> [--notes-url <url>] [--tamper] [--artifact <target>=<file>]... [--base-url <url>] [--wrong-sha]>";

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
        Some("licenses") => licenses::run(&root),
        Some("two-binary") => two_binary::run(&root),
        Some("release-check") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            release_check::run(&root, &rest)
        }
        Some("package") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            package::run(&root, &rest)
        }
        Some("package-verify") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            package::run_verify(&root, &rest)
        }
        Some("update-test-keygen") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            update_feed::run_keygen(&rest)
        }
        Some("update-feed") => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            update_feed::run_feed(&rest)
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
