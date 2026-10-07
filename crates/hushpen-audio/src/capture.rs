//! Microphone capture into the session WAV.
//!
//! Three kinds of threads cooperate. The mic thread owns the cpal stream
//! (streams are not `Send` everywhere), watches the device, and pushes raw
//! frames into a channel. The test feeder pushes frames from a WAV into the
//! same channel. The worker thread downmixes, resamples to 16 kHz mono, writes
//! the session WAV, and measures levels.

use crate::devices::{DEFAULT_ID, inputs_of};
use crate::error::CaptureError;
use crate::levels::LevelMeter;
use crate::resample::{MonoResampler, TARGET_RATE, downmix};
use crate::session::SessionWriter;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, FromSample, SampleFormat, SizedSample, StreamConfig};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// How long the stream may deliver no callbacks at all before the capture
/// counts as lost.
const STALL_LIMIT: Duration = Duration::from_millis(1500);
/// How often the mic thread checks that the selected device still exists.
const DEVICE_POLL: Duration = Duration::from_secs(1);
const MIC_START_TIMEOUT: Duration = Duration::from_secs(8);
const MIC_STOP_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, PartialEq)]
pub enum CaptureEvent {
    /// 0.0 to 1.0, about 20 times a second while audio flows.
    Level(f32),
    /// Sent at most once. The session stays open; call [`Capture::stop`] to
    /// close the file.
    Error(CaptureError),
}

pub type EventSink = Arc<dyn Fn(CaptureEvent) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub path: PathBuf,
    pub samples: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Mic,
    Feed,
}

pub(crate) enum Msg {
    Frames {
        source: Source,
        rate: u32,
        channels: u16,
        data: Vec<f32>,
    },
    FeedStart,
    FeedEnd,
    Stop(Sender<Result<Finished, CaptureError>>),
}

struct MicHandle {
    stop: Sender<()>,
    done: Receiver<()>,
}

pub struct Capture {
    pub(crate) tx: Sender<Msg>,
    worker: Option<JoinHandle<()>>,
    mic: Option<MicHandle>,
    pub(crate) feeding: Arc<AtomicBool>,
}

impl Capture {
    /// Starts recording from `device` (`"default"` or an id from
    /// `list_inputs`) into a new WAV at `path`. The file is created only after
    /// the microphone opened.
    pub fn start(device: &str, path: PathBuf, sink: EventSink) -> Result<Self, CaptureError> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let selection = device.to_string();
        let mic_tx = tx.clone();
        let mic_sink = Arc::clone(&sink);
        thread::Builder::new()
            .name("hushpen-mic".into())
            .spawn(move || {
                run_mic(&selection, mic_tx, mic_sink, ready_tx, stop_rx);
                let _ = done_tx.send(());
            })?;
        match ready_rx.recv_timeout(MIC_START_TIMEOUT) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                let _ = stop_tx.send(());
                return Err(CaptureError::unavailable("the microphone did not start"));
            }
        }
        let mic = MicHandle {
            stop: stop_tx,
            done: done_rx,
        };
        match Self::with_worker(path, sink, tx, rx) {
            Ok(mut capture) => {
                capture.mic = Some(mic);
                Ok(capture)
            }
            Err(error) => {
                mic.shut_down();
                Err(error)
            }
        }
    }

    /// A session with no microphone. Only [`Capture::feeder`] supplies audio.
    pub fn start_without_mic(path: PathBuf, sink: EventSink) -> Result<Self, CaptureError> {
        let (tx, rx) = mpsc::channel();
        Self::with_worker(path, sink, tx, rx)
    }

    fn with_worker(
        path: PathBuf,
        sink: EventSink,
        tx: Sender<Msg>,
        rx: Receiver<Msg>,
    ) -> Result<Self, CaptureError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let writer = SessionWriter::create(&path)?;
        let worker = thread::Builder::new()
            .name("hushpen-capture".into())
            .spawn(move || Worker::new(path, writer, sink).run(&rx))?;
        Ok(Self {
            tx,
            worker: Some(worker),
            mic: None,
            feeding: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Closes the microphone, fixes the WAV header, and returns the file.
    pub fn stop(mut self) -> Result<Finished, CaptureError> {
        if let Some(mic) = self.mic.take() {
            mic.shut_down();
        }
        let (reply, answer) = mpsc::channel();
        self.tx
            .send(Msg::Stop(reply))
            .map_err(|_| CaptureError::failed("the capture thread ended early"))?;
        let result = answer
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| CaptureError::failed("the capture thread did not answer"))?;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        result
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(mic) = self.mic.take() {
            mic.shut_down();
        }
        // A dropped sender makes the worker finalize the file.
    }
}

