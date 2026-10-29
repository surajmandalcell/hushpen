//! Cue sounds: short sounds for start, stop, and cancel, played on the default output device.
//!
//! The three sounds are WAV files built into the program (`assets/sounds`: mono, 48 kHz,
//! 16-bit, peak at full scale, faded at both ends), resampled to the output rate when they play.
//! A [`CuePlayer`] hands every request to
//! its own thread, so a cue never waits for the audio system and never delays the microphone.
//! The thread keeps the output stream open for minutes after a cue: opening one takes up to a
//! second on PulseAudio, and a start cue that waits that long is no cue. It closes the stream
//! when the app has been quiet for a long time.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, StreamConfig, SupportedBufferSize};
use hushpen_core::dictation::Cue;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// How long an output stream stays open after the last cue.
const KEEP_OPEN: Duration = Duration::from_secs(600);
/// The loudest sample of a cue at volume 1.0, so a full volume cue is loud but never clips.
const PEAK: f32 = 0.8;

const START: &[u8] = include_bytes!("../../../assets/sounds/cue-start.wav");
const SUCCESS: &[u8] = include_bytes!("../../../assets/sounds/cue-success.wav");
const CANCEL: &[u8] = include_bytes!("../../../assets/sounds/cue-cancel.wav");

fn wav(cue: Cue) -> &'static [u8] {
    match cue {
        Cue::Start => START,
        Cue::Stop => SUCCESS,
        Cue::Cancel => CANCEL,
    }
}

/// The samples of a built-in WAV as mono between -1.0 and 1.0, with their rate.
fn decode(bytes: &[u8]) -> (u32, Vec<f32>) {
    let reader = match hound::WavReader::new(Cursor::new(bytes)) {
        Ok(reader) => reader,
        Err(error) => {
            log::warn!("a cue sound does not decode: {error}");
            return (1, Vec::new());
        }
    };
    let spec = reader.spec();
    let channels = usize::from(spec.channels.max(1));
    let scale = 1.0 / (1_i64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.into_samples::<f32>().flatten().collect(),
        hound::SampleFormat::Int => reader
            .into_samples::<i32>()
            .flatten()
            .map(|s| s as f32 * scale)
            .collect(),
    };
    let mono = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect();
    (spec.sample_rate.max(1), mono)
}

/// `samples` at rate `from`, resampled to rate `to` by linear interpolation. A cue is short and
/// bright, so this is enough.
fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() {
        return samples.to_vec();
    }
    let count = (samples.len() as u64 * u64::from(to) / u64::from(from)) as usize;
    let step = f64::from(from) / f64::from(to);
    (0..count)
        .map(|i| {
            let at = i as f64 * step;
            let index = at as usize;
            let next = samples.get(index + 1).copied().unwrap_or(0.0);
            let here = samples.get(index).copied().unwrap_or(0.0);
            here + (next - here) * (at - index as f64) as f32
        })
        .collect()
}

/// The cue as mono samples at `rate`, between -[`PEAK`] and [`PEAK`] at full `volume`. The
/// volume is a linear gain from 0.0 to 1.0.
pub fn render(cue: Cue, rate: u32, volume: f32) -> Vec<f32> {
    let gain = PEAK * volume.clamp(0.0, 1.0);
    let (from, samples) = decode(wav(cue));
    resample(&samples, from, rate.max(1))
        .into_iter()
        .map(|sample| (sample * gain).clamp(-PEAK, PEAK))
        .collect()
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
            assert!((100..=500).contains(&ms), "{cue:?} lasts {ms} ms");
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
    fn each_cue_plays_its_own_sound_file() {
        assert_eq!(wav(Cue::Start), START);
        assert_eq!(wav(Cue::Stop), SUCCESS);
        assert_eq!(wav(Cue::Cancel), CANCEL);
        for bytes in [START, SUCCESS, CANCEL] {
            let (rate, samples) = decode(bytes);
            assert_eq!(rate, 48_000);
            assert!(!samples.is_empty());
        }
    }

    #[test]
    fn resampling_keeps_the_length_in_time_and_the_shape() {
        let ramp: Vec<f32> = (0..480).map(|i| i as f32 / 480.0).collect();
        let half = resample(&ramp, 48_000, 24_000);
        assert_eq!(half.len(), 240);
        assert!((half[120] - 0.5).abs() < 0.01, "{}", half[120]);
        let double = resample(&ramp, 48_000, 96_000);
        assert_eq!(double.len(), 960);
        assert!((double[1] - 0.5 / 480.0).abs() < 1e-6);
        assert_eq!(resample(&ramp, 48_000, 48_000), ramp);
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
