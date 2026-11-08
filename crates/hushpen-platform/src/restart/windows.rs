//! A detached restart helper that waits for the old process handle.

#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateEventW, CreateProcessW, DETACHED_PROCESS, EVENT_MODIFY_STATE, OpenEventW, OpenProcess,
    PROCESS_INFORMATION, PROCESS_SYNCHRONIZE, STARTUPINFOW, SetEvent, WaitForMultipleObjects,
    WaitForSingleObject,
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
