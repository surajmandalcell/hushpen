//! Playback of a saved WAV on the default output device, with pause, seek, and a position.
//!
//! The file is decoded into memory when the player opens, so seeking is instant and the output
//! callback only reads memory. The output stream opens on a thread of its own at the first
//! `play`, because opening one takes up to a second on PulseAudio and the window must not wait
//! for it. The stream closes when the player is dropped.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, StreamConfig, SupportedBufferSize};
use std::fmt;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackError(pub String);

impl fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlaybackError {}

/// The decoded audio and where the output is in it. The output callback and the window share
/// one of these.
struct Track {
    /// Mono samples at `rate`.
    samples: Arc<[f32]>,
    rate: u32,
    /// The next sample to play, in `samples`, with a fraction.
    position: f64,
    playing: bool,
    /// The sample rate of the output device. Zero until the output has opened.
    out_rate: u32,
    error: Option<String>,
}

impl Track {
    /// Writes the next frames of the track, or silence while it is paused or ended. Linear
    /// interpolation turns the file rate into the device rate.
    fn fill(&mut self, out: &mut [f32], channels: usize) {
        let step = f64::from(self.rate) / f64::from(self.out_rate.max(1));
        let length = self.samples.len();
        for frame in out.chunks_mut(channels.max(1)) {
            let mut value = 0.0;
            if self.playing {
                let index = self.position as usize;
                if index >= length {
                    self.playing = false;
                    self.position = length as f64;
                } else {
                    let current = self.samples[index];
                    let next = self.samples.get(index + 1).copied().unwrap_or(current);
                    let fraction = (self.position - index as f64) as f32;
                    value = current + (next - current) * fraction;
                    self.position += step;
                }
            }
            frame.fill(value);
        }
    }

    fn position_ms(&self) -> u64 {
        (self.position / f64::from(self.rate) * 1000.0) as u64
    }
}

fn lock(track: &Mutex<Track>) -> MutexGuard<'_, Track> {
    track
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct Player {
    track: Arc<Mutex<Track>>,
    duration_ms: u64,
    requests: Option<Sender<()>>,
    started: bool,
    /// Never opens an output device: `play` only marks the track as playing.
    silent: bool,
}

impl Player {
    /// Decodes the file. Opens no device.
    pub fn open(path: &Path) -> Result<Self, PlaybackError> {
        let (samples, rate) = decode(path)?;
        let duration_ms = (samples.len() as u64 * 1000) / u64::from(rate);
        let track = Arc::new(Mutex::new(Track {
            samples: samples.into(),
            rate,
            position: 0.0,
            playing: false,
            out_rate: 0,
            error: None,
        }));
        Ok(Self {
            track,
            duration_ms,
            requests: None,
            started: false,
            silent: false,
        })
    }

