//! Microphone capture into the session WAV.
//!
//! Three kinds of threads cooperate. The mic thread (see [`crate::mic_stream`]) owns the cpal
//! stream (streams are not `Send` everywhere), watches the device, and pushes raw frames into
//! a channel. The test feeder pushes frames from a WAV into the same channel. The worker
//! thread downmixes, resamples to 16 kHz mono, writes the session WAV, and measures levels.

use crate::enhance::{Enhance, Enhancer};
use crate::error::CaptureError;
use crate::levels::LevelMeter;
use crate::mic_stream::{MicHandle, MicHub};
use crate::resample::{MonoResampler, TARGET_RATE, downmix};
use crate::session::SessionWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, LazyLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Silence is inserted for a pause in the microphone's delivery longer than this. Host
/// buffers are 20 ms to 100 ms, so shorter gaps are only jitter.
const MID_STREAM_GAP_MS: u64 = 250;
/// At the end of a session the file is padded to the wall clock when it falls short by more
/// than this.
const END_GAP_MS: u64 = 100;
/// How long a stop waits for the host to hand over the audio it still holds. Audio buffers
/// are 20 ms to 100 ms, so a callback after the stop request means the last words are in the
/// file. A source that sends nothing, such as an idle virtual one, costs the whole wait.
const TAIL_DRAIN_MAX: Duration = Duration::from_millis(250);
/// The most a boost may raise the microphone: 200%.
pub(crate) const MAX_GAIN: f32 = 2.0;
/// Below this a boosted sample passes unchanged; above it a soft knee bends it toward full
/// scale, so a loud word under a strong boost is rounded instead of cut flat.
const KNEE: f32 = 0.8;

