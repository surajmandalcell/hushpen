//! `cargo xtask gate`: the checks every change must pass, in fail-fast order.

use std::path::Path;
use std::process::Command;

use crate::net_audit;

/// Parallelism the gate asks for unless the caller already set it. The
/// owner's Mac also runs a desktop and test containers while the gate runs.
const DEFAULT_BUILD_JOBS: &str = "4";
const DEFAULT_TEST_THREADS: &str = "4";

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Cargo(&'static [&'static str]),
    NetAudit,
}

impl Step {
    pub fn display(&self) -> String {
        match self {
            Step::Cargo(args) => format!("cargo {}", args.join(" ")),
            Step::NetAudit => "cargo xtask net-audit".to_string(),
        }
    }
}

pub fn steps() -> Vec<Step> {
    vec![
        Step::Cargo(&["fmt", "--all", "--check"]),
        Step::Cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]),
        Step::Cargo(&["test", "--workspace", "--locked"]),
        Step::Cargo(&["deny", "check"]),
        Step::NetAudit,
    ]
}

/// Explains why build output would land inside the repo, or `None` when it
/// goes elsewhere. CI runners are exempt because they are throwaway.
pub fn target_dir_problem(root: &Path, target_dir: Option<&str>, ci: bool) -> Option<String> {
    if ci {
        return None;
    }
    let Some(dir) = target_dir.filter(|dir| !dir.is_empty()) else {
        return Some(
            "CARGO_TARGET_DIR is not set, so build output would go to <repo>/target; \
             source the mission env file first"
                .to_string(),
        );
    };
    let dir = Path::new(dir);
    if !dir.is_absolute() {
        return Some("CARGO_TARGET_DIR must be an absolute path".to_string());
    }
    dir.starts_with(root)
        .then(|| format!("CARGO_TARGET_DIR {} is inside the repo", dir.display()))
}

pub fn run(root: &Path) -> Result<(), String> {
    let target_dir = std::env::var("CARGO_TARGET_DIR").ok();
    let ci = std::env::var_os("CI").is_some();
    if let Some(problem) = target_dir_problem(root, target_dir.as_deref(), ci) {
        return Err(problem);
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let steps = steps();
    for (index, step) in steps.iter().enumerate() {
        println!("==> [{}/{}] {}", index + 1, steps.len(), step.display());
        match step {
            Step::Cargo(args) => run_cargo(&cargo, root, args)?,
            Step::NetAudit => net_audit::run(root)?,
        }
    }
    println!("==> gate passed");
    Ok(())
}

fn run_cargo(cargo: &str, root: &Path, args: &[&str]) -> Result<(), String> {
    let mut command = Command::new(cargo);
    command.args(args).current_dir(root);
    for (key, value) in [
        ("CARGO_BUILD_JOBS", DEFAULT_BUILD_JOBS),
        ("RUST_TEST_THREADS", DEFAULT_TEST_THREADS),
    ] {
        if std::env::var_os(key).is_none() {
            command.env(key, value);
        }
    }
    let status = command
        .status()
        .map_err(|error| format!("cannot start {cargo}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` failed ({status})", args.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_runs_the_four_required_commands_then_net_audit() {
        let shown: Vec<String> = steps().iter().map(Step::display).collect();
        assert_eq!(
            shown,
            [
                "cargo fmt --all --check",
                "cargo clippy --workspace --all-targets --locked -- -D warnings",
                "cargo test --workspace --locked",
                "cargo deny check",
                "cargo xtask net-audit",
            ]
        );
    }

    #[test]
    fn target_dir_inside_the_repo_is_refused() {
        let root = Path::new("/repo");
        assert!(target_dir_problem(root, Some("/repo/target"), false).is_some());
        assert!(target_dir_problem(root, None, false).is_some());
        assert!(target_dir_problem(root, Some("target"), false).is_some());
    }

    #[test]
    fn target_dir_elsewhere_or_on_ci_is_accepted() {
        let root = Path::new("/repo");
        assert!(target_dir_problem(root, Some("/data/target"), false).is_none());
        assert!(target_dir_problem(root, None, true).is_none());
    }
}