    /// A player for tests of the code around it: it opens no device and its position stays
    /// where `seek_ms` puts it.
    pub fn silent(path: &Path) -> Result<Self, PlaybackError> {
        let mut player = Self::open(path)?;
        player.silent = true;
        Ok(player)
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub fn position_ms(&self) -> u64 {
        lock(&self.track).position_ms().min(self.duration_ms)
    }

    pub fn is_playing(&self) -> bool {
        lock(&self.track).playing
    }

    /// Why the output could not open or stopped, once it has.
    pub fn error(&self) -> Option<String> {
        lock(&self.track).error.clone()
    }

    /// Starts or resumes. From the end it starts again from the beginning.
    pub fn play(&mut self) {
        {
            let mut track = lock(&self.track);
            if track.position as usize >= track.samples.len() {
                track.position = 0.0;
            }
            track.error = None;
            track.playing = true;
        }
        if !self.started && !self.silent {
            self.started = true;
            let (requests, queue) = mpsc::channel();
            let shared = Arc::clone(&self.track);
            let spawned = thread::Builder::new()
                .name("hushpen-playback".into())
                .spawn(move || run(&queue, &shared));
            match spawned {
                Ok(_) => self.requests = Some(requests),
                Err(error) => self.fail(format!("the playback thread did not start: {error}")),
            }
        }
    }

    pub fn pause(&mut self) {
        lock(&self.track).playing = false;
    }

    /// Moves to `ms`, keeping playing or paused as it is.
    pub fn seek_ms(&mut self, ms: u64) {
        let mut track = lock(&self.track);
        let ms = ms.min(self.duration_ms);
        track.position = ms as f64 / 1000.0 * f64::from(track.rate);
    }

    fn fail(&self, message: String) {
        let mut track = lock(&self.track);
        track.playing = false;
        track.error = Some(message);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // The thread ends when its queue closes, which drops the stream.
        self.requests = None;
    }
}

fn decode(path: &Path) -> Result<(Vec<f32>, u32), PlaybackError> {
    let fail =
        |error: &dyn fmt::Display| PlaybackError(format!("could not read the audio: {error}"));
    let mut reader = hound::WavReader::open(path).map_err(|error| fail(&error))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels.max(1));
    if spec.sample_rate == 0 {
        return Err(PlaybackError("the audio has no sample rate".into()));
    }
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|error| fail(&error))?,
        (hound::SampleFormat::Int, bits @ 8..=32) => {
            let scale = (1_i64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|error| fail(&error))?
        }
        _ => return Err(PlaybackError("the audio format is not supported".into())),
    };
    let mono = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok((mono, spec.sample_rate))
}

/// The output thread: opens the device and holds the stream until the player is dropped.
fn run(queue: &Receiver<()>, track: &Arc<Mutex<Track>>) {
    let stream = match open_output(track) {
        Ok(stream) => stream,
        Err(message) => {
            log::warn!("could not open the output device for playback: {message}");
            let mut track = lock(track);
            track.playing = false;
            track.error = Some("The sound output could not be opened.".into());
            return;
        }
    };
    while queue.recv().is_ok() {}
    drop(stream);
}

fn open_output(track: &Arc<Mutex<Track>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("there is no output device")?;
    let config = device
        .default_output_config()
        .map_err(|error| error.to_string())?;
    let mut stream_config = config.config();
    let (rate, channels) = (
        stream_config.sample_rate,
        usize::from(stream_config.channels),
    );
    lock(track).out_rate = rate;
    let build = |stream_config: StreamConfig| match config.sample_format() {
        SampleFormat::F32 => typed::<f32>(&device, stream_config, channels, track),
        SampleFormat::I16 => typed::<i16>(&device, stream_config, channels, track),
        SampleFormat::U16 => typed::<u16>(&device, stream_config, channels, track),
        SampleFormat::I32 => typed::<i32>(&device, stream_config, channels, track),
        other => {
            log::warn!("unsupported output sample format {other:?}");
            Err(cpal::Error::new(cpal::ErrorKind::UnsupportedConfig))
        }
    };
    // PulseAudio queues about 2 s of output unless a buffer size is requested. The position
    // would run that far ahead of the sound, and a seek would play 2 s of the old place first.
    let mut built = None;
    if let SupportedBufferSize::Range { min, max } = *config.buffer_size() {
        stream_config.buffer_size = BufferSize::Fixed((rate / 10).clamp(min, max));
        built = build(stream_config).ok();
        stream_config.buffer_size = BufferSize::Default;
    }
    let stream = match built {
        Some(stream) => stream,
        None => build(stream_config).map_err(|error| error.to_string())?,
    };
    stream.play().map_err(|error| error.to_string())?;
    Ok(stream)
}