impl MicHandle {
    /// Waits briefly. A host that hangs while closing a stream must not hang
    /// the app; the thread is left to finish on its own.
    fn shut_down(self) {
        let _ = self.stop.send(());
        let _ = self.done.recv_timeout(MIC_STOP_TIMEOUT);
    }
}

struct SourceState {
    rate: u32,
    channels: u16,
    resampler: MonoResampler,
}

struct Worker {
    path: PathBuf,
    writer: Option<SessionWriter>,
    sink: EventSink,
    meter: LevelMeter,
    mic: Option<SourceState>,
    feed: Option<SourceState>,
    feeding: bool,
    failed: bool,
    mono: Vec<f32>,
    out: Vec<f32>,
}

impl Worker {
    fn new(path: PathBuf, writer: SessionWriter, sink: EventSink) -> Self {
        Self {
            path,
            writer: Some(writer),
            sink,
            meter: LevelMeter::new(TARGET_RATE),
            mic: None,
            feed: None,
            feeding: false,
            failed: false,
            mono: Vec::new(),
            out: Vec::new(),
        }
    }

    fn run(mut self, rx: &Receiver<Msg>) {
        loop {
            match rx.recv() {
                Ok(Msg::Frames {
                    source,
                    rate,
                    channels,
                    data,
                }) => {
                    // While a test feed plays, it replaces the microphone.
                    if source == Source::Mic && self.feeding {
                        continue;
                    }
                    self.ingest(source, rate, channels, &data);
                }
                Ok(Msg::FeedStart) => self.feeding = true,
                Ok(Msg::FeedEnd) => {
                    self.flush(Source::Feed);
                    self.feeding = false;
                }
                Ok(Msg::Stop(reply)) => {
                    let result = self.close();
                    let _ = reply.send(result);
                    return;
                }
                Err(_) => {
                    let _ = self.close();
                    return;
                }
            }
        }
    }

    fn state(&mut self, source: Source) -> &mut Option<SourceState> {
        match source {
            Source::Mic => &mut self.mic,
            Source::Feed => &mut self.feed,
        }
    }

    fn ingest(&mut self, source: Source, rate: u32, channels: u16, data: &[f32]) {
        let changed = self
            .state(source)
            .as_ref()
            .is_some_and(|state| state.rate != rate || state.channels != channels);
        if changed {
            self.flush(source);
        }
        if self.state(source).is_none() {
            match MonoResampler::new(rate) {
                Ok(resampler) => {
                    *self.state(source) = Some(SourceState {
                        rate,
                        channels,
                        resampler,
                    });
                }
                Err(error) => return self.fail(error),
            }
        }
        let mut mono = std::mem::take(&mut self.mono);
        let mut out = std::mem::take(&mut self.out);
        mono.clear();
        out.clear();
        downmix(data, usize::from(channels), &mut mono);
        if let Some(state) = self.state(source) {
            state.resampler.process(&mono, &mut out);
        }
        self.write(&out);
        self.mono = mono;
        self.out = out;
    }

    fn flush(&mut self, source: Source) {
        let Some(mut state) = self.state(source).take() else {
            return;
        };
        let mut out = Vec::new();
        state.resampler.finish(&mut out);
        self.write(&out);
    }

    fn write(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        if let Some(writer) = self.writer.as_mut()
            && !self.failed
            && let Err(error) = writer.write(samples)
        {
            self.fail(error.into());
        }
        let sink = Arc::clone(&self.sink);
        self.meter
            .push(samples, |level| sink(CaptureEvent::Level(level)));
    }

    fn fail(&mut self, error: CaptureError) {
        if !self.failed {
            self.failed = true;
            (self.sink)(CaptureEvent::Error(error));
        }
    }

    fn close(&mut self) -> Result<Finished, CaptureError> {
        self.flush(Source::Mic);
        self.flush(Source::Feed);
        let writer = self
            .writer
            .take()
            .ok_or_else(|| CaptureError::failed("the session file is already closed"))?;
        let samples = writer.finalize()?;
        Ok(Finished {
            path: self.path.clone(),
            samples,
            duration_ms: samples * 1000 / u64::from(TARGET_RATE),
        })
    }
}

/// Finds the cpal device for a saved id. `"default"` follows the system.
fn find_device(host: &cpal::Host, selection: &str) -> Result<cpal::Device, CaptureError> {
    if selection.is_empty() || selection == DEFAULT_ID {
        return host
            .default_input_device()
            .ok_or_else(|| CaptureError::unavailable("no default microphone"));
    }
    let id = DeviceId::from_str(selection)
        .map_err(|error| CaptureError::unavailable(format!("bad device id: {error}")))?;
    host.device_by_id(&id)
        .ok_or_else(|| CaptureError::unavailable(format!("microphone {selection} is not there")))
}

