//! Single instance per data folder.
//!
//! An exclusive lock on `run/hushpen.lock` decides who is first. The kernel
//! drops the lock when the process dies, so a crash never blocks the next
//! start. The first instance listens on `run/hushpen.sock`; a later start
//! sends `{"cmd":"show"}`, waits for the reply, and exits.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SHOW_REQUEST: &str = r#"{"cmd":"show"}"#;
const SHOW_REPLY: &str = "ok\n";
/// `sun_path` holds about 104 bytes on macOS and 108 on Linux.
const SOCKET_PATH_LIMIT: usize = 100;
/// How long a second start waits for the first one to answer.
const ASK_DEADLINE: Duration = Duration::from_secs(3);

pub enum Start {
    First(Instance),
    AlreadyRunning,
}

/// The first instance, holding the lock and the bound socket.
pub struct Instance {
    listener: UnixListener,
    guard: Guard,
}

/// Removes the socket and lock files. Dropping it also releases the lock.
pub struct Guard {
    socket: PathBuf,
    lock_path: PathBuf,
    _lock: File,
}

impl Guard {
    pub fn release(&self) {
        let _ = fs::remove_file(&self.socket);
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.release();
    }
}

pub fn socket_path(data_dir: &Path) -> PathBuf {
    let preferred = data_dir.join("run").join("hushpen.sock");
    if preferred.as_os_str().len() < SOCKET_PATH_LIMIT {
        return preferred;
    }
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    base.join(format!(
        "hushpen-{:08x}.sock",
        fnv1a(data_dir.as_os_str().as_encoded_bytes())
    ))
}

fn lock_path(socket: &Path) -> PathBuf {
    socket.with_extension("lock")
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

pub fn start(data_dir: &Path) -> io::Result<Start> {
    let socket = socket_path(data_dir);
    if let Some(parent) = socket.parent() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    let lock_path = lock_path(&socket);
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)?;
    match lock.try_lock() {
        Ok(()) => {
            // A socket file here is a leftover from a crashed instance.
            let _ = fs::remove_file(&socket);
            let listener = UnixListener::bind(&socket)?;
            Ok(Start::First(Instance {
                listener,
                guard: Guard {
                    socket,
                    lock_path,
                    _lock: lock,
                },
            }))
        }
        Err(TryLockError::WouldBlock) => {
            ask_running_instance(&socket)?;
            Ok(Start::AlreadyRunning)
        }
        Err(TryLockError::Error(error)) => Err(error),
    }
}

fn ask_running_instance(socket: &Path) -> io::Result<()> {
    let deadline = Instant::now() + ASK_DEADLINE;
    loop {
        match request_show(socket) {
            Ok(()) => return Ok(()),
            Err(error) if Instant::now() >= deadline => return Err(error),
            // The first instance may still be binding its socket.
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn request_show(socket: &Path) -> io::Result<()> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    writeln!(stream, "{SHOW_REQUEST}")?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply == SHOW_REPLY {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected reply from the running instance",
        ))
    }
}

impl Instance {
    /// Answer `show` requests on a background thread. `on_show` runs on that
    /// thread before the reply goes out, so it must only queue work.
    pub fn serve(self, on_show: impl Fn() + Send + 'static) -> Guard {
        let Instance { listener, guard } = self;
        let spawned = std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    answer(stream, &on_show);
                }
            });
        if let Err(error) = spawned {
            log::error!("single-instance listener did not start: {error}");
        }
        guard
    }
}

fn answer(mut stream: UnixStream, on_show: &impl Fn()) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut line = String::new();
    let Ok(clone) = stream.try_clone() else {
        return;
    };
    if BufReader::new(clone).read_line(&mut line).is_err() {
        return;
    }
    if line.trim() == SHOW_REQUEST {
        on_show();
        let _ = stream.write_all(SHOW_REPLY.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn scratch(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("hp-inst-{name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn first(data: &Path) -> Instance {
        match start(data).unwrap() {
            Start::First(instance) => instance,
            Start::AlreadyRunning => panic!("expected to be the first instance"),
        }
    }

    #[test]
    fn a_second_start_asks_the_first_to_show_and_does_not_become_an_instance() {
        let data = scratch("second");
        let shown = Arc::new(AtomicUsize::new(0));
        let counter = shown.clone();
        let _guard = first(&data).serve(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        assert!(matches!(start(&data).unwrap(), Start::AlreadyRunning));
        assert_eq!(
            shown.load(Ordering::SeqCst),
            1,
            "the reply comes after the show request ran"
        );
        assert!(matches!(start(&data).unwrap(), Start::AlreadyRunning));
        assert_eq!(shown.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_leftover_socket_and_lock_file_do_not_block_a_start() {
        let data = scratch("stale");
        let socket = socket_path(&data);
        fs::create_dir_all(socket.parent().unwrap()).unwrap();
        fs::write(&socket, b"left by a crashed instance").unwrap();
        fs::write(lock_path(&socket), b"").unwrap();

        let _guard = first(&data).serve(|| {});
        assert!(matches!(start(&data).unwrap(), Start::AlreadyRunning));
    }

    #[test]
    fn releasing_the_guard_lets_the_next_start_become_first() {
        let data = scratch("release");
        let guard = first(&data).serve(|| {});
        drop(guard);
        assert!(!socket_path(&data).exists(), "the socket file is removed");

        let _again = first(&data).serve(|| {});
    }

    #[test]
    fn different_data_folders_run_side_by_side() {
        let (a, b) = (scratch("side-a"), scratch("side-b"));
        let _a = first(&a).serve(|| {});
        let _b = first(&b).serve(|| {});
    }

    #[test]
    fn a_long_data_path_falls_back_to_a_short_hashed_socket_path() {
        let long = PathBuf::from(format!("/{}", "deep/".repeat(40)));
        let socket = socket_path(&long);
        assert!(socket.as_os_str().len() < SOCKET_PATH_LIMIT, "{socket:?}");
        let name = socket.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("hushpen-") && name.ends_with(".sock"),
            "{name}"
        );

        let other = PathBuf::from(format!("/{}", "other/".repeat(40)));
        assert_ne!(socket, socket_path(&other));
    }

    #[test]
    fn a_short_data_path_keeps_the_socket_in_its_run_folder() {
        assert_eq!(
            socket_path(Path::new("/data")),
            PathBuf::from("/data/run/hushpen.sock")
        );
    }
}
