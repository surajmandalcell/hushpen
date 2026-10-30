//! The clip a voice is cloned from, made from a microphone take or an audio file: the silence
//! before and after the speech goes, at most the first 20 seconds of speech stay, the level is
//! raised to a steady peak, and the audio is resampled to the 24 kHz the voice worker reads.
//!
//! A file comes from outside, so it is decoded on a worker thread, and a panic in the decoder
//! is caught there. A clip is short, so the app decodes it itself rather than through the
//! engine child: at most [`READ_SECONDS`] of a file are read.

use hushpen_audio::decode::FileDecoder;
use hushpen_audio::resample::{MonoResampler, TARGET_RATE};
use hushpen_core::chatterbox::{MAX_CLIP_SECS, MIN_CLIP_SECS, SAMPLE_RATE};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

/// The rate of the audio the decoder and the microphone take give.
const INPUT_RATE: u32 = TARGET_RATE;
/// How much of a file is read. A longer file still gives a clip of its first speech.
pub const READ_SECONDS: u32 = 60;
/// 20 ms frames, in which the loudness is measured.
const FRAME: usize = INPUT_RATE as usize / 50;
/// A frame this much quieter than the loudest one is silence.
const QUIET: f32 = 0.05;
/// A frame below this level is silence, however quiet the voice is.
const FLOOR: f32 = 0.003;
/// What stays of the quiet before and after the speech, so no sound is cut short.
const MARGIN: usize = 5 * FRAME;
/// The peak a clip is brought to.
const PEAK: f32 = 0.9;

/// A clip ready to keep: 24 kHz mono.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub samples: Vec<f32>,
    /// The speech ran longer than 20 seconds, so the clip keeps its start.
    pub cut: bool,
}

impl Clip {
    pub fn duration_ms(&self) -> u64 {
        self.samples.len() as u64 * 1000 / u64::from(SAMPLE_RATE)
    }
}

/// Why no clip came of the audio, in words for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipProblem(pub String);

/// The clip of 16 kHz mono `audio`.
pub fn prepare(audio: &[f32]) -> Result<Clip, ClipProblem> {
    let speech = trim(audio);
    let seconds = speech.len() as f32 / INPUT_RATE as f32;
    if speech.is_empty() {
        return Err(ClipProblem(
            "No speech was heard. Record or choose a clip of someone speaking.".into(),
        ));
    }
    if seconds < MIN_CLIP_SECS {
        return Err(ClipProblem(format!(
            "The clip has {seconds:.0} s of speech. A voice needs at least {MIN_CLIP_SECS:.0} s."
        )));
    }
    let longest = (MAX_CLIP_SECS * INPUT_RATE as f32) as usize;
    let cut = speech.len() > longest;
    let speech = &speech[..speech.len().min(longest)];
    let peak = speech
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let gain = if peak > 0.0 { PEAK / peak } else { 1.0 };
    let level: Vec<f32> = speech.iter().map(|sample| sample * gain).collect();
    let mut resampler = MonoResampler::to_rate(INPUT_RATE, SAMPLE_RATE)
        .map_err(|error| ClipProblem(format!("The clip could not be made: {}", error.detail)))?;
    let mut samples = Vec::with_capacity(level.len() * 3 / 2 + 1);
    resampler.process(&level, &mut samples);
    resampler.finish(&mut samples);
    for sample in &mut samples {
        *sample = sample.clamp(-1.0, 1.0);
    }
    Ok(Clip { samples, cut })
}