/// Reports one failure, once.
struct Reporter {
    failed: AtomicBool,
    sink: EventSink,
}

impl Reporter {
    fn report(&self, error: CaptureError) {
        if !self.failed.swap(true, Ordering::SeqCst) {
            log::warn!("{error}");
            (self.sink)(CaptureEvent::Error(error));
        }
    }
}

fn run_mic(
    selection: &str,
    tx: Sender<Msg>,
    sink: EventSink,
    ready: Sender<Result<(), CaptureError>>,
    stop: Receiver<()>,
) {
    let host = cpal::default_host();
    let epoch = Instant::now();
    let last_data = Arc::new(AtomicU64::new(0));
    let reporter = Arc::new(Reporter {
        failed: AtomicBool::new(false),
        sink,
    });
    let opened = find_device(&host, selection).and_then(|device| {
        let config = device.default_input_config()?;
        let stream = build_stream(&device, &config, &tx, &last_data, epoch, &reporter)?;
        stream.play()?;
        Ok(stream)
    });
    let stream = match opened {
        Ok(stream) => {
            let _ = ready.send(Ok(()));
            stream
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    last_data.store(epoch.elapsed().as_millis() as u64, Ordering::SeqCst);
    let mut next_poll = Instant::now() + DEVICE_POLL;
    while let Err(RecvTimeoutError::Timeout) = stop.recv_timeout(Duration::from_millis(250)) {
        if Instant::now() < next_poll {
            continue;
        }
        next_poll = Instant::now() + DEVICE_POLL;
        let silent_for = epoch
            .elapsed()
            .saturating_sub(Duration::from_millis(last_data.load(Ordering::SeqCst)));
        if silent_for > STALL_LIMIT {
            reporter.report(CaptureError::unavailable(
                "the microphone stopped sending audio",
            ));
        } else if selection != DEFAULT_ID
            && !selection.is_empty()
            && inputs_of(&host).is_ok_and(|devices| devices.iter().all(|d| d.id != selection))
        {
            reporter.report(CaptureError::unavailable("the microphone was removed"));
        }
    }
    drop(stream);
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    tx: &Sender<Msg>,
    last_data: &Arc<AtomicU64>,
    epoch: Instant,
    reporter: &Arc<Reporter>,
) -> Result<cpal::Stream, CaptureError> {
    let stream_config = config.config();
    match config.sample_format() {
        SampleFormat::F32 => typed::<f32>(device, stream_config, tx, last_data, epoch, reporter),
        SampleFormat::I16 => typed::<i16>(device, stream_config, tx, last_data, epoch, reporter),
        SampleFormat::U16 => typed::<u16>(device, stream_config, tx, last_data, epoch, reporter),
        SampleFormat::I32 => typed::<i32>(device, stream_config, tx, last_data, epoch, reporter),
        other => Err(CaptureError::failed(format!(
            "unsupported microphone sample format {other:?}"
        ))),
    }
}

fn typed<T>(
    device: &cpal::Device,
    config: StreamConfig,
    tx: &Sender<Msg>,
    last_data: &Arc<AtomicU64>,
    epoch: Instant,
    reporter: &Arc<Reporter>,
) -> Result<cpal::Stream, CaptureError>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let (rate, channels) = (config.sample_rate, config.channels);
    let tx = tx.clone();
    let last_data = Arc::clone(last_data);
    let on_error = Arc::clone(reporter);
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            last_data.store(epoch.elapsed().as_millis() as u64, Ordering::SeqCst);
            let data = data
                .iter()
                .map(|sample| sample.to_sample::<f32>())
                .collect();
            let _ = tx.send(Msg::Frames {
                source: Source::Mic,
                rate,
                channels,
                data,
            });
        },
        move |error| {
            use cpal::ErrorKind::{DeviceChanged, RealtimeDenied, Xrun};
            if matches!(error.kind(), DeviceChanged | Xrun | RealtimeDenied) {
                log::debug!("microphone stream: {error}");
            } else {
                on_error.report(error.into());
            }
        },
        None,
    )?;
    Ok(stream)
}

