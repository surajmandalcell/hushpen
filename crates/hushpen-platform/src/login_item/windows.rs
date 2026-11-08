#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::w;

const RUN: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

#[derive(Debug, Clone)]
pub struct WindowsLoginItem {
    exec: PathBuf,
}

fn command(exec: &Path) -> std::io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = exec.as_os_str().encode_wide().collect();
    if path.iter().any(|&c| c == 0 || c == u16::from(b'"')) {
        return Err(std::io::Error::other(
            "the login path has an invalid character",
        ));
    }
    Ok([
        vec![u16::from(b'"')],
        path,
        "\" --hidden\0".encode_utf16().collect(),
    ]
    .concat())
}

impl WindowsLoginItem {
    pub fn current() -> std::io::Result<Self> {
        Ok(Self {
            exec: std::env::current_exe()?,
        })
    }

    pub fn is_enabled(&self) -> bool {
        // SAFETY: the constant names are terminated; a size query needs no data buffer.
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN,
                w!("Hushpen"),
                RRF_RT_REG_SZ,
                None,
                None,
                None,
            )
        }
        .is_ok()
    }

    pub fn set(&self, enabled: bool) -> std::io::Result<()> {
        let result = if enabled {
            let value = command(&self.exec)?;
            // SAFETY: value is a terminated UTF-16 string and its byte length matches the buffer.
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN,
                    w!("Hushpen"),
                    REG_SZ.0,
                    Some(value.as_ptr().cast()),
                    (value.len() * 2) as u32,
                )
            }
        } else {
            // SAFETY: the operation deletes only Hushpen's named value in the current user's key.
            unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, w!("Hushpen")) }
        };
        if !enabled && result == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        result.ok().map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_quotes_the_exe_and_starts_hidden() {
        let value = command(Path::new(r"C:\Program Files\Hushpen\hushpen.exe")).unwrap();
        assert_eq!(
            String::from_utf16(&value[..value.len() - 1]).unwrap(),
            "\"C:\\Program Files\\Hushpen\\hushpen.exe\" --hidden"
        );
        assert!(command(Path::new("bad\"path")).is_err());
    }
}
