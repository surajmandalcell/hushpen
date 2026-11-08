#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, w};

fn wide(text: &OsStr) -> std::io::Result<Vec<u16>> {
    let mut value: Vec<u16> = text.encode_wide().collect();
    if value.contains(&0) {
        return Err(std::io::Error::other(
            "the open target contains a null character",
        ));
    }
    value.push(0);
    Ok(value)
}

fn check_url(url: &str) -> std::io::Result<()> {
    let lower = url.to_ascii_lowercase();
    if ["https://", "http://", "ms-settings:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "only http, https, or ms-settings addresses can be opened",
        ))
    }
}

fn open(file: &OsStr, parameters: Option<&OsStr>) -> std::io::Result<()> {
    let file = wide(file)?;
    let parameters = parameters.map(wide).transpose()?;
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: parameters
            .as_ref()
            .map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr())),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: all strings stay live through the call and the structure has the required size.
    unsafe { ShellExecuteExW(&mut info) }.map_err(std::io::Error::other)
}

pub(super) fn open_url(url: &str) -> std::io::Result<()> {
    check_url(url)?;
    open(OsStr::new(url), None)
}

pub(super) fn open_path(path: &Path) -> std::io::Result<()> {
    let path = std::path::absolute(path)?;
    open(path.as_os_str(), None)
}

pub(super) fn reveal_path(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        return open_path(path);
    }
    let path = std::path::absolute(path)?;
    let parameters = format!("/select,\"{}\"", path.display());
    open(OsStr::new("explorer.exe"), Some(OsStr::new(&parameters)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_settings_urls_are_allowed() {
        for url in [
            "https://hushpen.mandalsuraj.com",
            "HTTP://localhost",
            "ms-settings:privacy-microphone",
        ] {
            assert!(check_url(url).is_ok());
        }
        for url in [
            "file:///C:/Windows",
            "shell:startup",
            "--help",
            "",
            "ftp://host",
        ] {
            assert!(check_url(url).is_err());
        }
        assert!(wide(OsStr::new("bad\0path")).is_err());
    }
}
