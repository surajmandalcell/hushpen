use super::{Output, Workers, finish_speech, invalid, lock, send};
use hound::{SampleFormat, WavSpec, WavWriter};
use hushpen_core::protocol::Event;
use hushpen_engine::{JobOutcome, TranscribeSpec};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_CAPTURE: Duration = Duration::from_secs(61 * 60);

pub(super) struct Input {
    pub job: u64,
    worker_job: u64,
    writer: WavWriter<BufWriter<File>>,
    bytes: u64,
    until: Instant,
}

impl Input {
    pub fn start(
        request: &Value,
        cache: &Path,
        workers: &Arc<Workers>,
        output: &Output,
        jobs: &Arc<Mutex<HashMap<u64, u64>>>,
    ) -> io::Result<Self> {
        let fields = request
            .as_object()
            .ok_or_else(|| invalid("invalid live audio request"))?;
        if fields
            .keys()
            .any(|key| !matches!(key.as_str(), "type" | "job" | "language"))
        {
            return Err(invalid("unknown live audio field"));
        }
        let job = request["job"]
            .as_u64()
            .ok_or_else(|| invalid("invalid live job id"))?;
        let language = match &request["language"] {
            Value::Null => None,
            Value::String(code) if code.len() <= 16 => Some(code.clone()),
            _ => return Err(invalid("invalid live audio language")),
        };
        let audio = tempfile::Builder::new().suffix(".wav").tempfile_in(cache)?;
        let format = WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer =
            WavWriter::new(BufWriter::new(audio.reopen()?), format).map_err(io::Error::other)?;
        writer.flush().map_err(io::Error::other)?;
        if workers.speech_busy.swap(true, Ordering::AcqRel) {
            return Err(invalid("speech worker is busy"));
        }
        let activity = workers.job_started();
        let latest = Arc::new(Mutex::new(None));
        let partial = Arc::clone(&latest);
        let handle = workers.speech.transcribe_live(
            TranscribeSpec {
                wav_path: audio.path().into(),
                language,
                prompt: None,
            },
            Arc::new(move |text| *lock(&partial) = Some(text)),
        );
        let worker_job = handle.id();
        lock(jobs).insert(job, worker_job);
        let workers = Arc::clone(workers);
        let output = Arc::clone(output);
        let jobs = Arc::clone(jobs);
        let until = Instant::now() + MAX_CAPTURE;
        std::thread::spawn(move || {
            let _activity = activity;
            let outcome = loop {
                let outcome = handle.wait_timeout(Duration::from_millis(25));
                let text = lock(&latest).take();
                if let Some(text) = text
                    && send(&output, &Event::Partial { job, text }.to_json()).is_err()
                {
                    handle.cancel();
                    break JobOutcome::Cancelled;
                }
                if let Some(outcome) = outcome {
                    break outcome;
                }
                if Instant::now() > until + Duration::from_secs(300) {
                    handle.cancel();
                    break JobOutcome::Cancelled;
                }
            };
            finish_speech(outcome, job, &output);
            drop(audio);
            lock(&jobs).remove(&job);
            workers.speech_busy.store(false, Ordering::Release);
        });
        Ok(Self {
            job,
            worker_job,
            writer,
            bytes: 0,
            until,
        })
    }

    pub fn deadline(&self) -> Instant {
        self.until.min(Instant::now() + Duration::from_secs(60))
    }

    pub fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.bytes += bytes.len() as u64;
        if Instant::now() >= self.until || self.bytes > 256 * 1024 * 1024 {
            return Err(invalid("live audio exceeded its transfer limit"));
        }
        if bytes.is_empty() || bytes.len() > 64 * 1024 || !bytes.len().is_multiple_of(2) {
            return Err(invalid("invalid live PCM frame"));
        }
        for pair in bytes.as_chunks::<2>().0 {
            self.writer
                .write_sample(i16::from_le_bytes(*pair))
                .map_err(io::Error::other)?;
        }
        self.writer.flush().map_err(io::Error::other)
    }

    pub fn finish(self, workers: &Workers) -> io::Result<()> {
        self.writer.finalize().map_err(io::Error::other)?;
        workers.speech.finish_live(self.worker_job);
        Ok(())
    }
}
