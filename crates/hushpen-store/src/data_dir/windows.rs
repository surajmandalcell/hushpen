//! Windows data paths and file access.

#![allow(unsafe_code)]

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::{GENERIC_WRITE, HANDLE, HLOCAL, LocalFree, RPC_E_CHANGED_MODE};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation,
    PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
    GetDiskFreeSpaceExW,
};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::{BOOL, PCWSTR, PWSTR};

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    wide.push(0);
    Ok(wide)
}

pub(super) fn roaming_data() -> io::Result<PathBuf> {
    // SAFETY: this initializes COM for the current thread without changing an existing apartment.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if initialized.is_err() && initialized != RPC_E_CHANGED_MODE {
        return Err(io::Error::other(windows::core::Error::from_hresult(
            initialized,
        )));
    }
    // SAFETY: the known folder id is valid. The returned allocation is released below.
    let result = unsafe { SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None) };
    let result = result.map_err(io::Error::other).and_then(|path| {
        // SAFETY: the shell returns a terminated UTF-16 string owned by this call.
        let result = PathBuf::from(OsString::from_wide(unsafe { path.as_wide() }));
        // SAFETY: the shell allocated this string with the COM task allocator.
        unsafe { CoTaskMemFree(Some(path.0.cast())) };
        if result.is_absolute() {
            Ok(result)
        } else {
            Err(io::ErrorKind::InvalidData.into())
        }
    });
    if initialized.is_ok() {
        // SAFETY: this balances only the successful CoInitializeEx call above.
        unsafe { CoUninitialize() };
    }
    result
}

pub(crate) fn free_bytes(path: &Path) -> io::Result<u64> {
    let path = wide(path)?;
    let mut available = 0;
    // SAFETY: the path is terminated and the available-byte output is writable.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(path.as_ptr()), Some(&mut available), None, None) }
        .map_err(io::Error::other)?;
    Ok(available)
}

pub(crate) fn replace(temp: &Path, path: &Path) -> io::Result<()> {
    // Rust also uses FileRenameInfoEx when MoveFileExW cannot replace an open reader's file.
    std::fs::rename(temp, path)
}

fn current_user_sid() -> io::Result<String> {
    let mut token = HANDLE::default();
    // SAFETY: the pseudo process handle is valid and token is a writable output.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(io::Error::other)?;
    // SAFETY: OpenProcessToken returned an owned handle, now owned and closed by OwnedHandle.
    let token = unsafe { OwnedHandle::from_raw_handle(token.0) };
    let handle = HANDLE(token.as_raw_handle());
    let mut length = 0;
    // SAFETY: the empty query asks only for the size of the token information buffer.
    let _ = unsafe { GetTokenInformation(handle, TokenUser, None, 0, &mut length) };
    if length < std::mem::size_of::<TOKEN_USER>() as u32 {
        return Err(io::Error::last_os_error());
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: usize storage supplies the alignment and byte length required for TOKEN_USER.
    unsafe {
        GetTokenInformation(
            handle,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            length,
            &mut length,
        )
    }
    .map_err(io::Error::other)?;
    // SAFETY: GetTokenInformation filled this aligned buffer with a TOKEN_USER and its SID.
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = PWSTR::null();
    // SAFETY: the SID stays inside storage until conversion finishes; sid is a writable output.
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) }.map_err(io::Error::other)?;
    // SAFETY: the conversion returns a terminated UTF-16 SID string owned by this call.
    let result = unsafe { sid.to_string() }.map_err(io::Error::other);
    // SAFETY: ConvertSidToStringSidW allocates its string with LocalAlloc.
    unsafe { LocalFree(Some(HLOCAL(sid.0.cast()))) };
    result
}

fn private_sddl(sid: &str) -> io::Result<String> {
    let Some(numbers) = sid.strip_prefix("S-1-") else {
        return Err(io::ErrorKind::InvalidInput.into());
    };
    if numbers.split('-').count() < 2
        || numbers
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|ch| ch.is_ascii_digit()))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(format!("D:P(A;;FA;;;{sid})"))
}

fn with_private_descriptor<T>(
    operation: impl FnOnce(PSECURITY_DESCRIPTOR) -> io::Result<T>,
) -> io::Result<T> {
    let sddl = private_sddl(&current_user_sid()?)?;
    let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the SDDL is terminated; descriptor receives a LocalAlloc-owned descriptor.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(io::Error::other)?;
    let result = operation(descriptor);
    // SAFETY: the descriptor came from the conversion above and no borrowed pointers escape.
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    result
}

pub(crate) fn create_private(path: &Path) -> io::Result<File> {
    let path = wide(path)?;
    with_private_descriptor(|descriptor| {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: BOOL(0),
        };
        // SAFETY: the path and descriptor live through CreateFileW. CREATE_NEW never opens an
        // existing file, so private access applies before any byte can be written.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_DELETE,
                Some(&attributes),
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(io::Error::other)?;
        // SAFETY: CreateFileW returned an owned file handle, transferred to File here.
        Ok(unsafe { File::from_raw_handle(handle.0) })
    })
}

pub(crate) fn copy_private(source: &Path, target: &Path) -> io::Result<u64> {
    let mut source = File::open(source)?;
    let mut target = create_private(target)?;
    let copied = io::copy(&mut source, &mut target)?;
    target.sync_all()?;
    Ok(copied)
}