/// `audio` from the first frame of speech to the last, with a little quiet on each side.
fn trim(audio: &[f32]) -> &[f32] {
    let levels: Vec<f32> = audio
        .chunks(FRAME)
        .map(|frame| (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt())
        .collect();
    let loudest = levels.iter().copied().fold(0.0_f32, f32::max);
    let threshold = (loudest * QUIET).max(FLOOR);
    let Some(first) = levels.iter().position(|level| *level > threshold) else {
        return &[];
    };
    let last = levels
        .iter()
        .rposition(|level| *level > threshold)
        .unwrap_or(first);
    let start = (first * FRAME).saturating_sub(MARGIN);
    let end = ((last + 1) * FRAME + MARGIN).min(audio.len());
    &audio[start..end]
}

/// The first [`READ_SECONDS`] of an audio file as 16 kHz mono. WAV, MP3, M4A, and the other
/// formats of the import decode.
pub fn read_file(path: &Path) -> Result<Vec<f32>, ClipProblem> {
    let read = catch_unwind(AssertUnwindSafe(|| {
        let mut decoder = FileDecoder::open(path)?;
        let limit = (READ_SECONDS * INPUT_RATE) as usize;
        let mut samples = Vec::new();
        while samples.len() < limit && decoder.read(&mut samples)? {}
        samples.truncate(limit);
        Ok(samples)
    }));
    match read {
        Ok(Ok(samples)) => Ok(samples),
        Ok(Err(hushpen_audio::decode::DecodeError::Unsupported(text))) => Err(ClipProblem(text)),
        Ok(Err(hushpen_audio::decode::DecodeError::Failed(detail))) => {
            log::warn!("VOICE_CLIP_UNREADABLE {detail}");
            Err(ClipProblem(
                "This file could not be read as audio. It may be damaged or empty.".into(),
            ))
        }
        Err(_) => {
            log::warn!("VOICE_CLIP_UNREADABLE the decoder panicked");
            Err(ClipProblem("This file could not be read as audio.".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `seconds` of a 220 Hz tone at `level`, the way speech would sit in a take.
    fn tone(seconds: f32, level: f32) -> Vec<f32> {
        (0..(seconds * INPUT_RATE as f32) as usize)
            .map(|n| level * (n as f32 * 220.0 * std::f32::consts::TAU / INPUT_RATE as f32).sin())
            .collect()
    }

    fn silence(seconds: f32) -> Vec<f32> {
        vec![0.0; (seconds * INPUT_RATE as f32) as usize]
    }

    #[test]
    fn the_quiet_around_the_speech_goes_and_the_clip_is_24_khz_at_a_steady_peak() {
        let mut audio = silence(2.0);
        audio.extend(tone(8.0, 0.1));
        audio.extend(silence(3.0));
        let clip = prepare(&audio).unwrap();
        assert!(!clip.cut);
        let seconds = clip.samples.len() as f32 / SAMPLE_RATE as f32;
        assert!((seconds - 8.2).abs() < 0.1, "{seconds}");
        assert!(clip.duration_ms().abs_diff(8_200) < 100);
        let peak = clip.samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
        assert!((peak - PEAK).abs() < 0.05, "{peak}");
    }

    #[test]
    fn long_speech_keeps_its_first_20_seconds() {
        let clip = prepare(&tone(45.0, 0.5)).unwrap();
        assert!(clip.cut);
        let seconds = clip.samples.len() as f32 / SAMPLE_RATE as f32;
        assert!((seconds - MAX_CLIP_SECS).abs() < 0.05, "{seconds}");
    }

    #[test]
    fn short_or_silent_audio_gives_no_clip() {
        let mut short = silence(6.0);
        short.extend(tone(3.0, 0.5));
        let problem = prepare(&short).unwrap_err();
        assert!(problem.0.contains("at least 5 s"), "{}", problem.0);
        let silent = prepare(&silence(10.0)).unwrap_err();
        assert!(silent.0.starts_with("No speech"), "{}", silent.0);
        assert!(prepare(&[]).is_err());
    }

    #[test]
    fn a_wav_file_reads_and_a_file_that_is_not_audio_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("take.wav");
        hushpen_audio::write_wav(&wav, 48_000, &vec![0.25; 48_000 * 2]).unwrap();
        let samples = read_file(&wav).unwrap();
        assert!(
            samples.len().abs_diff(2 * INPUT_RATE as usize) < 400,
            "{}",
            samples.len()
        );
        let text = dir.path().join("notes.mp3");
        std::fs::write(&text, b"not audio at all").unwrap();
        assert!(read_file(&text).is_err());
        assert!(read_file(&dir.path().join("gone.wav")).is_err());
    }
}
