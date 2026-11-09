//! A detached restart helper that waits for the old process handle.

#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, CreateEventW, CreateProcessW, DETACHED_PROCESS,
    EVENT_MODIFY_STATE, OpenEventW, OpenProcess, PROCESS_INFORMATION, PROCESS_SYNCHRONIZE,
    STARTUPINFOW, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
};
use windows::core::{PCWSTR, PWSTR};

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut value: Vec<_> = value.encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The restart path contains a null character.",
        ));
    }
    value.push(0);
    Ok(value)
}

fn quote(value: &[u16]) -> Vec<u16> {
    let mut out = vec![b'"' as u16];
    let mut slashes = 0;
    for &unit in value {
        if unit == b'\\' as u16 {
            slashes += 1;
        } else {
            out.extend(std::iter::repeat_n(
                b'\\' as u16,
                if unit == b'"' as u16 {
                    slashes * 2 + 1
                } else {
                    slashes
                },
            ));
            out.push(unit);
            slashes = 0;
        }
    }
    out.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    out.push(b'"' as u16);
    out
}

fn spawn(exe: &Path, args: &[OsString]) -> io::Result<OwnedHandle> {
    let exe = wide(exe.as_os_str())?;
    let mut command = quote(&exe[..exe.len() - 1]);
    for arg in args {
        let value = wide(arg)?;
        command.push(b' ' as u16);
        command.extend(quote(&value[..value.len() - 1]));
    }
    command.push(0);
    if command.len() > 32767 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The restart command is too long.",
        ));
    }
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: The command is mutable, all buffers stay live, and no handles are inherited by the detached child.
    unsafe {
        CreateProcessW(
            PCWSTR(exe.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            DETACHED_PROCESS,
            None,
            PCWSTR::null(),
            &startup,
            &mut process,
        )
        .map_err(io::Error::other)?;
        let _ = CloseHandle(process.hThread);
        Ok(OwnedHandle::from_raw_handle(process.hProcess.0))
    }
}

pub(super) fn start(current: &Path, target: &Path, default: &Path) -> io::Result<()> {
    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let event = format!("Local\\HushpenRestart-{pid}-{nonce}");
    let event_name = wide(OsStr::new(&event))?;
    // SAFETY: The noninherited event and process handle remain owned until the helper confirms its own handle is open.
    let (ready, _old) = unsafe {
        let ready = CreateEventW(None, true, false, PCWSTR(event_name.as_ptr()))
            .map_err(io::Error::other)?;
        let ready = OwnedHandle::from_raw_handle(ready.0);
        let old = OpenProcess(PROCESS_SYNCHRONIZE, false, pid).map_err(io::Error::other)?;
        (ready, OwnedHandle::from_raw_handle(old.0))
    };
    let args = [
        OsString::from(super::HELPER_ARG),
        OsString::from(pid.to_string()),
        OsString::from(event),
        current.as_os_str().to_owned(),
        target.as_os_str().to_owned(),
        default.as_os_str().to_owned(),
    ];
    let child = spawn(&std::env::current_exe()?, &args)?;
    // SAFETY: Both wait handles stay open throughout the bounded startup handshake.
    let result = unsafe {
        WaitForMultipleObjects(
            &[HANDLE(ready.as_raw_handle()), HANDLE(child.as_raw_handle())],
            false,
            5000,
        )
    };
    if result == WAIT_OBJECT_0 {
        Ok(())
    } else {
        Err(io::Error::other("The restart helper did not become ready."))
    }
}

fn request(args: &[String]) -> io::Result<(u32, &str, PathBuf, PathBuf, PathBuf)> {
    let [pid, event, current, target, default] = args else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The restart arguments are incomplete.",
        ));
    };
    let pid = pid
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid != 0)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "The restart process ID is invalid.",
            )
        })?;
    if !event.starts_with(&format!("Local\\HushpenRestart-{pid}-")) || event.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The restart event is invalid.",
        ));
    }
    let (current, target, default) = (
        PathBuf::from(current),
        PathBuf::from(target),
        PathBuf::from(default),
    );
    if !current.is_absolute() || !target.is_absolute() || !default.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The restart folders must be absolute.",
        ));
    }
    Ok((pid, event, current, target, default))
}

pub(super) fn wait(args: &[String]) -> io::Result<(PathBuf, PathBuf, PathBuf)> {
    let (pid, event, current, target, default) = request(args)?;
    let event = wide(OsStr::new(event))?;
    // SAFETY: The old process handle is opened before signaling readiness, and every handle is closed after the bounded wait.
    unsafe {
        let old = OpenProcess(PROCESS_SYNCHRONIZE, false, pid).map_err(io::Error::other)?;
        let old = OwnedHandle::from_raw_handle(old.0);
        let ready = OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(event.as_ptr()))
            .map_err(io::Error::other)?;
        let ready = OwnedHandle::from_raw_handle(ready.0);
        SetEvent(HANDLE(ready.as_raw_handle())).map_err(io::Error::other)?;
        if WaitForSingleObject(HANDLE(old.as_raw_handle()), 60_000) != WAIT_OBJECT_0 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The old Hushpen process did not exit.",
            ));
        }
    }
    Ok((current, target, default))
}

