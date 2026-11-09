//! `cargo xtask gate`: the checks every change must pass, in fail-fast order.

use std::path::Path;
use std::process::Command;

use crate::{net_audit, two_binary};

/// Parallelism the gate asks for unless the caller already set it. The
/// owner's Mac also runs a desktop and test containers while the gate runs.
const DEFAULT_BUILD_JOBS: &str = "4";
const DEFAULT_TEST_THREADS: &str = "4";

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Cargo(&'static [&'static str]),
    NetAudit,
    TwoBinary,
    Programs,
}

impl Step {
    pub fn display(&self) -> String {
        match self {
            Step::Cargo(args) => format!("cargo {}", args.join(" ")),
            Step::NetAudit => "cargo xtask net-audit".to_string(),
            Step::TwoBinary => "cargo xtask two-binary".to_string(),
            Step::Programs => "run the four Windows programs with --version".to_string(),
        }
    }
}

pub fn steps(windows: bool) -> Vec<Step> {
    let mut steps = vec![
        Step::Cargo(&["fmt", "--all", "--check"]),
        Step::TwoBinary,
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
    ];
    if windows {
        steps.push(Step::Cargo(&[
            "build",
            "--locked",
            "-p",
            "hushpen-app",
            "-p",
            "hushpen-engine",
            "-p",
            "hushpen-llm",
            "-p",
            "hushpen-tts",
        ]));
        steps.push(Step::Programs);
    }
    steps
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
    #[cfg(target_os = "windows")]
    park_running_exe(
        &std::env::current_exe().map_err(|error| format!("cannot find xtask: {error}"))?,
    )?;

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let steps =
        steps(crate::package::names::Target::host()? == crate::package::names::Target::WindowsX64);
    for (index, step) in steps.iter().enumerate() {
        println!("==> [{}/{}] {}", index + 1, steps.len(), step.display());
        match step {
            Step::Cargo(args) => run_cargo(&cargo, root, args)?,
            Step::NetAudit => net_audit::run(root)?,
            Step::TwoBinary => two_binary::run(root)?,
            Step::Programs => {
                for name in ["hushpen", "hushpen-asr", "hushpen-llm", "hushpen-tts"] {
                    crate::package::run_tool(
                        Command::new(
                            crate::package::target_dir(root)
                                .join("debug")
                                .join(format!("{name}.exe")),
                        )
                        .arg("--version"),
                        &format!("{name}.exe --version"),
                    )?;
                }
            }
        }
    }
    println!("==> gate passed");
    Ok(())
}

#[cfg(target_os = "windows")]
fn park_running_exe(exe: &Path) -> Result<(), String> {
    use std::fs;
    let parked = exe.with_file_name("xtask-gate-running.exe");
    match fs::remove_file(&parked) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "cannot remove the previous gate executable: {error}"
            ));
        }
    }
    // Windows can rename a loaded image, but Cargo cannot overwrite that image's file.
    fs::rename(exe, &parked).map_err(|error| format!("cannot park the running xtask: {error}"))?;
    if let Err(error) = fs::copy(&parked, exe) {
        let restored = match fs::remove_file(exe) {
            Ok(()) => fs::rename(&parked, exe),
            Err(remove) if remove.kind() == std::io::ErrorKind::NotFound => {
                fs::rename(&parked, exe)
            }
            Err(remove) => Err(remove),
        };
        return Err(match restored {
            Ok(()) => format!("cannot copy xtask back for Cargo (original restored): {error}"),
            Err(restore) => format!(
                "cannot copy xtask back for Cargo: {error}; original at {} could not be restored: {restore}",
                parked.display()
            ),
        });
    }
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

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "child process helper for the running-executable parking test"]
    fn parking_child() {
        let Some(dir) = std::env::var_os("HUSHPEN_GATE_PARK_TEST_DIR") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::write(dir.join("ready"), "").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !dir.join("stop").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "parking test parent did not stop the child"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parking_a_running_executable_leaves_cargos_output_replaceable() {
        use std::fs;
        use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
        let root = crate::root::repo_root().unwrap();
        let dir = crate::package::target_dir(&root).join(format!(
            "gate-parking-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("xtask.exe");
        fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let mut child = Command::new(&exe)
            .args(["--ignored", "--exact", "gate::tests::parking_child"])
            .env("HUSHPEN_GATE_PARK_TEST_DIR", &dir)
            .spawn()
            .unwrap();
        let result = (|| -> Result<(), String> {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !dir.join("ready").exists() {
                if child
                    .try_wait()
                    .map_err(|error| error.to_string())?
                    .is_some()
                    || Instant::now() >= deadline
                {
                    return Err("the parking helper did not become ready".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            fs::write(dir.join("xtask-gate-running.exe"), "previous gate")
                .map_err(|error| error.to_string())?;
            park_running_exe(&exe)?;
            fs::write(&exe, "new Cargo output").map_err(|error| error.to_string())?;
            if fs::read(&exe).map_err(|error| error.to_string())? != b"new Cargo output" {
                return Err("Cargo's output did not change".into());
            }
            fs::write(dir.join("stop"), "").map_err(|error| error.to_string())?;
            loop {
                if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                    return status
                        .success()
                        .then_some(())
                        .ok_or_else(|| format!("parking helper failed: {status}"));
                }
                if Instant::now() >= deadline {
                    return Err("the parking helper did not stop".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })();
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while child.try_wait().unwrap().is_none() {
                assert!(
                    Instant::now() < deadline,
                    "the parking helper did not terminate"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        fs::remove_dir_all(&dir).unwrap();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn gate_runs_the_required_commands_with_the_two_binary_check_and_net_audit() {
        let shown: Vec<String> = steps(false).iter().map(Step::display).collect();
        assert_eq!(
            shown,
            [
                "cargo fmt --all --check",
                "cargo xtask two-binary",
                "cargo clippy --workspace --all-targets --locked -- -D warnings",
                "cargo test --workspace --locked",
                "cargo deny check",
                "cargo xtask net-audit",
            ]
        );
    }

    #[test]
    fn windows_gate_builds_and_loads_all_four_programs() {
        let windows = steps(true);
        let common = steps(false);
        assert_eq!(&windows[..common.len()], common);
        assert_eq!(windows.len(), common.len() + 2);
        assert!(matches!(windows.last(), Some(Step::Programs)));
        let Step::Cargo(build) = &windows[common.len()] else {
            panic!("missing Windows build")
        };
        assert_eq!(build[0], "build");
        for package in [
            "hushpen-app",
            "hushpen-engine",
            "hushpen-llm",
            "hushpen-tts",
        ] {
            assert!(build.contains(&package));
        }
    }

    #[test]
    fn target_dir_inside_the_repo_is_refused() {
        let root = std::env::temp_dir().join("hushpen-gate-repo");
        let target = root.join("target");
        assert!(target_dir_problem(&root, Some(target.to_str().unwrap()), false).is_some());
        assert!(target_dir_problem(&root, None, false).is_some());
        assert!(target_dir_problem(&root, Some("target"), false).is_some());
    }

    #[test]
    fn target_dir_elsewhere_or_on_ci_is_accepted() {
        let base = std::env::temp_dir();
        let root = base.join("hushpen-gate-repo");
        let target = base.join("hushpen-target");
        assert!(target_dir_problem(&root, Some(target.to_str().unwrap()), false).is_none());
        assert!(target_dir_problem(&root, None, true).is_none());
    }
}