pub(crate) fn tighten_private(path: &Path) -> io::Result<()> {
    let path = wide(path)?;
    with_private_descriptor(|descriptor| {
        let mut present = BOOL(0);
        let mut defaulted = BOOL(0);
        let mut dacl = std::ptr::null_mut();
        // SAFETY: descriptor is valid and all ACL outputs are writable and local to this call.
        unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) }
            .map_err(io::Error::other)?;
        if !present.as_bool() || dacl.is_null() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        // SAFETY: the filename is terminated and dacl belongs to the live descriptor. Protection
        // prevents parent-folder grants from widening this file's current-user-only access.
        unsafe {
            SetNamedSecurityInfoW(
                PCWSTR(path.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(dacl),
                None,
            )
        }
        .ok()
        .map_err(io::Error::other)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Security::Authorization::GetNamedSecurityInfoW;
    use windows::Win32::Security::{
        ACCESS_ALLOWED_ACE, GetAce, GetSecurityDescriptorControl, SE_DACL_PROTECTED,
    };

    fn assert_private(path: &Path) {
        let path = wide(path).unwrap();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let mut dacl = std::ptr::null_mut();
        // SAFETY: the path is terminated and both outputs are writable.
        unsafe {
            GetNamedSecurityInfoW(
                PCWSTR(path.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut dacl),
                None,
                &mut descriptor,
            )
        }
        .ok()
        .unwrap();
        let mut control = 0;
        let mut revision = 0;
        // SAFETY: the returned descriptor and DACL stay valid until LocalFree below.
        unsafe {
            GetSecurityDescriptorControl(descriptor, &mut control, &mut revision).unwrap();
            assert_ne!(control & SE_DACL_PROTECTED.0, 0);
            assert!(!dacl.is_null());
            assert_eq!((*dacl).AceCount, 1);
            let mut ace = std::ptr::null_mut();
            GetAce(dacl, 0, &mut ace).unwrap();
            let ace = &*ace.cast::<ACCESS_ALLOWED_ACE>();
            assert_eq!(ace.Header.AceType, 0);
            let mut sid = PWSTR::null();
            let sid_address = std::ptr::addr_of!(ace.SidStart).cast_mut().cast();
            ConvertSidToStringSidW(windows::Win32::Security::PSID(sid_address), &mut sid).unwrap();
            assert_eq!(sid.to_string().unwrap(), current_user_sid().unwrap());
            LocalFree(Some(HLOCAL(sid.0.cast())));
            LocalFree(Some(HLOCAL(descriptor.0)));
        }
    }

    #[test]
    fn private_descriptor_accepts_only_sid_numbers_and_blocks_inheritance() {
        assert_eq!(
            private_sddl("S-1-5-21-123-456-789-1001").unwrap(),
            "D:P(A;;FA;;;S-1-5-21-123-456-789-1001)"
        );
        for invalid in ["", "WD", "S-1-", "S-1-5", "S-1-5-", "S-1-5-21)(A;;FA;;;WD"] {
            assert!(private_sddl(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn paths_are_terminated_without_accepting_embedded_nulls() {
        assert_eq!(
            wide(Path::new(r"D:\Hushpen\config")).unwrap().last(),
            Some(&0)
        );
        assert!(wide(Path::new("D:\\bad\0path")).is_err());
    }

    #[test]
    fn private_file_has_one_current_user_grant_from_creation_and_after_replace() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("private.json");
        let file = create_private(&path).unwrap();
        assert_private(&path);
        drop(file);
        assert!(
            create_private(&path).is_err(),
            "an existing file must not be truncated"
        );
        crate::atomic::write_private(&path, b"first", 0o600).unwrap();
        let mut reader = File::open(&path).unwrap();
        crate::atomic::write_private(&path, b"second", 0o600).unwrap();
        let mut original = String::new();
        std::io::Read::read_to_string(&mut reader, &mut original).unwrap();
        assert_eq!(original, "first");
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert_private(&path);
    }

    #[test]
    fn tightening_an_existing_file_keeps_only_the_current_user_grant() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("private.json");
        std::fs::write(&path, b"saved").unwrap();
        tighten_private(&path).unwrap();
        assert_private(&path);
        assert_eq!(std::fs::read(&path).unwrap(), b"saved");
    }

    #[test]
    fn moving_a_data_folder_copies_secrets_with_private_access() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        let data = super::super::DataDir::open(&source).unwrap();
        let secrets = crate::secrets::Secrets::open(&data.config_dir()).unwrap();
        secrets.set_endpoint_api_key(Some("test-key")).unwrap();

        crate::data_move::move_data(&source, &target, &source).unwrap();
        crate::data_move::finish_move_cleanup(&source, &target, &source).unwrap();

        let copied = target.join("config/secrets.json");
        assert_private(&copied);
        assert_eq!(
            crate::secrets::Secrets::open(&target.join("config"))
                .unwrap()
                .endpoint_api_key()
                .as_deref(),
            Some("test-key")
        );
    }

    #[test]
    fn free_space_and_roaming_folder_use_windows_paths() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(free_bytes(tmp.path()).unwrap() > 0);
        assert!(roaming_data().unwrap().is_absolute());
    }
}