pub(super) fn relaunch() -> io::Result<()> {
    let _process = spawn(&std::env::current_exe()?, &[])?;
    Ok(())
}

const UPDATE_HELPER: &str = r#"param([int]$OldPid, [string]$Ready, [string]$Setup, [string]$Target, [string]$Log, [string]$Show)
$ErrorActionPreference = 'Stop'
$oldExited = $false
$setupRunning = $false
$installed = $false
$result = 1
try {
    $old = [Diagnostics.Process]::GetProcessById($OldPid)
    $null = $old.Handle
    $readyEvent = [Threading.EventWaitHandle]::OpenExisting($Ready)
    $null = $readyEvent.Set()
    $readyEvent.Dispose()
    if (-not $old.WaitForExit(60000)) { throw 'The old Hushpen process did not exit.' }
    $oldExited = $true
    $old.Dispose()
    Add-Content -LiteralPath $Log -Value 'UPDATE_SETUP_START'
    $directory = Split-Path -LiteralPath $Target
    $installer = Start-Process -FilePath $Setup -ArgumentList "/S /D=$directory" -PassThru
    # Windows PowerShell needs the handle to retain the exit code.
    $null = $installer.Handle
    $setupRunning = $true
    if (-not $installer.WaitForExit(600000)) { throw 'Setup did not exit within ten minutes.' }
    $setupRunning = $false
    if ($installer.ExitCode -ne 0) { throw "Setup ended with code $($installer.ExitCode)." }
    $installer.Dispose()
    $installed = $true
    $result = 0
    Add-Content -LiteralPath $Log -Value 'UPDATE_SETUP_DONE'
} catch {
    Add-Content -LiteralPath $Log -Value "UPDATE_SWAP_FAILED $_"
} finally {
    if ($oldExited -and -not $setupRunning) {
        $arguments = if ($Show -eq '1') { '--show-window' } else { '--hidden' }
        Start-Process -FilePath $Target -ArgumentList $arguments
    }
    if ($installed) {
        Remove-Item -LiteralPath $Setup, $PSCommandPath -Force
        $directory = Split-Path -LiteralPath $Setup
        if (-not (Get-ChildItem -LiteralPath $directory -Force)) {
            Remove-Item -LiteralPath $directory -Force
        }
    }
}
exit $result
"#;

fn powershell() -> io::Result<PathBuf> {
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|root| root.is_absolute())
        .ok_or_else(|| io::Error::other("The Windows system folder is unavailable."))?;
    Ok(root.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"))
}

