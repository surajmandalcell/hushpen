//! Downloads one model file: host policy on every hop, resume or clean restart, a streamed
//! sha256, and the final rename with the `.verified` stamp.
//!
//! The partial file lives in `cache/downloads/<file>.download`. It is removed on cancel, on a
//! blocked host, and on a hash or size mismatch. A network failure keeps it so the next try can
//! resume.

use super::fetch::{FetchError, Fetcher, Response};
use super::policy::{Blocked, Checked, MAX_REDIRECTS, Policy, resolve};
use super::{Purpose, record};
use hushpen_core::error::{DOWNLOAD_FAILED, MODEL_HASH_MISMATCH, MODEL_HOST_BLOCKED};
use hushpen_store::model_files::{self, hex};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::time::{Duration, Instant};

/// A download that gets no bytes for this long fails and keeps its partial file.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

const POLL: Duration = Duration::from_millis(50);
const REPORT_EVERY: Duration = Duration::from_millis(100);
const CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct Spec {
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
    pub partial: PathBuf,
    pub dest: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Bytes {
        done: u64,
        total: u64,
    },
    /// All bytes are in; the hash is being compared.
    Verifying,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: &'static str,
    pub detail: String,
}

impl Failure {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    fn failed(detail: impl Into<String>) -> Self {
        Self::new(DOWNLOAD_FAILED, detail)
    }
}

