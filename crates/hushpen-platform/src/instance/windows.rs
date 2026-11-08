//! A file lock and a local named pipe restricted to the current user.

#![allow(unsafe_code)]

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_NO_DATA, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, GENERIC_READ,
    GENERIC_WRITE, GetLastError, HANDLE, HLOCAL, LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE,
    OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_NOWAIT,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PeekNamedPipe,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCWSTR, PWSTR};

const REQUEST: &[u8] = b"{\"cmd\":\"show\"}\n";
const REPLY: &[u8] = b"ok\n";
const DEADLINE: Duration = Duration::from_secs(3);

pub enum Start {
    First(Instance),
    AlreadyRunning,
}

pub struct Instance {
    pipe: OwnedHandle,
    guard: Guard,
}

pub struct Guard {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    _lock: File,
}

impl Guard {
    pub fn release(&self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.release();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn hash(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes
        .into_iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
}

fn pipe_name(sid: &str, data: &Path) -> PathBuf {
    let user = hash(sid.bytes());
    let root = hash(data.as_os_str().to_string_lossy().to_lowercase().bytes());
    PathBuf::from(format!(r"\\.\pipe\hushpen-{user:016x}-{root:016x}"))
}

pub fn socket_path(data_dir: &Path) -> PathBuf {
    pipe_name(&current_sid().unwrap_or_default(), data_dir)
}

fn current_sid() -> io::Result<String> {
    // SAFETY: Token buffers are pointer-aligned; the SID and converted string are read before their storage is freed.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(io::Error::other)?;
        let mut bytes = 0;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut bytes);
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        let read = GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            bytes,
            &mut bytes,
        );
        let _ = CloseHandle(token);
        read.map_err(io::Error::other)?;
        if (bytes as usize) < size_of::<TOKEN_USER>() {
            return Err(io::Error::other("The user token has no SID."));
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut text).map_err(io::Error::other)?;
        let result = text.to_string().map_err(io::Error::other);
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        result
    }
}

fn descriptor_text(sid: &str) -> io::Result<Vec<u16>> {
    let numbers: Vec<_> = sid.split('-').collect();
    if numbers.len() < 4
        || numbers[0] != "S"
        || numbers[1] != "1"
        || numbers[2..]
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The user SID is invalid.",
        ));
    }
    Ok(format!("D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect())
}

fn create_pipe(path: &Path, sid: &str) -> io::Result<OwnedHandle> {
    use std::os::windows::ffi::OsStrExt;
    let name: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let descriptor = descriptor_text(sid)?;
    // SAFETY: All name and descriptor buffers remain live during pipe creation; no handles are inherited.
    unsafe {
        let mut security = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(descriptor.as_ptr()),
            SDDL_REVISION_1,
            &mut security,
            None,
        )
        .map_err(io::Error::other)?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security.0,
            bInheritHandle: false.into(),
        };
        let pipe = CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            256,
            256,
            1000,
            Some(&attributes),
        );
        let error = GetLastError();
        let _ = LocalFree(Some(HLOCAL(security.0)));
        if pipe.is_invalid() {
            return Err(io::Error::from_raw_os_error(error.0 as i32));
        }
        Ok(OwnedHandle::from_raw_handle(pipe.0))
    }
}

pub fn start(data_dir: &Path) -> io::Result<Start> {
    let run = data_dir.join("run");
    fs::create_dir_all(&run)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(run.join("hushpen.lock"))?;
    let sid = current_sid()?;
    let path = pipe_name(&sid, &fs::canonicalize(data_dir)?);
    match lock.try_lock() {
        Ok(()) => Ok(Start::First(Instance {
            pipe: create_pipe(&path, &sid)?,
            guard: Guard {
                stop: Arc::new(AtomicBool::new(false)),
                thread: None,
                _lock: lock,
            },
        })),
        Err(TryLockError::WouldBlock) => {
            let deadline = Instant::now() + DEADLINE;
            loop {
                match request_show(&path) {
                    Ok(()) => return Ok(Start::AlreadyRunning),
                    Err(error) if Instant::now() >= deadline => return Err(error),
                    Err(_) => std::thread::sleep(Duration::from_millis(25)),
                }
            }
        }
        Err(TryLockError::Error(error)) => Err(error),
    }
}

fn raw(pipe: &OwnedHandle) -> HANDLE {
    HANDLE(pipe.as_raw_handle())
}