/// Multiplies microphone samples by `gain` (1.0 to [`MAX_GAIN`]) and keeps them inside -1.0 to
/// 1.0 with a soft knee. A gain of 1.0 changes nothing.
pub(crate) fn boost(samples: &mut [f32], gain: f32) {
    let gain = if gain.is_finite() {
        gain.clamp(1.0, MAX_GAIN)
    } else {
        1.0
    };
    if gain == 1.0 {
        return;
    }
    for sample in samples {
        let raised = *sample * gain;
        let size = raised.abs();
        *sample = if size <= KNEE {
            raised
        } else {
            raised.signum() * (KNEE + (1.0 - KNEE) * ((size - KNEE) / (1.0 - KNEE)).tanh())
        };
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaptureEvent {
    /// 0.0 to 1.0, about 20 times a second while audio flows.
    Level(f32),
    /// The microphone stream is running and the session now receives its audio.
    Started,
    /// The microphone did not open. The session stays open; call [`Capture::stop`] to discard
    /// it.
    StartFailed(CaptureError),
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
        /// When the host handed the audio over.
        at: Instant,
    },
    FeedStart,
    FeedEnd,
    Stop(Sender<Result<Finished, CaptureError>>),
}

static HUB: LazyLock<MicHub> = LazyLock::new(MicHub::new);

/// Opens the microphone stream for `device` ahead of time, paused, so the next
/// [`Capture::start`] begins at once. Returns immediately.
pub fn warm_microphone(device: &str) {
    HUB.warm(device);
}

pub struct Capture {
    pub(crate) tx: Sender<Msg>,
    worker: Option<JoinHandle<()>>,
    mic: Option<MicHandle>,
    pub(crate) feeding: Arc<AtomicBool>,
    started: Instant,
    /// Milliseconds after `started` of the newest microphone delivery the worker wrote, plus 1.
    mic_seen: Arc<AtomicU64>,
    /// The boost of the microphone, as the bits of an `f32`.
    gain: Arc<AtomicU32>,
    /// The enhance of the microphone, as [`Enhance::to_bits`].
    enhance: Arc<AtomicU64>,
}

impl Capture {
    /// Starts recording from `device` (`"default"` or an id from
    /// `list_inputs`) into a new WAV at `path`. Returns at once: the microphone is resumed on
    /// its own thread, and a microphone that does not open ends in
    /// [`CaptureEvent::StartFailed`]. The file's clock starts with this call, so time spent
    /// opening a stream is silence in the file instead of missing from it.
    pub fn start(device: &str, path: PathBuf, sink: EventSink) -> Result<Self, CaptureError> {
        let started = Instant::now();
        let (tx, rx) = mpsc::channel();
        let mut capture =
            Self::with_worker(path, Arc::clone(&sink), tx.clone(), rx, true, started)?;
        capture.mic = Some(HUB.attach(device, &tx, &sink)?);
        Ok(capture)
    }

    /// A session with no microphone. Only [`Capture::feeder`] supplies audio.
    pub fn start_without_mic(path: PathBuf, sink: EventSink) -> Result<Self, CaptureError> {
        let started = Instant::now();
        let (tx, rx) = mpsc::channel();
        Self::with_worker(path, sink, tx, rx, false, started)
    }

    /// `paced` makes the file follow the wall clock, counted from `started`: time in which
    /// the microphone delivered nothing becomes silence.
    fn with_worker(
        path: PathBuf,
        sink: EventSink,
        tx: Sender<Msg>,
        rx: Receiver<Msg>,
        paced: bool,
        started: Instant,
    ) -> Result<Self, CaptureError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let writer = SessionWriter::create(&path)?;
        let mic_seen = Arc::new(AtomicU64::new(0));
        let seen = Arc::clone(&mic_seen);
        let gain = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let boost = Arc::clone(&gain);
        let enhance = Arc::new(AtomicU64::new(Enhance::OFF.to_bits()));
        let shaping = Arc::clone(&enhance);
        let worker = thread::Builder::new()
            .name("hushpen-capture".into())
            .spawn(move || {
                let mut worker = Worker::new(path, writer, sink, paced, started);
                worker.mic_seen = seen;
                worker.gain = boost;
                worker.enhance = shaping;
                worker.run(&rx);
            })?;
        Ok(Self {
            tx,
            worker: Some(worker),
            mic: None,
            feeding: Arc::new(AtomicBool::new(false)),
            started,
            mic_seen,
            gain,
            enhance,
        })
    }

    /// Boosts the microphone by `gain` (1.0 to [`MAX_GAIN`]) from the next audio it delivers,
    /// for the file and the level alike. A test feed is never boosted.
    pub fn set_gain(&self, gain: f32) {
        self.gain.store(gain.to_bits(), Ordering::SeqCst);
    }

    /// Runs the microphone through `enhance` from the next audio it delivers, before the boost.
    /// A test feed is never enhanced.
    pub fn set_enhance(&self, enhance: Enhance) {
        self.enhance.store(enhance.to_bits(), Ordering::SeqCst);
    }

    /// Closes the microphone, fixes the WAV header, and returns the file.
    pub fn stop(mut self) -> Result<Finished, CaptureError> {
        if self.mic.is_some() {
            self.drain_tail();
        }
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

impl Capture {
    /// Waits until the microphone has delivered something after now, so audio the host still
    /// held when the user stopped is in the file instead of being replaced by padding.
    fn drain_tail(&self) {
        let asked_ms = self.started.elapsed().as_millis() as u64;
        let give_up = Instant::now() + TAIL_DRAIN_MAX;
        while self.mic_seen.load(Ordering::SeqCst) <= asked_ms && Instant::now() < give_up {
            thread::sleep(Duration::from_millis(5));
        }
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
    paced: bool,
    started: Instant,
    /// 16 kHz samples in the file so far.
    written: u64,
    mic_seen: Arc<AtomicU64>,
    gain: Arc<AtomicU32>,
    enhance: Arc<AtomicU64>,
    enhancer: Enhancer,
    shaped: Vec<f32>,
}

impl Worker {
    fn new(
        path: PathBuf,
        writer: SessionWriter,
        sink: EventSink,
        paced: bool,
        started: Instant,
    ) -> Self {
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
            paced,
            started,
            written: 0,
            mic_seen: Arc::new(AtomicU64::new(0)),
            gain: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            enhance: Arc::new(AtomicU64::new(Enhance::OFF.to_bits())),
            enhancer: Enhancer::new(),
            shaped: Vec::new(),
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
                    at,
                }) => {
                    // While a test feed plays, it replaces the microphone.
                    if source == Source::Mic && self.feeding {
                        continue;
                    }
                    if source == Source::Mic {
                        let chunk_ms = (data.len() / usize::from(channels.max(1))) as u64 * 1000
                            / u64::from(rate.max(1));
                        self.pad_to(at, chunk_ms, MID_STREAM_GAP_MS);
                    }
                    self.ingest(source, rate, channels, &data);
                    if source == Source::Mic {
                        let ms = at.saturating_duration_since(self.started).as_millis() as u64;
                        self.mic_seen.fetch_max(ms + 1, Ordering::SeqCst);
                    }
                }
                Ok(Msg::FeedStart) => {
                    self.pad_to(Instant::now(), 0, MID_STREAM_GAP_MS);
                    self.feeding = true;
                }
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
        if source == Source::Mic {
            self.enhance_mic(&mut out);
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
        if source == Source::Mic {
            self.enhance_mic(&mut out);
            let gain = f32::from_bits(self.gain.load(Ordering::SeqCst));
            self.enhancer.finish(gain, &mut out);
        }
        self.write(&out);
    }

    /// Runs 16 kHz microphone audio through the enhance and then the boost, as set now.
    fn enhance_mic(&mut self, samples: &mut Vec<f32>) {
        let enhance = Enhance::from_bits(self.enhance.load(Ordering::SeqCst));
        let gain = f32::from_bits(self.gain.load(Ordering::SeqCst));
        let mut shaped = std::mem::take(&mut self.shaped);
        shaped.clear();
        self.enhancer.process(enhance, gain, samples, &mut shaped);
        std::mem::swap(samples, &mut shaped);
        self.shaped = shaped;
    }

    /// Fills the file with silence up to `at` minus `chunk_ms` when the gap is longer than
    /// `min_gap_ms`. A virtual source sends nothing while idle, and a late start or a dropout
    /// must not pull the later audio earlier in the file.
    fn pad_to(&mut self, at: Instant, chunk_ms: u64, min_gap_ms: u64) {
        if !self.paced {
            return;
        }
        let elapsed_ms = at.saturating_duration_since(self.started).as_millis() as u64;
        let written_ms = self.written * 1000 / u64::from(TARGET_RATE);
        let gap_ms = elapsed_ms.saturating_sub(written_ms + chunk_ms);
        if gap_ms > min_gap_ms {
            let silence = vec![0.0; (gap_ms * u64::from(TARGET_RATE) / 1000) as usize];
            self.write(&silence);
        }
    }

    fn write(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        self.written += samples.len() as u64;
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
        self.pad_to(Instant::now(), 0, END_GAP_MS);
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

    /// Runs a worker on a thread the way `Capture` does and returns its sender.
    fn worker(path: &Path, paced: bool) -> (Sender<Msg>, Instant, JoinHandle<()>) {
        worker_since(path, paced, Instant::now())
    }

    fn worker_since(
        path: &Path,
        paced: bool,
        started: Instant,
    ) -> (Sender<Msg>, Instant, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel();
        let writer = SessionWriter::create(path).unwrap();
        let worker = Worker::new(path.to_path_buf(), writer, events().0, paced, started);
        (tx, started, thread::spawn(move || worker.run(&rx)))
    }

    fn tone_second(at: Instant) -> Msg {
        let data = (0..16_000)
            .map(|i| f32::sin(2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0) * 0.4)
            .collect();
        Msg::Frames {
            source: Source::Mic,
            rate: 16_000,
            channels: 1,
            data,
            at,
        }
    }

    fn stop(tx: &Sender<Msg>) -> Finished {
        let (reply, answer) = mpsc::channel();
        tx.send(Msg::Stop(reply)).unwrap();
        answer.recv().unwrap().unwrap()
    }

    /// A capture whose microphone is a thread that delivers 100 ms of tone `after` from now, as
    /// a host does with audio it was still holding. `None` delivers nothing.
    fn capture_with_slow_host(path: &Path, after: Option<Duration>) -> Capture {
        let (tx, rx) = mpsc::channel();
        let started = Instant::now();
        let mut capture = Capture::with_worker(
            path.to_path_buf(),
            events().0,
            tx.clone(),
            rx,
            true,
            started,
        )
        .unwrap();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            // A closed stream drops what the host still held, so a stop signal before `after`
            // ends the thread without delivering.
            if let Some(after) = after
                && stop_rx.recv_timeout(after).is_err()
            {
                let data = (0..1_600)
                    .map(|i| {
                        f32::sin(2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0) * 0.4
                    })
                    .collect();
                let _ = tx.send(Msg::Frames {
                    source: Source::Mic,
                    rate: 16_000,
                    channels: 1,
                    data,
                    at: Instant::now(),
                });
            }
            let _ = stop_rx.recv();
            let _ = done_tx.send(());
        });
        capture.mic = Some(MicHandle {
            id: 0,
            stop: stop_tx,
            done: done_rx,
        });
        capture
    }

    #[test]
    fn a_boost_raises_quiet_audio_and_bends_loud_audio_below_full_scale() {
        let mut same = vec![0.2, -0.5, 0.9];
        boost(&mut same, 1.0);
        assert_eq!(same, [0.2, -0.5, 0.9]);

        let mut raised = vec![0.2, -0.2, 0.0];
        boost(&mut raised, 1.5);
        assert!((raised[0] - 0.3).abs() < 1e-6 && (raised[1] + 0.3).abs() < 1e-6);
        assert_eq!(raised[2], 0.0);

        let mut loud: Vec<f32> = (0..=100).map(|i| i as f32 / 100.0).collect();
        boost(&mut loud, 2.0);
        assert!(
            loud.windows(2).all(|pair| pair[0] <= pair[1]),
            "it keeps the order"
        );
        assert!(loud.iter().all(|s| *s < 1.0), "nothing reaches full scale");
        assert!(loud[100] > 0.95, "a full sample stays loud: {}", loud[100]);

        let mut wild = vec![0.25];
        boost(&mut wild, 9.0);
        assert!((wild[0] - 0.5).abs() < 1e-6, "the gain stops at 200%");
        boost(&mut wild, f32::NAN);
        assert!(
            (wild[0] - 0.5).abs() < 1e-6,
            "a broken gain changes nothing"
        );
    }

    fn loudest_after(source: Source, gain: f32, enhance: Enhance) -> i16 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let (tx, rx) = mpsc::channel();
        let writer = SessionWriter::create(&path).unwrap();
        let mut worker = Worker::new(path.clone(), writer, events().0, false, Instant::now());
        worker.gain = Arc::new(AtomicU32::new(gain.to_bits()));
        worker.enhance = Arc::new(AtomicU64::new(enhance.to_bits()));
        let running = thread::spawn(move || worker.run(&rx));
        if source == Source::Feed {
            tx.send(Msg::FeedStart).unwrap();
        }
        let Msg::Frames {
            rate,
            channels,
            data,
            at,
            ..
        } = tone_second(Instant::now())
        else {
            unreachable!()
        };
        tx.send(Msg::Frames {
            source,
            rate,
            channels,
            data,
            at,
        })
        .unwrap();
        if source == Source::Feed {
            tx.send(Msg::FeedEnd).unwrap();
        }
        let finished = stop(&tx);
        running.join().unwrap();
        hound::WavReader::open(&finished.path)
            .unwrap()
            .samples::<i16>()
            .map(|s| s.unwrap().saturating_abs())
            .max()
            .unwrap()
    }

    #[test]
    fn the_boost_raises_the_microphone_in_the_file_and_never_a_test_feed() {
        let plain = loudest_after(Source::Mic, 1.0, Enhance::OFF);
        let boosted = loudest_after(Source::Mic, 1.5, Enhance::OFF);
        let ratio = f32::from(boosted) / f32::from(plain);
        assert!((ratio - 1.5).abs() < 0.05, "{plain} -> {boosted}");
        assert_eq!(loudest_after(Source::Feed, 2.0, Enhance::OFF), plain);
    }

    #[test]
    fn the_enhance_shapes_the_microphone_in_the_file_and_never_a_test_feed() {
        let plain = loudest_after(Source::Mic, 1.0, Enhance::OFF);
        let cut = Enhance {
            low_cut_hz: 1_000,
            ..Enhance::OFF
        };
        let shaped = loudest_after(Source::Mic, 1.0, cut);
        assert!(
            shaped < plain / 4,
            "440 Hz under a 1 kHz low cut: {plain} -> {shaped}"
        );
        assert_eq!(loudest_after(Source::Feed, 1.0, cut), plain);
    }

    #[test]
    fn noise_reduction_keeps_the_length_of_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let (tx, rx) = mpsc::channel();
        let writer = SessionWriter::create(&path).unwrap();
        let mut worker = Worker::new(path.clone(), writer, events().0, false, Instant::now());
        let on = Enhance {
            noise_reduction: true,
            ..Enhance::AUTO
        };
        worker.enhance = Arc::new(AtomicU64::new(on.to_bits()));
        let running = thread::spawn(move || worker.run(&rx));
        tx.send(tone_second(Instant::now())).unwrap();
        tx.send(tone_second(Instant::now())).unwrap();
        let finished = stop(&tx);
        running.join().unwrap();
        assert_eq!(finished.samples, 32_000, "2 s at 16 kHz, none held back");
    }

    #[test]
    fn audio_the_host_hands_over_after_the_stop_request_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let capture = capture_with_slow_host(&path, Some(Duration::from_millis(90)));
        thread::sleep(Duration::from_millis(10));
        let finished = capture.stop().unwrap();

        let samples: Vec<i16> = hound::WavReader::open(&finished.path)
            .unwrap()
            .samples::<i16>()
            .map(Result::unwrap)
            .collect();
        assert!(
            samples.iter().any(|s| s.abs() > 5_000),
            "the 100 ms the host still held is a tone in the file, not silence"
        );
    }

    #[test]
    fn a_silent_host_delays_the_stop_only_up_to_the_drain_limit() {
        let dir = tempfile::tempdir().unwrap();
        let capture = capture_with_slow_host(&dir.path().join("a.wav"), None);
        let asked = Instant::now();
        capture.stop().unwrap();
        let took = asked.elapsed();
        assert!(
            (TAIL_DRAIN_MAX..TAIL_DRAIN_MAX + Duration::from_millis(500)).contains(&took),
            "took {took:?}"
        );
    }

    #[test]
    fn a_late_first_delivery_leaves_the_lead_as_silence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let (tx, started, handle) = worker(&path, true);
        tx.send(tone_second(started + Duration::from_secs(3)))
            .unwrap();
        let finished = stop(&tx);
        handle.join().unwrap();

        assert!(
            (2_950..=3_050).contains(&finished.duration_ms),
            "2 s of lead then 1 s of tone, got {} ms",
            finished.duration_ms
        );
        let samples: Vec<i16> = hound::WavReader::open(&path)
            .unwrap()
            .samples::<i16>()
            .map(Result::unwrap)
            .collect();
        assert!(samples[..30_000].iter().all(|s| *s == 0));
        assert!(samples[34_000..].iter().any(|s| s.abs() > 5_000));
    }

    #[test]
    fn a_pause_in_delivery_becomes_silence_and_later_audio_keeps_its_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let (tx, started, handle) = worker(&path, true);
        tx.send(tone_second(started + Duration::from_secs(1)))
            .unwrap();
        tx.send(tone_second(started + Duration::from_secs(4)))
            .unwrap();
        let finished = stop(&tx);
        handle.join().unwrap();

        assert!(
            (3_950..=4_050).contains(&finished.duration_ms),
            "1 s tone, 2 s pause, 1 s tone, got {} ms",
            finished.duration_ms
        );
    }

    #[test]
    fn delivery_jitter_is_not_padded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let (tx, started, handle) = worker(&path, true);
        tx.send(tone_second(started + Duration::from_millis(1_100)))
            .unwrap();
        tx.send(tone_second(started + Duration::from_millis(2_150)))
            .unwrap();
        let finished = stop(&tx);
        handle.join().unwrap();

        assert_eq!(finished.duration_ms, 2_000);
    }

    #[test]
    fn the_end_of_a_session_is_padded_to_the_wall_clock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let (tx, _, handle) = worker(&path, true);
        thread::sleep(Duration::from_millis(500));
        let finished = stop(&tx);
        handle.join().unwrap();

        assert!(
            (480..=700).contains(&finished.duration_ms),
            "an idle source still gives the elapsed time, got {} ms",
            finished.duration_ms
        );
    }

    #[test]
    fn time_spent_opening_the_stream_counts_toward_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let start_action = Instant::now() - Duration::from_millis(1_100);
        let (tx, _, handle) = worker_since(&path, true, start_action);
        let finished = stop(&tx);
        handle.join().unwrap();

        assert!(
            (1_100..=1_300).contains(&finished.duration_ms),
            "the 1.1 s before the worker existed is in the file, got {} ms",
            finished.duration_ms
        );
    }

    #[test]
    fn a_two_second_session_with_a_late_first_callback_matches_the_wall_clock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let start_action = Instant::now() - Duration::from_millis(2_000);
        let (tx, _, handle) = worker_since(&path, true, start_action);
        // the stream opened late and the first delivery, 1 s of audio, arrives only now
        tx.send(tone_second(Instant::now())).unwrap();
        let finished = stop(&tx);
        handle.join().unwrap();

        let wall_ms = start_action.elapsed().as_millis() as i64;
        assert!(
            (finished.duration_ms as i64 - wall_ms).abs() <= 300,
            "wall {wall_ms} ms, file {} ms",
            finished.duration_ms
        );
    }

    #[test]
    fn an_unpaced_session_writes_only_what_arrives() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let (tx, started, handle) = worker(&path, false);
        tx.send(tone_second(started + Duration::from_secs(3)))
            .unwrap();
        let finished = stop(&tx);
        handle.join().unwrap();

        assert_eq!(finished.duration_ms, 1_000);
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
                CaptureEvent::Error(_) | CaptureEvent::Started | CaptureEvent::StartFailed(_) => {
                    None
                }
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