fn update_log_tail(path: &Path) -> String {
    let read = || -> io::Result<Vec<u8>> {
        let mut file = fs::File::open(path)?;
        let offset = file.metadata()?.len().saturating_sub(4096);
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        file.take(4096).read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    match read() {
        Ok(bytes) => String::from_utf8_lossy(&bytes).trim().to_owned(),
        Err(error) => format!("unavailable ({error})"),
    }
}

pub(super) fn apply_update(setup: &Path, target: &Path, log: &Path, show: bool) -> io::Result<()> {
    apply_update_for_pid(std::process::id(), setup, target, log, show)
}

fn apply_update_for_pid(
    pid: u32,
    setup: &Path,
    target: &Path,
    log: &Path,
    show: bool,
) -> io::Result<()> {
    for path in [setup, target, log] {
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Update paths must be absolute.",
            ));
        }
        wide(path.as_os_str())?;
    }
    if pid == 0 || !setup.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The setup file or process ID is invalid.",
        ));
    }
    let script = setup.with_file_name("apply-update.ps1");
    fs::write(&script, UPDATE_HELPER)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let event = format!("Local\\HushpenUpdate-{pid}-{nonce}");
    let event_name = wide(OsStr::new(&event))?;
    // SAFETY: The event and child handles remain owned during the bounded startup handshake.
    let ready = unsafe {
        let ready = CreateEventW(None, true, false, PCWSTR(event_name.as_ptr()))
            .map_err(io::Error::other)?;
        OwnedHandle::from_raw_handle(ready.0)
    };
    let args = [
        OsString::from("-NoProfile"),
        OsString::from("-NonInteractive"),
        OsString::from("-ExecutionPolicy"),
        OsString::from("Bypass"),
        OsString::from("-File"),
        script.into_os_string(),
        OsString::from(pid.to_string()),
        OsString::from(event),
        setup.as_os_str().to_owned(),
        target.as_os_str().to_owned(),
        log.as_os_str().to_owned(),
        OsString::from(if show { "1" } else { "0" }),
    ];
    let startup_path = log.with_file_name("update-startup.log");
    let startup_log = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&startup_path)?;
    // A console-free PowerShell child outlives its parent without DETACHED_PROCESS.
    let mut child = Command::new(powershell()?)
        .args(args)
        .creation_flags((CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP).0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(startup_log))
        .spawn()?;
    // SAFETY: Both handles stay open until the helper opens the old process or exits.
    let result = unsafe {
        WaitForMultipleObjects(
            &[HANDLE(ready.as_raw_handle()), HANDLE(child.as_raw_handle())],
            false,
            10_000,
        )
    };
    if result == WAIT_OBJECT_0 {
        Ok(())
    } else {
        let mut status = child.try_wait();
        if !matches!(status, Ok(Some(_))) {
            let _ = child.kill();
            // SAFETY: The process handle stays valid until the bounded cleanup wait ends.
            unsafe {
                let _ = WaitForSingleObject(HANDLE(child.as_raw_handle()), 5000);
            }
            status = child.try_wait();
        }
        Err(io::Error::other(format!(
            "The update helper did not become ready (wait {}, exit {status:?}). Startup: {}. Update: {}",
            result.0,
            update_log_tail(&startup_path),
            update_log_tail(log),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_arguments_escape_quotes_and_trailing_backslashes() {
        for (value, expected) in [
            ("", "\"\""),
            ("hello world", "\"hello world\""),
            (r"C:\Data\", "\"C:\\Data\\\\\""),
            ("a\"b", "\"a\\\"b\""),
        ] {
            let value: Vec<_> = value.encode_utf16().collect();
            assert_eq!(String::from_utf16(&quote(&value)).unwrap(), expected);
        }
        assert!(wide(OsStr::new("bad\0path")).is_err());
    }

    #[test]
    fn restart_requests_require_a_process_event_and_absolute_folders() {
        let args = |pid: &str, event: &str, folder: &str| {
            vec![
                pid.into(),
                event.into(),
                folder.into(),
                r"C:\Target".into(),
                r"C:\Default".into(),
            ]
        };
        assert!(request(&args("42", "Local\\HushpenRestart-42-1", r"C:\Current")).is_ok());
        assert!(request(&args("0", "Local\\HushpenRestart-0-1", r"C:\Current")).is_err());
        assert!(request(&args("42", "Local\\HushpenRestart-43-1", r"C:\Current")).is_err());
        assert!(request(&args("42", "Local\\HushpenRestart-42-1", "relative")).is_err());
        assert!(request(&[]).is_err());
    }
}

#[cfg(test)]
mod update_tests {
    use super::*;

    #[test]
    fn update_errors_read_only_the_last_four_kibibytes_of_each_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("startup.log");
        fs::write(&path, format!("{}tail", "x".repeat(5000))).unwrap();
        let tail = update_log_tail(&path);
        assert_eq!(tail.len(), 4096);
        assert!(tail.ends_with("tail"));
        assert!(update_log_tail(&dir.path().join("missing")).starts_with("unavailable"));
    }

    #[test]
    fn the_helper_is_valid_powershell_and_refuses_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("helper.ps1");
        fs::write(&script, UPDATE_HELPER).unwrap();
        let output = Command::new(powershell().unwrap())
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg("$errors = $null; $null = [Management.Automation.Language.Parser]::ParseFile($env:HUSHPEN_HELPER_TEST_PATH, [ref]$null, [ref]$errors); if ($errors.Count) { $errors; exit 1 }")
            .env("HUSHPEN_HELPER_TEST_PATH", &script)
            .output().unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(apply_update(Path::new("relative.exe"), &script, &script, false).is_err());
    }

    #[test]
    fn the_helper_waits_then_logs_a_failed_setup_without_replacing_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::env::var_os("SystemRoot").unwrap();
        let setup = dir.path().join("test-setup.exe");
        fs::copy(PathBuf::from(&root).join(r"System32\whoami.exe"), &setup).unwrap();
        let target = dir.path().join("missing-hushpen.exe");
        let log = dir.path().join("update.log");
        let mut old = Command::new(PathBuf::from(root).join(r"System32\ping.exe"))
            .args(["-n", "61", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let prepared = apply_update_for_pid(old.id(), &setup, &target, &log, false);
        let started_early = log.exists();
        if old.try_wait().unwrap().is_none() {
            old.kill().unwrap();
        }
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while old.try_wait().unwrap().is_none() {
            assert!(
                std::time::Instant::now() < until,
                "the old test process did not stop"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        prepared.unwrap_or_else(|error| panic!("{error}"));
        assert!(
            !started_early,
            "setup starts only after the old process exits"
        );
        let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if fs::read_to_string(&log).is_ok_and(|text| text.contains("UPDATE_SWAP_FAILED")) {
                break;
            }
            assert!(
                std::time::Instant::now() < until,
                "the helper must report setup failure"
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(!target.exists());
    }
}
