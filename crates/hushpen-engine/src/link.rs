//! The byte stream between a supervisor and its worker.

use crate::ChildSpec;
use hushpen_core::protocol::{FrameKind, MAX_FRAME, Request, write_frame, write_json};
use hushpen_core::remote::UnloadRule;
use hushpen_remote::{
    RemoteControl, RemoteStream, RemoteWriter, WorkerKind, select_worker_with_rule,
};
use std::io::{self, Read, Write};
use std::process::{Child, ChildStderr, Command, Stdio};

pub enum Link {
    Child(ChildSpec),
    Remote(RemoteSpec),
}

/// A job's authenticated connection. Cancelling closes it; the route owner reconnects.
pub struct RemoteSpec {
    stream: Option<RemoteStream>,
    kind: WorkerKind,
    unload_rule: Option<UnloadRule>,
}

impl RemoteSpec {
    pub fn new(stream: RemoteStream, kind: WorkerKind) -> Self {
        Self {
            stream: Some(stream),
            kind,
            unload_rule: None,
        }
    }

    pub fn with_unload_rule(mut self, rule: UnloadRule) -> Self {
        self.unload_rule = Some(rule);
        self
    }
}

impl From<ChildSpec> for Link {
    fn from(value: ChildSpec) -> Self {
        Self::Child(value)
    }
}

pub(crate) struct Connection {
    pub child: Option<Child>,
    pub stdin: Box<dyn Write + Send>,
    pub stdout: Box<dyn Read + Send>,
    pub stderr: Option<ChildStderr>,
}

struct RemoteOutput(RemoteWriter);
impl Write for RemoteOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl Drop for RemoteOutput {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

impl Link {
    pub(crate) fn control(&self) -> Option<RemoteControl> {
        match self {
            Self::Child(_) => None,
            Self::Remote(spec) => spec.stream.as_ref().map(RemoteStream::control),
        }
    }

    pub(crate) fn open(&mut self) -> io::Result<Connection> {
        match self {
            Self::Child(spec) => {
                let mut child = Command::new(&spec.program)
                    .args(&spec.args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;
                let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
                match pipes {
                    (Some(stdin), Some(stdout), Some(stderr)) => Ok(Connection {
                        child: Some(child),
                        stdin: Box::new(stdin),
                        stdout: Box::new(stdout),
                        stderr: Some(stderr),
                    }),
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        Err(io::Error::other("the worker has no pipes"))
                    }
                }
            }
            Self::Remote(spec) => {
                let mut stream = spec.stream.take().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotConnected,
                        "the remote worker disconnected",
                    )
                })?;
                select_worker_with_rule(&mut stream, spec.kind, spec.unload_rule)?;
                let (reader, writer) = stream.split()?;
                Ok(Connection {
                    child: None,
                    stdin: Box::new(RemoteOutput(writer)),
                    stdout: Box::new(reader),
                    stderr: None,
                })
            }
        }
    }

    pub(crate) fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }
}

/// Sends the WAV in bounded binary frames before the normal transcription request.
pub(crate) fn write_speech(writer: &mut impl Write, request: &Request) -> io::Result<()> {
    let path = match request {
        Request::Transcribe { wav_path, .. } => Some(wav_path),
        Request::ImportFile { path, .. } => Some(path),
        _ => None,
    };
    if let Some(path) = path {
        if !std::fs::metadata(path)?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote audio must be a file",
            ));
        }
        let mut file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        if len == 0 || len > 256 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "remote audio exceeds 256 MiB",
            ));
        }
        write_json(
            writer,
            &serde_json::json!({"type":"remote_audio", "bytes":len}),
        )
        .map_err(io::Error::other)?;
        let mut buffer = vec![0; MAX_FRAME.min(64 * 1024)];
        let mut left = len;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while left > 0 {
            if std::time::Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "audio transfer timed out",
                ));
            }
            let count = left.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..count])?;
            write_frame(writer, FrameKind::Binary, &buffer[..count]).map_err(io::Error::other)?;
            left -= count as u64;
        }
    }
    write_json(writer, &request.to_json()).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::protocol::read_frame;

    #[test]
    fn remote_audio_crosses_in_chunks_before_the_unchanged_request() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("speech.wav");
        let audio = vec![7_u8; 140_000];
        std::fs::write(&path, &audio).unwrap();
        let request = Request::Transcribe {
            job: 1,
            wav_path: path.to_string_lossy().into_owned(),
            language: Some("en".into()),
            prompt: None,
        };
        let mut bytes = Vec::new();
        write_speech(&mut bytes, &request).unwrap();
        let mut input = &bytes[..];
        let header = read_frame(&mut input).unwrap().unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header.payload).unwrap();
        assert_eq!(header["bytes"], 140_000);
        let mut received = Vec::new();
        for _ in 0..3 {
            let frame = read_frame(&mut input).unwrap().unwrap();
            assert_eq!(frame.kind, FrameKind::Binary);
            assert!(frame.payload.len() <= 64 * 1024);
            received.extend(frame.payload);
        }
        assert_eq!(received, audio);
        let frame = read_frame(&mut input).unwrap().unwrap();
        assert_eq!(Request::from_frame(&frame).unwrap(), request);
        assert!(read_frame(&mut input).unwrap().is_none());
    }

    #[test]
    fn remote_audio_rejects_empty_files_and_directories_before_writing() {
        let folder = tempfile::tempdir().unwrap();
        let empty = folder.path().join("empty.wav");
        std::fs::write(&empty, []).unwrap();
        for path in [folder.path(), empty.as_path()] {
            let request = Request::Transcribe {
                job: 1,
                wav_path: path.to_string_lossy().into_owned(),
                language: None,
                prompt: None,
            };
            let mut bytes = Vec::new();
            assert!(write_speech(&mut bytes, &request).is_err());
            assert!(bytes.is_empty());
        }
    }
}