impl From<Blocked> for Failure {
    fn from(blocked: Blocked) -> Self {
        Self::new(blocked.code, blocked.reason)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Cancelled,
}

pub struct Downloader {
    policy: Policy,
    fetcher: Box<dyn Fetcher>,
    idle: Duration,
}

enum Message {
    Data(Vec<u8>),
    End,
    Error(String),
}

enum Opened {
    Body(Response),
    /// The server cannot serve the range we asked for.
    RangeRejected,
}

impl Downloader {
    pub fn new(policy: Policy, fetcher: Box<dyn Fetcher>) -> Self {
        Self {
            policy,
            fetcher,
            idle: IDLE_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub fn with_idle(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    pub fn run(
        &self,
        spec: &Spec,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Outcome, Failure> {
        let result = self.attempt(spec, cancel, progress);
        let discard = match &result {
            Ok(Outcome::Cancelled) => true,
            Err(failure) => matches!(failure.code, MODEL_HOST_BLOCKED | MODEL_HASH_MISMATCH),
            Ok(Outcome::Done) => false,
        };
        if discard {
            let _ = fs::remove_file(&spec.partial);
        }
        match &result {
            Ok(outcome) => log::info!("MODEL_DOWNLOAD_END {outcome:?}"),
            Err(failure) => log::warn!("{} {}", failure.code, failure.detail),
        }
        result
    }

    fn attempt(
        &self,
        spec: &Spec,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Outcome, Failure> {
        let start = self
            .policy
            .check_start(&spec.url)
            .map_err(|blocked| refused(&blocked))?;
        let mut existing = fs::metadata(&spec.partial).map_or(0, |meta| meta.len());
        if existing > spec.bytes {
            let _ = fs::remove_file(&spec.partial);
            existing = 0;
        }
        if existing == spec.bytes {
            let mut hasher = Sha256::new();
            if !hash_prefix(&spec.partial, existing, &mut hasher, cancel)? {
                return Ok(Outcome::Cancelled);
            }
            return finish(spec, hasher, progress).map(|()| Outcome::Done);
        }

        let mut range = (existing > 0).then_some(existing);
        let response = loop {
            match self.open(&start, range, cancel)? {
                None => return Ok(Outcome::Cancelled),
                Some(Opened::Body(response)) => break response,
                Some(Opened::RangeRejected) if range.is_some() => {
                    let _ = fs::remove_file(&spec.partial);
                    range = None;
                }
                Some(Opened::RangeRejected) => {
                    return Err(Failure::failed(
                        "the server answered 416 to a plain request",
                    ));
                }
            }
        };

        let resume_from = match (range, response.status, response.range_start) {
            (Some(from), 206, Some(start)) if start == from => from,
            (_, 200, _) => 0,
            (_, status, _) => {
                let _ = fs::remove_file(&spec.partial);
                return Err(Failure::failed(format!("unexpected answer {status}")));
            }
        };
        if let Some(length) = response.content_length
            && length != spec.bytes - resume_from
        {
            return Err(Failure::new(
                MODEL_HASH_MISMATCH,
                format!("the server's file is {length} bytes, not the pinned size"),
            ));
        }

        let mut hasher = Sha256::new();
        let mut file = if resume_from > 0 {
            if !hash_prefix(&spec.partial, resume_from, &mut hasher, cancel)? {
                return Ok(Outcome::Cancelled);
            }
            let mut file = OpenOptions::new()
                .write(true)
                .open(&spec.partial)
                .map_err(|error| {
                    Failure::failed(format!("cannot reopen the partial file: {error}"))
                })?;
            file.set_len(resume_from)
                .and_then(|()| file.seek(SeekFrom::End(0)).map(|_| ()))
                .map_err(|error| {
                    Failure::failed(format!("cannot resume the partial file: {error}"))
                })?;
            file
        } else {
            if let Some(dir) = spec.partial.parent() {
                fs::create_dir_all(dir).map_err(|error| {
                    Failure::failed(format!("cannot create the download folder: {error}"))
                })?;
            }
            File::create(&spec.partial).map_err(|error| {
                Failure::failed(format!("cannot create the partial file: {error}"))
            })?
        };

        let (sender, receiver) = sync_channel::<Message>(8);
        let mut body = response.body;
        std::thread::Builder::new()
            .name("hushpen-download".into())
            .spawn(move || {
                loop {
                    let mut buffer = vec![0u8; CHUNK];
                    let message = match body.read(&mut buffer) {
                        Ok(0) => Message::End,
                        Ok(read) => {
                            buffer.truncate(read);
                            Message::Data(buffer)
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(error) => Message::Error(error.to_string()),
                    };
                    let last = !matches!(message, Message::Data(_));
                    if sender.send(message).is_err() || last {
                        break;
                    }
                }
            })
            .map_err(|error| {
                Failure::failed(format!("cannot start the download thread: {error}"))
            })?;

        let mut done = resume_from;
        let mut last_data = Instant::now();
        let mut last_report = Instant::now();
        progress(Progress::Bytes {
            done,
            total: spec.bytes,
        });
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Ok(Outcome::Cancelled);
            }
            match receiver.recv_timeout(POLL) {
                Ok(Message::Data(chunk)) => {
                    done += chunk.len() as u64;
                    if done > spec.bytes {
                        return Err(Failure::new(
                            MODEL_HASH_MISMATCH,
                            "the server sent more bytes than the pinned size",
                        ));
                    }
                    file.write_all(&chunk).map_err(|error| {
                        Failure::failed(format!("cannot write the partial file: {error}"))
                    })?;
                    hasher.update(&chunk);
                    last_data = Instant::now();
                    if last_report.elapsed() >= REPORT_EVERY {
                        last_report = Instant::now();
                        progress(Progress::Bytes {
                            done,
                            total: spec.bytes,
                        });
                    }
                }
                Ok(Message::End) => break,
                Ok(Message::Error(error)) => {
                    let _ = file.flush();
                    return Err(Failure::failed(format!("the connection failed: {error}")));
                }
                Err(RecvTimeoutError::Timeout) => {
                    if last_data.elapsed() >= self.idle {
                        let _ = file.flush();
                        return Err(Failure::failed(format!(
                            "no data for {} s",
                            self.idle.as_secs().max(1)
                        )));
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = file.flush();
                    return Err(Failure::failed("the connection closed"));
                }
            }
        }
        file.flush()
            .and_then(|()| file.sync_all())
            .map_err(|error| Failure::failed(format!("cannot write the partial file: {error}")))?;
        drop(file);
        progress(Progress::Bytes {
            done,
            total: spec.bytes,
        });
        if done < spec.bytes {
            return Err(Failure::failed("the connection ended early"));
        }
        finish(spec, hasher, progress).map(|()| Outcome::Done)
    }

    /// Follows redirects by hand. Every hop passes the policy before a connection opens.
    fn open(
        &self,
        start: &Checked,
        range: Option<u64>,
        cancel: &AtomicBool,
    ) -> Result<Option<Opened>, Failure> {
        let mut current = start.clone();
        let mut hops = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let response = match self.fetcher.get(&current.url, range) {
                Ok(response) => response,
                Err(FetchError(error)) => {
                    record(Purpose::ModelDownload, &current.host, "error");
                    return Err(Failure::failed(format!(
                        "cannot reach {}: {error}",
                        current.host
                    )));
                }
            };
            record(
                Purpose::ModelDownload,
                &current.host,
                &response.status.to_string(),
            );
            match response.status {
                200 | 206 => return Ok(Some(Opened::Body(response))),
                416 => return Ok(Some(Opened::RangeRejected)),
                301 | 302 | 303 | 307 | 308 => {
                    hops += 1;
                    if hops > MAX_REDIRECTS {
                        return Err(refused(&Blocked::new(
                            current.host,
                            format!("more than {MAX_REDIRECTS} redirects"),
                        )));
                    }
                    let target = response
                        .location
                        .as_deref()
                        .and_then(|location| resolve(&current.url, location))
                        .ok_or_else(|| Failure::failed("a redirect had no usable location"))?;
                    current = self
                        .policy
                        .check_redirect(&target)
                        .map_err(|blocked| refused(&blocked))?;
                }
                status => {
                    return Err(Failure::failed(format!(
                        "{} answered {status}",
                        current.host
                    )));
                }
            }
        }
    }
}

fn refused(blocked: &Blocked) -> Failure {
    record(Purpose::ModelDownload, &blocked.host, "blocked");
    Failure::from(blocked.clone())
}

/// Feeds the first `len` bytes of the partial file into `hasher`. `false` means cancelled.
fn hash_prefix(
    path: &Path,
    len: u64,
    hasher: &mut Sha256,
    cancel: &AtomicBool,
) -> Result<bool, Failure> {
    let mut file = File::open(path)
        .map_err(|error| Failure::failed(format!("cannot read the partial file: {error}")))?;
    let mut buffer = vec![0u8; 1 << 20];
    let mut left = len;
    while left > 0 {
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let want = buffer
            .len()
            .min(usize::try_from(left).unwrap_or(usize::MAX));
        let read = file
            .read(&mut buffer[..want])
            .map_err(|error| Failure::failed(format!("cannot read the partial file: {error}")))?;
        if read == 0 {
            return Err(Failure::failed("the partial file is shorter than expected"));
        }
        hasher.update(&buffer[..read]);
        left -= read as u64;
    }
    Ok(true)
}

#[cfg(test)]
mod tests;

/// Compares the hash, moves the file into place, and writes the stamp.
fn finish(spec: &Spec, hasher: Sha256, progress: &mut dyn FnMut(Progress)) -> Result<(), Failure> {
    progress(Progress::Verifying);
    let digest = hex(&hasher.finalize());
    if digest != spec.sha256 {
        return Err(Failure::new(
            MODEL_HASH_MISMATCH,
            "the downloaded file does not match the pinned sha256",
        ));
    }
    if let Some(dir) = spec.dest.parent() {
        fs::create_dir_all(dir).map_err(|error| {
            Failure::failed(format!("cannot create the models folder: {error}"))
        })?;
    }
    fs::rename(&spec.partial, &spec.dest)
        .map_err(|error| Failure::failed(format!("cannot move the model into place: {error}")))?;
    if let Err(error) = model_files::write_stamp(&spec.dest, &digest) {
        log::warn!("MODEL_STAMP_FAILED {error}");
    }
    Ok(())
}
