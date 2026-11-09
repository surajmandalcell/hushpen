#![allow(unsafe_code)]

use hushpen_core::permission::{Access, Preflight};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, w};

pub(super) struct WindowsPreflight;

impl Preflight for WindowsPreflight {
    fn microphone(&self) -> Access {
        let apps = consent(w!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone"
        ));
        let desktop = consent(w!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\NonPackaged"
        ));
        microphone_access(apps.as_deref(), desktop.as_deref())
    }

    fn post_event(&self) -> Access {
        Access::NotApplicable
    }

    fn listen_event(&self) -> Access {
        Access::NotApplicable
    }
}

fn consent(key: PCWSTR) -> Option<String> {
    let mut value = [0u16; 16];
    let mut bytes = size_of_val(&value) as u32;
    // SAFETY: the key and value names are terminated; the writable buffer holds `bytes` bytes.
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            w!("Value"),
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if result.is_err() {
        if result != ERROR_FILE_NOT_FOUND {
            log::debug!("the microphone consent could not be read: {result:?}");
        }
        return None;
    }
    let end = value.iter().position(|ch| *ch == 0)?;
    String::from_utf16(&value[..end]).ok()
}

fn microphone_access(apps: Option<&str>, desktop: Option<&str>) -> Access {
    let access = |value: &str| {
        if value.eq_ignore_ascii_case("Allow") {
            Some(Access::Granted)
        } else if value.eq_ignore_ascii_case("Deny") {
            Some(Access::Denied)
        } else {
            None
        }
    };
    // Desktop apps have a separate consent switch from packaged apps.
    desktop
        .and_then(access)
        .or_else(|| apps.and_then(access))
        .unwrap_or(Access::NotDetermined)
}

pub(super) fn is_elevated() -> bool {
    let read = || -> windows::core::Result<bool> {
        let mut token = HANDLE::default();
        let mut elevation = TOKEN_ELEVATION::default();
        let mut bytes = 0;
        // SAFETY: the token is opened for reading, the output is sized and aligned, and the
        // owned token handle is closed after the query on both success and failure.
        unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
            let result = GetTokenInformation(
                token,
                TokenElevation,
                Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut bytes,
            );
            let _ = CloseHandle(token);
            result?;
        }
        Ok(elevation.TokenIsElevated != 0)
    };
    read().unwrap_or_else(|error| {
        log::debug!("the process elevation could not be read: {error}");
        false
    })
}

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
    fn desktop_consent_takes_priority_over_packaged_app_consent() {
        assert_eq!(
            microphone_access(Some("Deny"), Some("Allow")),
            Access::Granted
        );
        assert_eq!(
            microphone_access(Some("Allow"), Some("Deny")),
            Access::Denied
        );
    }

    #[test]
    fn missing_or_unknown_desktop_consent_uses_the_app_consent() {
        assert_eq!(microphone_access(Some("aLLoW"), None), Access::Granted);
        assert_eq!(
            microphone_access(Some("Deny"), Some("unknown")),
            Access::Denied
        );
        assert_eq!(microphone_access(None, None), Access::NotDetermined);
        assert_eq!(
            microphone_access(Some("unknown"), Some("unknown")),
            Access::NotDetermined
        );
    }

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