fn read_request(pipe: HANDLE, stop: Option<&AtomicBool>) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut bytes = Vec::new();
    loop {
        if Instant::now() >= deadline || stop.is_some_and(|stop| stop.load(Ordering::Acquire)) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "The show request timed out.",
            ));
        }
        let mut available = 0;
        // SAFETY: The pipe handle stays open through the query and the byte count output is local.
        unsafe { PeekNamedPipe(pipe, None, 0, None, Some(&mut available), None) }
            .map_err(io::Error::other)?;
        if available == 0 {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        let mut buffer = [0u8; 64];
        let mut read = 0;
        // SAFETY: The read is bounded by the available bytes and the local buffer's length.
        unsafe {
            ReadFile(
                pipe,
                Some(&mut buffer[..available.min(64) as usize]),
                Some(&mut read),
                None,
            )
        }
        .map_err(io::Error::other)?;
        bytes.extend_from_slice(&buffer[..read as usize]);
        if bytes.len() > 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "The show request is too long.",
            ));
        }
        if bytes.contains(&b'\n') {
            return Ok(bytes);
        }
    }
}

fn write(pipe: HANDLE, bytes: &[u8]) -> io::Result<()> {
    let mut written = 0;
    // SAFETY: The pipe is connected and the small message is within its output buffer size.
    unsafe { WriteFile(pipe, Some(bytes), Some(&mut written), None) }.map_err(io::Error::other)?;
    if written as usize == bytes.len() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "The show message was incomplete.",
        ))
    }
}

fn request_show(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: The name is terminated, no handle is inherited, and the returned handle is owned once.
    let pipe = unsafe {
        let handle = CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .map_err(io::Error::other)?;
        OwnedHandle::from_raw_handle(handle.0)
    };
    write(raw(&pipe), REQUEST)?;
    if read_request(raw(&pipe), None)? == REPLY {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The show reply was invalid.",
        ))
    }
}

impl Instance {
    pub fn serve(self, on_show: impl Fn() + Send + 'static) -> Guard {
        let Instance { pipe, mut guard } = self;
        let stop = Arc::clone(&guard.stop);
        match std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    // SAFETY: This thread owns the nonblocking pipe handle until it exits.
                    let connected = unsafe { ConnectNamedPipe(raw(&pipe), None) };
                    let code = connected.as_ref().err().map(|error| error.code());
                    if connected.is_ok() || code == Some(ERROR_PIPE_CONNECTED.to_hresult()) {
                        if read_request(raw(&pipe), Some(&stop)).is_ok_and(|bytes| bytes == REQUEST)
                        {
                            on_show();
                            let _ = write(raw(&pipe), REPLY);
                            // Keep the connection until the client consumes the reply or the request times out.
                            let deadline = Instant::now() + Duration::from_secs(1);
                            while Instant::now() < deadline && !stop.load(Ordering::Acquire) {
                                // SAFETY: A value-only query detects that the client has closed its pipe handle.
                                if unsafe { PeekNamedPipe(raw(&pipe), None, 0, None, None, None) }
                                    .is_err()
                                {
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(10));
                            }
                        }
                        // SAFETY: The completed connection is no longer used before the next accept.
                        let _ = unsafe { DisconnectNamedPipe(raw(&pipe)) };
                    } else if code == Some(ERROR_NO_DATA.to_hresult()) {
                        // SAFETY: A disconnected client leaves the pipe reusable after this reset.
                        let _ = unsafe { DisconnectNamedPipe(raw(&pipe)) };
                    } else if code != Some(ERROR_PIPE_LISTENING.to_hresult()) {
                        log::error!("The single-instance pipe failed: {connected:?}");
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }) {
            Ok(thread) => guard.thread = Some(thread),
            Err(error) => log::error!("The single-instance listener did not start: {error}"),
        }
        guard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_names_separate_users_and_data_roots() {
        let root = Path::new(r"C:\Data\Hushpen");
        let pipe = pipe_name("S-1-5-21-1000", root);
        assert!(pipe.to_string_lossy().starts_with(r"\\.\pipe\hushpen-"));
        assert_eq!(
            pipe,
            pipe_name("S-1-5-21-1000", Path::new(r"c:\data\hushpen"))
        );
        assert_ne!(pipe, pipe_name("S-1-5-21-1001", root));
        assert_ne!(pipe, pipe_name("S-1-5-21-1000", Path::new(r"C:\Other")));
    }

    #[test]
    fn the_pipe_dacl_grants_only_the_current_user() {
        let text = descriptor_text("S-1-5-21-1000").unwrap();
        assert_eq!(
            String::from_utf16(&text[..text.len() - 1]).unwrap(),
            "D:P(A;;GA;;;S-1-5-21-1000)"
        );
        for sid in [
            "",
            "S-1-5",
            "S-1-5-21)",
            "S-1-5-21-",
            "S-1-5-21-1000)(A;;GA;;;WD",
        ] {
            assert!(descriptor_text(sid).is_err(), "{sid}");
        }
    }
}
