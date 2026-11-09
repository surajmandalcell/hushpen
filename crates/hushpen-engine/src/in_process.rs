//! Bounded memory pipes let phone threads use the worker protocol without a process.

use crate::link::Connection;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

const CAPACITY: usize = 256 * 1024;
type Reader = Box<dyn Read + Send>;
type Writer = Box<dyn Write + Send>;

pub struct InProcess {
    serve: Arc<dyn Fn(Reader, Writer) + Send + Sync>,
    live: Arc<AtomicBool>,
}

impl InProcess {
    pub fn new(serve: impl Fn(Reader, Writer) + Send + Sync + 'static) -> Self {
        Self {
            serve: Arc::new(serve),
            live: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn speech() -> Self {
        Self::new(|input, output| {
            crate::server::serve(input, output, crate::server_config());
        })
    }

    pub fn speech_with_detector(detector: std::path::PathBuf) -> Self {
        Self::new(move |input, output| {
            crate::server::serve(
                input,
                output,
                crate::server_config_with_detector(detector.clone()),
            );
        })
    }

    pub(crate) fn open(&self) -> io::Result<Connection> {
        self.live
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the previous model thread is still stopping",
                )
            })?;
        let (input, stdin) = pipe();
        let (stdout, mut output) = pipe();
        output.1 = Some(Arc::clone(&self.live));
        let serve = Arc::clone(&self.serve);
        std::thread::Builder::new()
            .name("model-server".into())
            .spawn(move || serve(Box::new(input), Box::new(output)))?;
        Ok(Connection {
            child: None,
            stdin: Box::new(stdin),
            stdout: Box::new(stdout),
            stderr: None,
        })
    }
}

#[derive(Default)]
struct Buffer {
    bytes: VecDeque<u8>,
    reader_closed: bool,
    writer_closed: bool,
}

#[derive(Default)]
struct Pipe {
    buffer: Mutex<Buffer>,
    changed: Condvar,
}

struct PipeReader(Arc<Pipe>);
struct PipeWriter(Arc<Pipe>, Option<Arc<AtomicBool>>);

fn pipe() -> (PipeReader, PipeWriter) {
    let state = Arc::new(Pipe::default());
    (PipeReader(Arc::clone(&state)), PipeWriter(state, None))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Read for PipeReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let mut buffer = lock(&self.0.buffer);
        while buffer.bytes.is_empty() && !buffer.writer_closed {
            buffer = self
                .0
                .changed
                .wait(buffer)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        let count = output.len().min(buffer.bytes.len());
        for (target, byte) in output.iter_mut().zip(buffer.bytes.drain(..count)) {
            *target = byte;
        }
        self.0.changed.notify_all();
        Ok(count)
    }
}

impl Write for PipeWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if input.is_empty() {
            return Ok(0);
        }
        let mut buffer = lock(&self.0.buffer);
        while buffer.bytes.len() == CAPACITY && !buffer.reader_closed {
            buffer = self
                .0
                .changed
                .wait(buffer)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        if buffer.reader_closed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "model reader stopped",
            ));
        }
        let count = input.len().min(CAPACITY - buffer.bytes.len());
        buffer.bytes.extend(&input[..count]);
        self.0.changed.notify_all();
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        lock(&self.0.buffer).reader_closed = true;
        self.0.changed.notify_all();
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        lock(&self.0.buffer).writer_closed = true;
        self.0.changed.notify_all();
        if let Some(live) = &self.1 {
            live.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::protocol::{Event, PROTOCOL_VERSION, Request, read_frame, write_json};
    use std::time::Duration;

    #[test]
    fn speech_protocol_runs_on_a_thread_without_a_child() {
        let mut connection = InProcess::speech().open().unwrap();
        assert!(connection.child.is_none());
        write_json(
            &mut connection.stdin,
            &Request::Hello {
                protocol: PROTOCOL_VERSION,
            }
            .to_json(),
        )
        .unwrap();
        let frame = read_frame(&mut connection.stdout).unwrap().unwrap();
        assert!(matches!(
            Event::from_frame(&frame).unwrap(),
            Event::Ready { .. }
        ));
        write_json(&mut connection.stdin, &Request::Ping.to_json()).unwrap();
        let frame = read_frame(&mut connection.stdout).unwrap().unwrap();
        assert_eq!(Event::from_frame(&frame).unwrap(), Event::Pong);
        write_json(&mut connection.stdin, &Request::Shutdown.to_json()).unwrap();
        assert!(read_frame(&mut connection.stdout).unwrap().is_none());
    }

    #[test]
    fn a_full_pipe_unblocks_when_its_reader_closes() {
        let (reader, mut writer) = pipe();
        writer.write_all(&vec![1; CAPACITY]).unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || sender.send(writer.write_all(&[2])).unwrap());
        assert!(receiver.recv_timeout(Duration::from_millis(20)).is_err());
        drop(reader);
        assert_eq!(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn writer_close_preserves_buffered_bytes_then_reports_eof() {
        let (mut reader, mut writer) = pipe();
        writer.write_all(b"last").unwrap();
        drop(writer);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"last");
    }

    #[test]
    fn a_restart_waits_until_the_previous_server_releases_its_output() {
        let server = InProcess::new(|mut input, _output| {
            let mut byte = [0];
            let _ = input.read(&mut byte);
        });
        let connection = server.open().unwrap();
        assert!(matches!(server.open(), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
        drop(connection);
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while server.live.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(server.open().is_ok());
    }
}
