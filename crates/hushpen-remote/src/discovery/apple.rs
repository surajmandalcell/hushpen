#![allow(unsafe_code)]

use super::{instance_name, service_info, txt_record};
use crate::invalid;
use hushpen_core::remote::Device;
use std::ffi::{CString, c_char, c_int, c_short, c_uint, c_void};
use std::io;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

type ServiceRef = *mut c_void;
type RegisterReply = unsafe extern "C" fn(
    ServiceRef,
    u32,
    i32,
    *const c_char,
    *const c_char,
    *const c_char,
    *mut c_void,
);

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

#[link(name = "System")]
unsafe extern "C" {
    fn DNSServiceRegister(
        reference: *mut ServiceRef,
        flags: u32,
        interface: u32,
        name: *const c_char,
        service_type: *const c_char,
        domain: *const c_char,
        host: *const c_char,
        port: u16,
        txt_len: u16,
        txt: *const c_void,
        callback: RegisterReply,
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceRefSockFD(reference: ServiceRef) -> c_int;
    fn DNSServiceProcessResult(reference: ServiceRef) -> i32;
    fn DNSServiceRefDeallocate(reference: ServiceRef);
    fn poll(fds: *mut PollFd, count: c_uint, timeout: c_int) -> c_int;
}

pub struct Advertisement {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Advertisement {
    pub fn new(device: &Device, hostname: &str, port: u16, version: &str) -> io::Result<Self> {
        let info = service_info(device, hostname, port, version)?;
        let txt = txt_record(&info)?;
        let name =
            CString::new(instance_name(&device.id)).map_err(|_| invalid("invalid discovery id"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::clone(&stop);
        let (ready, result) = mpsc::sync_channel(1);
        let deadline = Instant::now() + Duration::from_secs(10);
        let thread = std::thread::Builder::new()
            .name("phone-bonjour".into())
            .spawn(move || {
                if let Err(error) = serve(&name, port, &txt, &running, deadline, &ready) {
                    let _ = ready.try_send(Err(error));
                }
            })?;
        let mut advertisement = Self {
            stop,
            thread: Some(thread),
        };
        match result.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(())) => Ok(advertisement),
            Ok(Err(error)) => {
                advertisement.close();
                Err(error)
            }
            Err(error) => {
                advertisement.close();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "Bonjour registration did not finish. Allow Local Network in Settings and start Server mode again: {error}"
                    ),
                ))
            }
        }
    }

    fn close(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        self.close();
    }
}

struct Registered(ServiceRef);

impl Drop for Registered {
    fn drop(&mut self) {
        // SAFETY: This thread owns the reference and has stopped processing callbacks.
        unsafe { DNSServiceRefDeallocate(self.0) };
    }
}

fn serve(
    name: &CString,
    port: u16,
    txt: &[u8],
    stop: &AtomicBool,
    deadline: Instant,
    ready: &mpsc::SyncSender<io::Result<()>>,
) -> io::Result<()> {
    let mut reference = ptr::null_mut();
    let mut status: Option<i32> = None;
    let txt_len =
        u16::try_from(txt.len()).map_err(|_| invalid("discovery TXT record is too long"))?;
    // SAFETY: All inputs are live for the call. Bonjour copies the strings and TXT bytes.
    // The callback context stays on this thread until the reference is deallocated.
    let error = unsafe {
        DNSServiceRegister(
            &mut reference,
            0,
            0,
            name.as_ptr(),
            c"_hushpen._tcp".as_ptr(),
            c"local.".as_ptr(),
            ptr::null(),
            port.to_be(),
            txt_len,
            txt.as_ptr().cast(),
            registered,
            (&mut status as *mut Option<i32>).cast(),
        )
    };
    check(error)?;
    if reference.is_null() {
        return Err(io::Error::other("Bonjour returned no registration"));
    }
    let service = Registered(reference);
    // SAFETY: The reference was initialized by a successful registration.
    let fd = unsafe { DNSServiceRefSockFD(service.0) };
    if fd < 0 {
        return Err(io::Error::other("Bonjour returned no event socket"));
    }
    let mut announced = false;
    while !stop.load(Ordering::Acquire) {
        if !announced && Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Bonjour registration timed out. Allow Local Network in Settings.",
            ));
        }
        let mut socket = PollFd {
            fd,
            events: 0x0001,
            revents: 0,
        };
        // SAFETY: PollFd matches the iOS SDK's pollfd, and one live record is supplied.
        let events = unsafe { poll(&mut socket, 1, 100) };
        if events < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if socket.revents & (0x0008 | 0x0010 | 0x0020) != 0 {
            return Err(io::Error::other("Bonjour event socket closed"));
        }
        if socket.revents & 0x0001 != 0 {
            // SAFETY: Poll confirmed readable data. Only this thread uses the live reference.
            check(unsafe { DNSServiceProcessResult(service.0) })?;
            if let Some(error) = status.take() {
                check(error)?;
                if !announced {
                    ready.send(Ok(())).map_err(io::Error::other)?;
                    announced = true;
                }
            }
        }
    }
    Ok(())
}

unsafe extern "C" fn registered(
    _: ServiceRef,
    _: u32,
    error: i32,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    context: *mut c_void,
) {
    // SAFETY: serve supplied this Option and processes callbacks on the same thread.
    unsafe { *context.cast::<Option<i32>>() = Some(error) };
}

fn check(error: i32) -> io::Result<()> {
    match error {
        0 => Ok(()),
        -65570 => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Allow Hushpen Local Network access in Settings, then start Server mode again.",
        )),
        error => Err(io::Error::other(format!(
            "Bonjour registration failed ({error})"
        ))),
    }
}