fn typed<T>(
    device: &cpal::Device,
    config: StreamConfig,
    channels: usize,
    track: &Arc<Mutex<Track>>,
) -> Result<cpal::Stream, cpal::Error>
where
    T: SizedSample + FromSample<f32> + Send + 'static,
{
    let data_track = Arc::clone(track);
    let error_track = Arc::clone(track);
    let mut scratch: Vec<f32> = Vec::new();
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            scratch.resize(data.len(), 0.0);
            lock(&data_track).fill(&mut scratch, channels);
            for (slot, value) in data.iter_mut().zip(&scratch) {
                *slot = T::from_sample_(*value);
            }
        },
        move |error| {
            log::debug!("playback output stream: {error}");
            let mut track = lock(&error_track);
            track.playing = false;
            track.error = Some("The sound output stopped.".into());
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(samples: Vec<f32>, rate: u32, out_rate: u32) -> Track {
        Track {
            samples: samples.into(),
            rate,
            position: 0.0,
            playing: true,
            out_rate,
            error: None,
        }
    }

    fn write_wav(path: &Path, rate: u32, channels: u16, frames: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for index in 0..frames {
            for _ in 0..channels {
                writer.write_sample((index % 100) as i16 * 100).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn the_position_advances_with_the_frames_that_are_played() {
        let mut track = track(vec![0.5; 16_000], 16_000, 16_000);
        let mut out = vec![0.0; 8_000];
        track.fill(&mut out, 1);
        assert_eq!(track.position_ms(), 500);
        assert!(out.iter().all(|sample| (*sample - 0.5).abs() < 1e-6));
    }

    #[test]
    fn a_higher_output_rate_plays_the_same_time_and_fills_every_channel() {
        let mut track = track(vec![0.25; 16_000], 16_000, 48_000);
        let mut out = vec![0.0; 48_000 * 2];
        track.fill(&mut out, 2);
        assert_eq!(track.position_ms(), 1_000);
        assert!(out.chunks(2).all(|frame| frame[0] == frame[1]));
    }

    #[test]
    fn interpolation_runs_between_two_samples() {
        let mut track = track(vec![0.0, 1.0, 1.0], 1, 2);
        let mut out = vec![0.0; 2];
        track.fill(&mut out, 1);
        assert_eq!(out, vec![0.0, 0.5]);
    }

    #[test]
    fn a_paused_track_is_silent_and_keeps_its_position() {
        let mut track = track(vec![1.0; 16_000], 16_000, 16_000);
        track.position = 4_000.0;
        track.playing = false;
        let mut out = vec![1.0; 1_000];
        track.fill(&mut out, 1);
        assert!(out.iter().all(|sample| *sample == 0.0));
        assert_eq!(track.position_ms(), 250);
    }

    #[test]
    fn the_end_stops_the_track_and_the_rest_of_the_buffer_is_silent() {
        let mut track = track(vec![1.0; 100], 16_000, 16_000);
        let mut out = vec![0.0; 300];
        track.fill(&mut out, 1);
        assert!(!track.playing);
        assert_eq!(track.position, 100.0);
        assert!(out[..100].iter().all(|sample| *sample == 1.0));
        assert!(out[100..].iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn a_file_decodes_to_mono_with_its_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, 16_000, 1, 32_000);
        let player = Player::open(&path).unwrap();
        assert_eq!(player.duration_ms(), 2_000);
        assert_eq!(player.position_ms(), 0);
        assert!(!player.is_playing());

        let stereo = dir.path().join("b.wav");
        write_wav(&stereo, 8_000, 2, 8_000);
        assert_eq!(Player::open(&stereo).unwrap().duration_ms(), 1_000);
    }

    #[test]
    fn seek_moves_the_position_and_clamps_to_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, 16_000, 1, 32_000);
        let mut player = Player::open(&path).unwrap();
        player.seek_ms(1_000);
        assert_eq!(player.position_ms(), 1_000);
        player.seek_ms(60_000);
        assert_eq!(player.position_ms(), 2_000);
        player.seek_ms(0);
        assert_eq!(player.position_ms(), 0);
        assert!(!player.is_playing());
    }

    #[test]
    fn a_file_that_is_not_a_wav_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.wav");
        std::fs::write(&path, b"not audio").unwrap();
        assert!(Player::open(&path).is_err());
        assert!(Player::open(&dir.path().join("missing.wav")).is_err());
    }
}
