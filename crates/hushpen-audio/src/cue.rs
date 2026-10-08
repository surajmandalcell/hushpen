//! Cue sounds: short tones for start, stop, and cancel, played on the default output device.
//!
//! The tones are made here, so nothing ships as a file. A [`CuePlayer`] hands every request to
//! its own thread, so a cue never waits for the audio system and never delays the microphone.
//! The thread keeps the output stream open for minutes after a cue: opening one takes up to a
//! second on PulseAudio, and a start cue that waits that long is no cue. It closes the stream
//! when the app has been quiet for a long time.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, StreamConfig, SupportedBufferSize};
use hushpen_core::dictation::Cue;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// How long an output stream stays open after the last cue.
const KEEP_OPEN: Duration = Duration::from_secs(600);
/// The loudest sample of a cue at volume 1.0, so a full volume cue is loud but never clips.
const PEAK: f32 = 0.8;
/// Each note fades in and out over this long, so no note clicks.
const FADE_MS: f32 = 8.0;

/// One note: its pitch in hertz and its length in milliseconds.
type Note = (f32, f32);

fn notes(cue: Cue) -> &'static [Note] {
    match cue {
        Cue::Start => &[(660.0, 70.0), (990.0, 90.0)],
        Cue::Stop => &[(990.0, 70.0), (660.0, 90.0)],
        Cue::Cancel => &[(330.0, 80.0), (220.0, 120.0)],
    }
}

/// The cue as mono samples at `rate`, between -[`PEAK`] and [`PEAK`] at full `volume`. The
/// volume is a linear gain from 0.0 to 1.0.
pub fn render(cue: Cue, rate: u32, volume: f32) -> Vec<f32> {
    let gain = PEAK * volume.clamp(0.0, 1.0);
    let rate = rate.max(1) as f32;
    let mut samples = Vec::new();
    for &(hertz, ms) in notes(cue) {
        let count = (rate * ms / 1000.0) as usize;
        let fade = ((rate * FADE_MS / 1000.0) as usize).clamp(1, count.max(1) / 2);
        for i in 0..count {
            let edge = i.min(count - 1 - i);
            let envelope = if edge < fade {
                0.5 - 0.5 * (PI * edge as f32 / fade as f32).cos()
            } else {
                1.0
            };
            samples.push(gain * envelope * (2.0 * PI * hertz * i as f32 / rate).sin());
        }
    }
    samples
}

/// What the output callback plays: one cue, then silence.
#[derive(Default)]
struct Playing {
    samples: Vec<f32>,
    position: usize,
}

impl Playing {
    fn next(&mut self) -> f32 {
        let value = self.samples.get(self.position).copied().unwrap_or(0.0);
        self.position = (self.position + 1).min(self.samples.len());
        value
    }

    fn start(&mut self, samples: Vec<f32>) {
        self.samples = samples;
        self.position = 0;
    }
}

enum Request {
    Play { cue: Cue, volume: f32 },
    Warm,
}

/// Plays cues on a thread of its own. Cheap to clone the handle through `Arc`.
pub struct CuePlayer {
    requests: Sender<Request>,
}

impl Default for CuePlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl CuePlayer {
    pub fn new() -> Self {
        let (requests, queue) = mpsc::channel();
        if let Err(error) = thread::Builder::new()
            .name("hushpen-cues".into())
            .spawn(move || run(&queue))
        {
            log::warn!("the cue sound thread did not start: {error}");
        }
        Self { requests }
    }

    /// Plays `cue` at `volume` (0.0 to 1.0) and returns at once.
    pub fn play(&self, cue: Cue, volume: f32) {
        let _ = self.requests.send(Request::Play { cue, volume });
    }

    /// Opens the output ahead of the first cue and returns at once.
    pub fn warm(&self) {
        let _ = self.requests.send(Request::Warm);
    }
}

struct Output {
    _stream: cpal::Stream,
    playing: Arc<Mutex<Playing>>,
    failed: Arc<AtomicBool>,
    rate: u32,
    quiet_since: Instant,
}

impl Output {
    fn open(host: &cpal::Host) -> Option<Self> {
        let device = host.default_output_device()?;
        let config = device
            .default_output_config()
            .map_err(|error| log::warn!("the output device has no usable format: {error}"))
            .ok()?;
        let playing = Arc::new(Mutex::new(Playing::default()));
        let failed = Arc::new(AtomicBool::new(false));
        let mut stream_config = config.config();
        let (rate, channels) = (
            stream_config.sample_rate,
            usize::from(stream_config.channels),
        );
        let build = |stream_config: StreamConfig| match config.sample_format() {
            SampleFormat::F32 => typed::<f32>(&device, stream_config, channels, &playing, &failed),
            SampleFormat::I16 => typed::<i16>(&device, stream_config, channels, &playing, &failed),
            SampleFormat::U16 => typed::<u16>(&device, stream_config, channels, &playing, &failed),
            SampleFormat::I32 => typed::<i32>(&device, stream_config, channels, &playing, &failed),
            other => {
                log::warn!("unsupported output sample format {other:?}");
                Err(cpal::Error::new(cpal::ErrorKind::UnsupportedConfig))
            }
        };
        // PulseAudio queues about 2 s of output unless a buffer size is requested, and a cue
        // that waits behind that much silence arrives long after the key press.
        let mut built = None;
        if let SupportedBufferSize::Range { min, max } = *config.buffer_size() {
            stream_config.buffer_size = BufferSize::Fixed((rate / 50).clamp(min, max));
            built = build(stream_config).ok();
            stream_config.buffer_size = BufferSize::Default;
        }
        let stream = match built {
            Some(stream) => Ok(stream),
            None => build(stream_config),
        }
        .and_then(|stream| stream.play().map(|()| stream))
        .map_err(|error| log::warn!("could not open the output device for cue sounds: {error}"))
        .ok()?;
        Some(Self {
            _stream: stream,
            playing,
            failed,
            rate,
            quiet_since: Instant::now(),
        })
    }

