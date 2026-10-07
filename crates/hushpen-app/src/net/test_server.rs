//! A tiny HTTP/1.1 server on loopback for the download tests. Only tests use it.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Req {
    pub path: String,
    /// First byte of a `Range: bytes=N-` header.
    pub range_from: Option<u64>,
}

pub type Handler = dyn Fn(&Req, &mut TcpStream) + Send + Sync;

pub struct TestServer {
    pub port: u16,
    pub requests: Arc<Mutex<Vec<Req>>>,
    stop: Arc<AtomicBool>,
}

impl TestServer {
    pub fn start(handler: impl Fn(&Req, &mut TcpStream) + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind a loopback port");
        let port = listener.local_addr().expect("local address").port();
        listener.set_nonblocking(true).expect("non-blocking accept");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let handler: Arc<Handler> = Arc::new(handler);
        let (seen, halt) = (Arc::clone(&requests), Arc::clone(&stop));
        std::thread::spawn(move || {
            while !halt.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // macOS hands out the accepted socket in the listener's non-blocking mode.
                        let _ = stream.set_nonblocking(false);
                        let (handler, seen) = (Arc::clone(&handler), Arc::clone(&seen));
                        std::thread::spawn(move || serve(stream, &*handler, &seen));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            port,
            requests,
            stop,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn seen(&self) -> Vec<Req> {
        self.requests.lock().expect("request log").clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve(stream: TcpStream, handler: &Handler, seen: &Mutex<Vec<Req>>) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let mut range_from = None;
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range_from = value
                .trim()
                .strip_prefix("bytes=")
                .and_then(|range| range.split('-').next())
                .and_then(|from| from.parse().ok());
        }
    }
    let request = Req { path, range_from };
    seen.lock().expect("request log").push(request.clone());
    let mut stream = stream;
    handler(&request, &mut stream);
}

/// Writes one complete response and closes the connection.
pub fn respond(stream: &mut TcpStream, status: &str, headers: &[(&str, String)], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// Serves `body`, honoring a `Range: bytes=N-` request with a `206` answer.
pub fn respond_with_range(stream: &mut TcpStream, request: &Req, body: &[u8]) {
    match request.range_from {
        Some(from) if (from as usize) < body.len() => {
            let total = body.len();
            let range = format!("bytes {from}-{}/{total}", total - 1);
            respond(
                stream,
                "206 Partial Content",
                &[("Content-Range", range)],
                &body[from as usize..],
            );
        }
        Some(_) => respond(stream, "416 Range Not Satisfiable", &[], b""),
        None => respond(stream, "200 OK", &[], body),
    }
}
