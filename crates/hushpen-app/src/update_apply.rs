//! `hushpen update-apply`: the small helper that puts a staged update in place and starts the
//! new version.
//!
//! The app copies its own binary beside the install and runs that copy with `update-apply`,
//! then quits. The copy waits for the app to exit, swaps the staged file or bundle into the
//! install path, and relaunches. The swap never leaves the install path empty or half written:
//! a file is replaced with one atomic rename, and a failed swap leaves the old version running.

use hushpen_core::error::UPDATE_SWAP_FAILED;
use hushpen_core::update::InstallKind;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "usage: hushpen update-apply --pid <pid> --kind <appimage|app> --target <path> \
--staged <path> [--no-relaunch] [--cleanup <path>]...";

/// How long the helper waits for the app to quit before it gives up and swaps nothing.
const QUIT_TIMEOUT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);

/// Environment the AppImage runtime sets for the old process. A new run sets its own.
const APPIMAGE_ENV: [&str; 4] = ["APPIMAGE", "APPDIR", "ARGV0", "OWD"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub pid: u32,
    pub kind: InstallKind,
    /// The AppImage file or the `.app` bundle that runs now.
    pub target: PathBuf,
    /// The complete new file or bundle, on the same volume as `target`.
    pub staged: PathBuf,
    pub relaunch: bool,
    /// Files and folders to remove at the end, whatever the result.
    pub cleanup: Vec<PathBuf>,
}

impl Job {
    /// The helper's command line, after `update-apply`.
    pub fn to_args(&self) -> Vec<String> {
        let mut args = vec![
            "--pid".to_owned(),
            self.pid.to_string(),
            "--kind".to_owned(),
            self.kind.key().to_owned(),
            "--target".to_owned(),
            self.target.display().to_string(),
            "--staged".to_owned(),
            self.staged.display().to_string(),
        ];
        if !self.relaunch {
            args.push("--no-relaunch".to_owned());
        }
        for path in &self.cleanup {
            args.push("--cleanup".to_owned());
            args.push(path.display().to_string());
        }
        args
    }
}

pub fn parse_args(args: &[String]) -> Result<Job, String> {
    let (mut pid, mut kind, mut target, mut staged) = (None, None, None, None);
    let (mut relaunch, mut cleanup) = (true, Vec::new());
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--pid" => {
                pid = Some(
                    value("--pid")?
                        .parse::<u32>()
                        .map_err(|_| format!("--pid needs a number\n{USAGE}"))?,
                );
            }
            "--kind" => {
                kind = Some(match InstallKind::parse(&value("--kind")?) {
                    kind @ (InstallKind::App | InstallKind::AppImage) => kind,
                    _ => return Err(format!("--kind is appimage or app\n{USAGE}")),
                });
            }
            "--target" => target = Some(PathBuf::from(value("--target")?)),
            "--staged" => staged = Some(PathBuf::from(value("--staged")?)),
            "--cleanup" => cleanup.push(PathBuf::from(value("--cleanup")?)),
            "--no-relaunch" => relaunch = false,
            other => return Err(format!("unknown argument {other:?}\n{USAGE}")),
        }
    }
    let missing = |name: &str| format!("{name} is required\n{USAGE}");
    Ok(Job {
        pid: pid.ok_or_else(|| missing("--pid"))?,
        kind: kind.ok_or_else(|| missing("--kind"))?,
        target: target.ok_or_else(|| missing("--target"))?,
        staged: staged.ok_or_else(|| missing("--staged"))?,
        relaunch,
        cleanup,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapError(pub String);

impl std::fmt::Display for SwapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// `<target>.old`, the copy of the running version that exists only during a swap.
pub fn backup_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".old");
    target.with_file_name(name)
}

/// Replaces the file at `target` with the complete file at `staged`.
///
/// The old file is kept under a second name by a hard link, then one atomic rename puts the new
/// file on the target path. At every instant the path holds a whole file, the old one or the
/// new one, so a kill in the middle leaves a version that starts. A failure removes the
/// second name and leaves the old file where it was.
pub fn swap_file(target: &Path, staged: &Path) -> Result<(), SwapError> {
    swap_file_with(target, staged, pause_for_tests)
}