    fn play(&mut self, cue: Cue, volume: f32) {
        let samples = render(cue, self.rate, volume);
        let length = Duration::from_secs_f32(samples.len() as f32 / self.rate.max(1) as f32);
        self.playing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .start(samples);
        self.quiet_since = Instant::now() + length;
    }

    fn stale(&self) -> bool {
        self.failed.load(Ordering::SeqCst) || self.quiet_since.elapsed() > KEEP_OPEN
    }
}

fn typed<T>(
    device: &cpal::Device,
    config: StreamConfig,
    channels: usize,
    playing: &Arc<Mutex<Playing>>,
    failed: &Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let playing = Arc::clone(playing);
    let failed = Arc::clone(failed);
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let mut playing = playing
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for frame in data.chunks_mut(channels.max(1)) {
                let value = T::from_sample_(playing.next());
                frame.fill(value);
            }
        },
        move |error| {
            log::debug!("cue output stream: {error}");
            failed.store(true, Ordering::SeqCst);
        },
        None,
    )
}

fn run(queue: &mpsc::Receiver<Request>) {
    let host = cpal::default_host();
    let mut output: Option<Output> = None;
    loop {
        let request = match queue.recv_timeout(Duration::from_secs(1)) {
            Ok(request) => Some(request),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if output.as_ref().is_some_and(Output::stale) {
            output = None;
        }
        let Some(request) = request else { continue };
        if output.is_none() {
            output = Output::open(&host);
        }
        match (request, output.as_mut()) {
            (Request::Play { cue, volume }, Some(output)) => output.play(cue, volume),
            (Request::Warm, _) | (Request::Play { .. }, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0, |peak, s| peak.max(s.abs()))
    }

    #[test]
    fn each_cue_is_short_audible_and_within_the_peak() {
        for cue in [Cue::Start, Cue::Stop, Cue::Cancel] {
            let samples = render(cue, 48_000, 1.0);
            let ms = samples.len() * 1000 / 48_000;
            assert!((100..=300).contains(&ms), "{cue:?} lasts {ms} ms");
            let loudest = peak(&samples);
            assert!(
                loudest > 0.5 && loudest <= PEAK + f32::EPSILON,
                "{cue:?} {loudest}"
            );
        }
    }

    #[test]
    fn the_three_cues_differ() {
        let start = render(Cue::Start, 48_000, 1.0);
        let stop = render(Cue::Stop, 48_000, 1.0);
        let cancel = render(Cue::Cancel, 48_000, 1.0);
        assert_ne!(start, stop);
        assert_ne!(start, cancel);
        assert_ne!(stop, cancel);
    }

    #[test]
    fn a_cue_starts_and_ends_near_silence_so_it_does_not_click() {
        for cue in [Cue::Start, Cue::Stop, Cue::Cancel] {
            let samples = render(cue, 44_100, 1.0);
            assert!(samples[0].abs() < 0.01, "{cue:?} start");
            assert!(samples[samples.len() - 1].abs() < 0.05, "{cue:?} end");
        }
    }

    #[test]
    fn a_cue_lasts_the_same_time_at_every_rate() {
        let slow = render(Cue::Start, 16_000, 1.0).len() as f32 / 16_000.0;
        let fast = render(Cue::Start, 96_000, 1.0).len() as f32 / 96_000.0;
        assert!((slow - fast).abs() < 0.002, "{slow} {fast}");
    }

    #[test]
    fn a_quarter_volume_cue_is_at_least_6_db_below_a_full_volume_one() {
        let full = peak(&render(Cue::Start, 48_000, 1.0));
        let quarter = peak(&render(Cue::Start, 48_000, 0.25));
        let db = 20.0 * (full / quarter).log10();
        assert!(db >= 6.0, "{db} dB");
    }

    #[test]
    fn a_volume_outside_the_range_is_clamped_and_zero_is_silent() {
        let full = peak(&render(Cue::Stop, 48_000, 1.0));
        assert_eq!(peak(&render(Cue::Stop, 48_000, 7.0)), full);
        assert_eq!(peak(&render(Cue::Stop, 48_000, 0.0)), 0.0);
        assert_eq!(peak(&render(Cue::Stop, 48_000, -1.0)), 0.0);
    }

    #[test]
    fn the_player_plays_one_cue_then_silence() {
        let mut playing = Playing::default();
        playing.start(vec![0.5, -0.5]);
        assert_eq!(
            [
                playing.next(),
                playing.next(),
                playing.next(),
                playing.next()
            ],
            [0.5, -0.5, 0.0, 0.0]
        );
        playing.start(vec![0.25]);
        assert_eq!(playing.next(), 0.25, "a new cue replaces the old one");
    }
}
