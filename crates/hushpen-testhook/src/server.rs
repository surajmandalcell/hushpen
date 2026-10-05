//! The hook's listener. One thread accepts, one thread serves each client.

use crate::endpoint::Endpoint;
use crate::protocol::{Request, Response};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

const MAX_LINE: usize = 1 << 20;

/// Answers one request. It runs on a client thread, so it may block while it
/// waits for the UI thread.
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, request: Request) -> Response;
}

pub struct HookServer {
    endpoint: Endpoint,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl HookServer {
    pub fn start(endpoint: Endpoint, handler: Arc<dyn Handler>) -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let accept = match &endpoint {
            Endpoint::Unix(path) => {
                let listener = bind_unix(path)?;
                spawn_accept(stop.clone(), handler, move || {
                    let (stream, _) = listener.accept()?;
                    let writer = stream.try_clone()?;
                    Ok((stream, writer))
                })?
            }
            Endpoint::Tcp(port) => {
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, *port))?;
                spawn_accept(stop.clone(), handler, move || {
                    let (stream, _) = listener.accept()?;
                    let writer = stream.try_clone()?;
                    Ok((stream, writer))
                })?
            }
        };
        Ok(Self {
            endpoint,
            stop,
            accept: Some(accept),
        })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

impl Drop for HookServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocked accept so the thread sees the flag. If nothing can
        // connect (the socket file is already gone) the thread stays blocked,
        // so it is left to die with the process instead of joined.
        let woke = match &self.endpoint {
            Endpoint::Unix(path) => UnixStream::connect(path).is_ok(),
            Endpoint::Tcp(port) => TcpStream::connect((Ipv4Addr::LOCALHOST, *port)).is_ok(),
        };
        if let Some(accept) = self.accept.take()
            && woke
        {
            let _ = accept.join();
        }
        if let Endpoint::Unix(path) = &self.endpoint {
            let _ = fs::remove_file(path);
        }
    }
}

fn bind_unix(path: &Path) -> io::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    // A file here is a leftover from a run that did not exit cleanly.
    let _ = fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

fn spawn_accept<R, W>(
    stop: Arc<AtomicBool>,
    handler: Arc<dyn Handler>,
    accept: impl Fn() -> io::Result<(R, W)> + Send + 'static,
) -> io::Result<JoinHandle<()>>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    thread::Builder::new()
        .name("testhook-accept".into())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let Ok((reader, writer)) = accept() else {
                    continue;
                };
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let handler = handler.clone();
                let _ = thread::Builder::new()
                    .name("testhook-client".into())
                    .spawn(move || serve(reader, writer, handler.as_ref()));
            }
        })
}

fn serve<R: Read, W: Write>(reader: R, mut writer: W, handler: &dyn Handler) {
    let mut reader = BufReader::new(reader);
    loop {
        let mut line = String::new();
        match (&mut reader).take(MAX_LINE as u64).read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if !line.ends_with('\n') && line.len() >= MAX_LINE {
            let _ = writeln!(
                writer,
                "{}",
                Response::error("request line too long").to_line()
            );
            return;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let response = match Request::parse(line) {
            Ok(request) => handler.handle(request),
            Err(error) => Response::error(error),
        };
        if writeln!(writer, "{}", response.to_line()).is_err() || writer.flush().is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client;
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::Duration;

    struct Echo;
    impl Handler for Echo {
        fn handle(&self, request: Request) -> Response {
            if request.cmd == "fail" {
                Response::error("asked to fail")
            } else {
                Response::Ok(json!({"cmd": request.cmd, "args": request.args}))
            }
        }
    }

    /// Removed on drop. Declare it before the server so the server goes first.
    struct Scratch(PathBuf);

    impl Scratch {
        fn join(&self, part: &str) -> PathBuf {
            self.0.join(part)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("hp-hook-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn ask(endpoint: &Endpoint, request: &Request) -> Response {
        client::send(endpoint, request, Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn the_socket_answers_requests_and_carries_arguments() {
        let dir = scratch("answers");
        let endpoint = Endpoint::Unix(dir.join("run").join("hook.sock"));
        let _server = HookServer::start(endpoint.clone(), Arc::new(Echo)).unwrap();

        let reply = ask(&endpoint, &Request::new("click").with("id", "sidebar.home"));
        assert_eq!(
            reply,
            Response::Ok(json!({"cmd": "click", "args": {"id": "sidebar.home"}}))
        );
        assert_eq!(
            ask(&endpoint, &Request::new("fail")),
            Response::error("asked to fail")
        );
    }

    #[test]
    fn the_socket_is_private_to_the_user() {
        let dir = scratch("private");
        let path = dir.join("run").join("hook.sock");
        let _server = HookServer::start(Endpoint::Unix(path.clone()), Arc::new(Echo)).unwrap();
        let mode = |p: &std::path::Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }

    #[test]
    fn malformed_lines_get_an_error_and_the_connection_stays_usable() {
        let dir = scratch("malformed");
        let path = dir.join("hook.sock");
        let _server = HookServer::start(Endpoint::Unix(path.clone()), Arc::new(Echo)).unwrap();
        let mut stream = UnixStream::connect(&path).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        writeln!(stream, "not json").unwrap();
        writeln!(stream, r#"{{"cmd":"state"}}"#).unwrap();
        let mut reader = BufReader::new(stream);
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        assert!(matches!(Response::parse(&first), Ok(Response::Err(_))));
        let mut second = String::new();
        reader.read_line(&mut second).unwrap();
        assert!(matches!(Response::parse(&second), Ok(Response::Ok(_))));
    }

    #[test]
    fn dropping_the_server_removes_the_socket_file() {
        let dir = scratch("drop");
        let path = dir.join("hook.sock");
        let server = HookServer::start(Endpoint::Unix(path.clone()), Arc::new(Echo)).unwrap();
        assert!(path.exists());
        drop(server);
        assert!(!path.exists());
    }

    #[test]
    fn a_leftover_socket_file_does_not_block_a_new_server() {
        let dir = scratch("stale");
        let path = dir.join("hook.sock");
        fs::write(&path, b"stale").unwrap();
        let _server = HookServer::start(Endpoint::Unix(path.clone()), Arc::new(Echo)).unwrap();
        assert_eq!(
            ask(&Endpoint::Unix(path), &Request::new("state")),
            Response::Ok(json!({"cmd": "state", "args": {}}))
        );
    }

    #[test]
    fn dropping_the_server_after_its_socket_file_vanished_does_not_hang() {
        let dir = scratch("vanished");
        let path = dir.join("hook.sock");
        let server = HookServer::start(Endpoint::Unix(path.clone()), Arc::new(Echo)).unwrap();
        fs::remove_file(&path).unwrap();
        drop(server);
    }

    /// The Mac host must never open a port, so this runs only in a container.
    #[test]
    fn the_tcp_endpoint_binds_loopback_only() {
        if !crate::endpoint::in_container() {
            return;
        }
        let endpoint = Endpoint::Tcp(4359);
        let server = HookServer::start(endpoint.clone(), Arc::new(Echo)).unwrap();
        assert_eq!(
            ask(&endpoint, &Request::new("state")),
            Response::Ok(json!({"cmd": "state", "args": {}}))
        );
        let remote = std::net::TcpListener::bind("0.0.0.0:4359");
        assert!(remote.is_err(), "the hook port must be taken on loopback");
        drop(server);
    }
}