pub(crate) fn swap_file_with(
    target: &Path,
    staged: &Path,
    between: impl FnOnce(),
) -> Result<(), SwapError> {
    let backup = backup_path(target);
    let _ = fs::remove_file(&backup);
    let old = fs::metadata(target)
        .map_err(|error| SwapError(format!("cannot read the installed file: {error}")))?;
    fs::set_permissions(staged, old.permissions())
        .map_err(|error| SwapError(format!("cannot set the mode of the new file: {error}")))?;
    // A volume without hard links gets a copy; the rename below is the step that matters.
    if fs::hard_link(target, &backup).is_err() {
        fs::copy(target, &backup)
            .map_err(|error| SwapError(format!("cannot keep the old file: {error}")))?;
    }
    between();
    if let Err(error) = fs::rename(staged, target) {
        if !target.exists() {
            let _ = fs::rename(&backup, target);
        } else {
            let _ = fs::remove_file(&backup);
        }
        return Err(SwapError(format!(
            "cannot put the new file in place: {error}"
        )));
    }
    let _ = fs::remove_file(&backup);
    Ok(())
}

/// Replaces the bundle folder at `target` with the complete bundle at `staged`. A folder cannot
/// be renamed over another, so the old bundle moves aside first and moves back if the second
/// rename fails.
pub fn swap_bundle(target: &Path, staged: &Path) -> Result<(), SwapError> {
    let backup = backup_path(target);
    let _ = fs::remove_dir_all(&backup);
    fs::rename(target, &backup)
        .map_err(|error| SwapError(format!("cannot move the old app aside: {error}")))?;
    if let Err(error) = fs::rename(staged, target) {
        return Err(match fs::rename(&backup, target) {
            Ok(()) => SwapError(format!("cannot put the new app in place: {error}")),
            Err(back) => SwapError(format!(
                "cannot put the new app in place: {error}; the old app is at {}: {back}",
                backup.display()
            )),
        });
    }
    let _ = fs::remove_dir_all(&backup);
    Ok(())
}

pub fn swap(kind: InstallKind, target: &Path, staged: &Path) -> Result<(), SwapError> {
    match kind {
        InstallKind::App => swap_bundle(target, staged),
        InstallKind::AppImage => swap_file(target, staged),
        other => Err(SwapError(format!(
            "{} installs are not swapped",
            other.key()
        ))),
    }
}

/// Test builds can hold the swap open between keeping the old file and replacing it, so a test
/// can kill the helper there.
fn pause_for_tests() {
    #[cfg(feature = "update-test-feed")]
    if let Some(ms) = std::env::var("HUSHPEN_UPDATE_SWAP_PAUSE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

/// Whether the process still runs. A zombie that nobody collected counts as gone.
pub fn process_alive(pid: u32) -> bool {
    if let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) {
        // The state letter follows the command name, which is in parentheses and may hold spaces.
        let state = stat
            .rsplit_once(')')
            .and_then(|(_, rest)| rest.split_whitespace().next());
        return state != Some("Z");
    }
    if Path::new("/proc/self").exists() {
        return false;
    }
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    while process_alive(pid) {
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(POLL);
    }
    true
}

fn relaunch(kind: InstallKind, target: &Path) -> Result<(), String> {
    match kind {
        InstallKind::App => Command::new("/usr/bin/open")
            .arg(target)
            .status()
            .map_err(|error| error.to_string())
            .and_then(|status| {
                status
                    .success()
                    .then_some(())
                    .ok_or_else(|| format!("open ended with {status}"))
            }),
        _ => {
            let mut command = Command::new(target);
            for name in APPIMAGE_ENV {
                command.env_remove(name);
            }
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            command
                .stdin(Stdio::null())
                .spawn()
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
    }
}

fn remove_any(path: &Path) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

fn say(line: &str) {
    eprintln!("update-apply: {line}");
}

/// Runs one job. Prints one line for each step on stderr; the app points that at a log file.
pub fn run(job: &Job) -> Result<(), SwapError> {
    say(&format!(
        "UPDATE_SWAP_WAIT pid {} kind {}",
        job.pid,
        job.kind.key()
    ));
    let quit = wait_for_exit(job.pid, QUIT_TIMEOUT);
    let result = if quit {
        swap(job.kind, &job.target, &job.staged)
    } else {
        Err(SwapError("the app did not quit in time".to_owned()))
    };
    match &result {
        Ok(()) => say("UPDATE_SWAP_DONE"),
        Err(error) => say(&format!("{UPDATE_SWAP_FAILED} {error}")),
    }
    // The old version keeps working after a failed swap, so it starts again. An app that is
    // still running is not started a second time.
    if job.relaunch && quit {
        match relaunch(job.kind, &job.target) {
            Ok(()) => say("UPDATE_RELAUNCH started"),
            Err(error) => say(&format!("UPDATE_RELAUNCH failed {error}")),
        }
    }
    for path in &job.cleanup {
        remove_any(path);
    }
    result
}

/// The `update-apply` subcommand.
pub fn main(args: &[String]) -> ExitCode {
    match parse_args(args) {
        Ok(job) => match run(&job) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(1),
        },
        Err(message) => {
            eprintln!("hushpen update-apply: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests;