/// A path for a new session file in `dir`, named by start time. A counter
/// keeps two sessions in the same millisecond apart.
pub fn new_session_path(dir: &Path, now_ms: u64) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    loop {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
        let path = dir.join(format!("{now_ms}-{n}.wav"));
        if !path.exists() {
            return path;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{WavSpec, WavWriter};
    use std::sync::Mutex;

    fn events() -> (EventSink, Arc<Mutex<Vec<CaptureEvent>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink_log = Arc::clone(&log);
        let sink: EventSink = Arc::new(move |event| sink_log.lock().unwrap().push(event));
        (sink, log)
    }

    /// 1 s of silence, then `speech_ms` of a 440 Hz tone, at 44.1 kHz stereo.
    fn padded_tone(path: &Path, speech_ms: u32) {
        let spec = WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec).unwrap();
        let lead = 44_100;
        let speech = 44_100 * speech_ms / 1000;
        for i in 0..lead + speech {
            let value = if i < lead {
                0
            } else {
                let t = i as f32 / 44_100.0;
                (f32::sin(2.0 * std::f32::consts::PI * 440.0 * t) * 12_000.0) as i16
            };
            writer.write_sample(value).unwrap();
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn a_fed_wav_becomes_a_16k_mono_session_with_levels() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.wav");
        padded_tone(&input, 1000);
        let (sink, log) = events();
        let capture = Capture::start_without_mic(dir.path().join("s/a.wav"), sink).unwrap();
        let started = Instant::now();
        let info = capture.feeder().feed_wav(&input).unwrap();
        assert_eq!(
            (info.duration_ms, info.sample_rate, info.channels),
            (2000, 44_100, 2)
        );
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "feed_wav returns at once"
        );
        thread::sleep(Duration::from_millis(2300));
        let finished = capture.stop().unwrap();

        let reader = hound::WavReader::open(&finished.path).unwrap();
        assert_eq!(reader.spec(), crate::session::spec());
        assert_eq!(reader.duration(), 32_000, "2 s at 16 kHz");
        assert_eq!(finished.duration_ms, 2000);

        let levels: Vec<f32> = log
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                CaptureEvent::Level(level) => Some(*level),
                CaptureEvent::Error(_) => None,
            })
            .collect();
        assert_eq!(levels.len(), 40, "20 values per second for 2 s");
        assert!(
            levels[..18].iter().all(|l| *l == 0.0),
            "the lead is at rest"
        );
        assert!(
            levels[22..].iter().all(|l| *l > 0.4),
            "the tone moves the meter"
        );
    }

    #[test]
    fn a_feed_plays_at_real_time_pace() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.wav");
        padded_tone(&input, 500);
        let (sink, log) = events();
        let capture = Capture::start_without_mic(dir.path().join("a.wav"), sink).unwrap();
        capture.feeder().feed_wav(&input).unwrap();
        thread::sleep(Duration::from_millis(700));
        let seen = log.lock().unwrap().len();
        assert!(
            (10..=16).contains(&seen),
            "about 0.7 s of 50 ms levels, got {seen}"
        );
        drop(capture);
    }

    #[test]
    fn a_second_feed_is_refused_while_one_plays() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.wav");
        padded_tone(&input, 100);
        let capture = Capture::start_without_mic(dir.path().join("a.wav"), events().0).unwrap();
        let feeder = capture.feeder();
        feeder.feed_wav(&input).unwrap();
        assert!(feeder.feed_wav(&input).is_err());
        thread::sleep(Duration::from_millis(1300));
        assert!(feeder.feed_wav(&input).is_ok(), "free again after the end");
        drop(capture);
    }

    #[test]
    fn a_dropped_capture_still_leaves_a_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let input = dir.path().join("in.wav");
        padded_tone(&input, 100);
        let capture = Capture::start_without_mic(path.clone(), events().0).unwrap();
        capture.feeder().feed_wav(&input).unwrap();
        thread::sleep(Duration::from_millis(1400));
        drop(capture);
        thread::sleep(Duration::from_millis(200));
        assert!(hound::WavReader::open(&path).unwrap().duration() >= 16_000);
    }

    #[test]
    fn a_wav_that_is_not_audio_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("x.wav");
        std::fs::write(&text, b"hello").unwrap();
        let capture = Capture::start_without_mic(dir.path().join("a.wav"), events().0).unwrap();
        assert_eq!(
            capture.feeder().feed_wav(&text).unwrap_err().code,
            "CAPTURE_FAILED"
        );
        assert!(
            capture
                .feeder()
                .feed_wav(&dir.path().join("none.wav"))
                .is_err()
        );
    }

    #[test]
    fn session_paths_are_unique() {
        let dir = tempfile::tempdir().unwrap();
        let a = new_session_path(dir.path(), 1_700_000_000_000);
        std::fs::write(&a, b"x").unwrap();
        let b = new_session_path(dir.path(), 1_700_000_000_000);
        assert_ne!(a, b);
        assert_eq!(a.parent(), Some(dir.path()));
    }
}
