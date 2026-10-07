//! Test feed: plays a WAV file into the capture path at real-time pace, in
//! place of the microphone. The product has no caller for it; the debug app's
//! test hook (`hookctl feed-wav`) does.

use crate::capture::{Capture, Msg, Source};
use crate::error::CaptureError;
use hound::{SampleFormat, WavReader};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

/// One message every 20 ms, like a microphone callback.
const SLICE: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedInfo {
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Cloneable handle that feeds one WAV at a time into a running capture.
#[derive(Clone)]
pub struct Feeder {
    tx: Sender<Msg>,
    busy: Arc<AtomicBool>,
}

impl Capture {
    pub fn feeder(&self) -> Feeder {
        Feeder {
            tx: self.tx.clone(),
            busy: Arc::clone(&self.feeding),
        }
    }
}

impl Feeder {
    /// Starts playing `path` and returns at once. The microphone is ignored
    /// until the file ends.
    pub fn feed_wav(&self, path: &Path) -> Result<FeedInfo, CaptureError> {
        let (samples, info) = read_wav(path)?;
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(CaptureError::failed("a WAV is already being fed"));
        }
        if self.tx.send(Msg::FeedStart).is_err() {
            self.busy.store(false, Ordering::SeqCst);
            return Err(CaptureError::failed("no capture session is running"));
        }
        let tx = self.tx.clone();
        let busy = Arc::clone(&self.busy);
        let (rate, channels) = (info.sample_rate, info.channels);
        let spawned = thread::Builder::new()
            .name("hushpen-feed".into())
            .spawn(move || {
                let slice = (rate as usize / 50) * usize::from(channels);
                let start = Instant::now();
                for (index, data) in samples.chunks(slice).enumerate() {
                    let due = start + SLICE * index as u32;
                    thread::sleep(due.saturating_duration_since(Instant::now()));
                    let frames = Msg::Frames {
                        source: Source::Feed,
                        rate,
                        channels,
                        data: data.to_vec(),
                        at: Instant::now(),
                    };
                    if tx.send(frames).is_err() {
                        break;
                    }
                }
                let _ = tx.send(Msg::FeedEnd);
                busy.store(false, Ordering::SeqCst);
            });
        if let Err(error) = spawned {
            let _ = self.tx.send(Msg::FeedEnd);
            self.busy.store(false, Ordering::SeqCst);
            return Err(error.into());
        }
        Ok(info)
    }
}

fn read_wav(path: &Path) -> Result<(Vec<f32>, FeedInfo), CaptureError> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => reader.samples::<f32>().collect::<Result<_, _>>()?,
        (SampleFormat::Int, bits @ 8..=32) => {
            let scale = 2_f32.powi(i32::from(bits) - 1);
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect::<Result<_, _>>()?
        }
        (format, bits) => {
            return Err(CaptureError::failed(format!(
                "unsupported WAV format {format:?} {bits} bit"
            )));
        }
    };
    let frames = samples.len() as u64 / u64::from(spec.channels.max(1));
    Ok((
        samples,
        FeedInfo {
            duration_ms: frames * 1000 / u64::from(spec.sample_rate.max(1)),
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        },
    ))
}
