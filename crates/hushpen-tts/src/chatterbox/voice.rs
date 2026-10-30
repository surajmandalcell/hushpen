//! The voice a Chatterbox model clones: the clip it hears, and the conditioning the speech
//! encoder makes of it. The conditioning is kept in a file beside the clip, so the encoder
//! runs once for each voice and model.

use super::audio;
use hushpen_core::chatterbox::{HEARD_CLIP_SECS, SAMPLE_RATE};
use std::io::Cursor;
use std::path::Path;

/// Speech this loud or quieter is raised, and speech this loud or louder is lowered, to
/// [`TARGET_RMS`]. A clip in between stays as it is.
const QUIET_RMS: f32 = 0.05;
const LOUD_RMS: f32 = 0.2;
const TARGET_RMS: f32 = 0.1;
/// A clip with less voice than this cannot be cloned.
const MIN_VOICE_SECS: f32 = 1.0;
const MAGIC: &[u8; 8] = b"HPCOND01";

/// The clip as a model hears it: 24 kHz mono, without the silence around the voice, at most
/// [`HEARD_CLIP_SECS`] long, at an even loudness. `hash` tells a changed clip from its old
/// conditioning.
pub struct Clip {
    pub samples: Vec<f32>,
    pub hash: u64,
}

/// One tensor of the conditioning: its shape and its values.
#[derive(Debug, Clone, PartialEq)]
pub struct Tensor<T> {
    pub shape: Vec<i64>,
    pub data: Vec<T>,
}

/// What the speech encoder makes of a clip.
#[derive(Debug, Clone, PartialEq)]
pub struct Conditioning {
    /// The clip as the language model reads it, before the text: `[1, frames, hidden]`.
    pub features: Tensor<f32>,
    /// The speech tokens of the clip, which the decoder continues: `[1, tokens]`.
    pub prompt: Tensor<i64>,
    /// The speaker embedding `[1, 192]` and the speaker features `[1, rows, 80]`.
    pub speaker: Tensor<f32>,
    pub speaker_features: Tensor<f32>,
}

pub fn read_clip(path: &Path) -> Result<Clip, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read the clip: {error}"))?;
    let (samples, rate) = decode_wav(&bytes)?;
    Ok(Clip {
        samples: prepare(&samples, rate)?,
        hash: fnv(&bytes),
    })
}

/// Mono samples of a WAV file and their rate.
fn decode_wav(bytes: &[u8]) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|error| format!("the clip is not a WAV file: {error}"))?;
    let spec = reader.spec();
    let unreadable = |error: hound::Error| format!("the clip cannot be read: {error}");
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(unreadable)?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1_u64 << spec.bits_per_sample.clamp(1, 32).saturating_sub(1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(unreadable)?
        }
    };
    let channels = usize::from(spec.channels.max(1));
    let mono = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect();
    Ok((mono, spec.sample_rate))
}

/// The clip at 24 kHz, trimmed, cut to the heard length, and at an even loudness.
pub fn prepare(samples: &[f32], rate: u32) -> Result<Vec<f32>, String> {
    let resampled = resample(samples, rate, SAMPLE_RATE);
    let voiced = audio::trim(&resampled);
    let rate = SAMPLE_RATE as f32;
    if (voiced.len() as f32) < rate * MIN_VOICE_SECS {
        return Err("the clip holds less than a second of voice".into());
    }
    let heard = &voiced[..voiced.len().min((rate * HEARD_CLIP_SECS) as usize)];
    let rms = (heard.iter().map(|s| s * s).sum::<f32>() / heard.len() as f32).sqrt();
    let gain = if (QUIET_RMS..=LOUD_RMS).contains(&rms) {
        1.0
    } else {
        TARGET_RMS / rms
    };
    Ok(heard
        .iter()
        .map(|sample| (sample * gain).clamp(-1.0, 1.0))
        .collect())
}

/// Linear resampling. The voice library already saves clips at 24 kHz, so this only serves
/// a clip from somewhere else.
fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || from == 0 || samples.is_empty() {
        return samples.to_vec();
    }
    let step = f64::from(from) / f64::from(to);
    let length = (samples.len() as f64 / step) as usize;
    (0..length)
        .map(|index| {
            let at = index as f64 * step;
            let left = at as usize;
            let right = (left + 1).min(samples.len() - 1);
            let part = (at - left as f64) as f32;
            samples[left] * (1.0 - part) + samples[right] * part
        })
        .collect()
}

/// FNV-1a over the bytes of the clip file.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The conditioning kept at `path` for the clip with `hash`, if there is one for it.
pub fn load(path: &Path, hash: u64) -> Option<Conditioning> {
    let bytes = std::fs::read(path).ok()?;
    let mut reader = Reader { bytes: &bytes };
    if reader.take(MAGIC.len())? != MAGIC || reader.u64()? != hash {
        return None;
    }
    let float = |bytes: &[u8]| Some(f32::from_le_bytes(bytes.try_into().ok()?));
    let conditioning = Conditioning {
        features: reader.tensor(float)?,
        prompt: reader.tensor(|bytes| Some(i64::from_le_bytes(bytes.try_into().ok()?)))?,
        speaker: reader.tensor(float)?,
        speaker_features: reader.tensor(float)?,
    };
    reader.bytes.is_empty().then_some(conditioning)
}

/// Writes the conditioning through a temporary file, so a crash never leaves half of one.
pub fn save(path: &Path, hash: u64, conditioning: &Conditioning) -> std::io::Result<()> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend(hash.to_le_bytes());
    put(&mut bytes, &conditioning.features, |value| {
        value.to_le_bytes()
    });
    put(&mut bytes, &conditioning.prompt, |value| {
        value.to_le_bytes()
    });
    put(&mut bytes, &conditioning.speaker, |value| {
        value.to_le_bytes()
    });
    put(&mut bytes, &conditioning.speaker_features, |value| {
        value.to_le_bytes()
    });
    let partial = path.with_extension("cond.part");
    std::fs::write(&partial, &bytes)?;
    std::fs::rename(&partial, path)
}

fn put<T: Copy, const N: usize>(bytes: &mut Vec<u8>, tensor: &Tensor<T>, le: fn(T) -> [u8; N]) {
    bytes.extend(u32::try_from(tensor.shape.len()).unwrap_or(0).to_le_bytes());
    for dim in &tensor.shape {
        bytes.extend(dim.to_le_bytes());
    }
    for value in &tensor.data {
        bytes.extend(le(*value));
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.bytes.split_at_checked(count)?;
        self.bytes = rest;
        Some(head)
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn tensor<T>(&mut self, value: impl Fn(&[u8]) -> Option<T>) -> Option<Tensor<T>> {
        let rank = u32::from_le_bytes(self.take(4)?.try_into().ok()?);
        let mut shape = Vec::new();
        for _ in 0..rank.min(8) {
            shape.push(i64::from_le_bytes(self.take(8)?.try_into().ok()?));
        }
        let count = shape.iter().try_fold(1_usize, |count, dim| {
            count.checked_mul(usize::try_from(*dim).ok()?)
        })?;
        let size = std::mem::size_of::<T>();
        let raw = self.take(count.checked_mul(size)?)?;
        let data = raw
            .chunks_exact(size)
            .map(&value)
            .collect::<Option<Vec<T>>>()?;
        Some(Tensor { shape, data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, seconds: f32, level: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|index| level * (std::f32::consts::TAU * 220.0 * index as f32 / rate as f32).sin())
            .collect()
    }

    fn conditioning() -> Conditioning {
        Conditioning {
            features: Tensor {
                shape: vec![1, 2, 3],
                data: vec![0.5, -1.0, 2.0, 3.5, 0.0, 1e-3],
            },
            prompt: Tensor {
                shape: vec![1, 4],
                data: vec![1, 6561, 4299, 0],
            },
            speaker: Tensor {
                shape: vec![1, 2],
                data: vec![0.25, -0.25],
            },
            speaker_features: Tensor {
                shape: vec![1, 1, 2],
                data: vec![9.0, 8.0],
            },
        }
    }

    #[test]
    fn a_saved_conditioning_reads_back_only_for_its_clip() {
        let dir = std::env::temp_dir().join(format!("hushpen-cond-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("reference.chatterbox-turbo.cond");
        save(&path, 7, &conditioning()).unwrap();
        assert_eq!(load(&path, 7), Some(conditioning()));
        assert_eq!(load(&path, 8), None, "another clip");
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.pop();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(load(&path, 7), None, "a cut file");
        assert_eq!(load(&dir.join("none.cond"), 7), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_clip_is_trimmed_cut_to_ten_seconds_and_levelled() {
        let mut samples = vec![0.0; 16_000];
        samples.extend(tone(16_000, 14.0, 0.01));
        let clip = prepare(&samples, 16_000).unwrap();
        let seconds = clip.len() as f32 / SAMPLE_RATE as f32;
        assert!((seconds - HEARD_CLIP_SECS).abs() < 0.01, "{seconds}");
        let rms = (clip.iter().map(|s| s * s).sum::<f32>() / clip.len() as f32).sqrt();
        assert!((rms - TARGET_RMS).abs() < 0.005, "{rms}");

        let even = tone(SAMPLE_RATE, 3.0, 0.15);
        let kept = prepare(&even, SAMPLE_RATE).unwrap();
        assert!(
            kept.iter().zip(&even).all(|(a, b)| a == b),
            "a clip at a good level stays"
        );
    }

    #[test]
    fn a_short_or_silent_clip_is_refused() {
        assert!(prepare(&tone(SAMPLE_RATE, 0.5, 0.2), SAMPLE_RATE).is_err());
        assert!(prepare(&vec![0.0; 48_000], SAMPLE_RATE).is_err());
        assert!(decode_wav(b"not a wav").is_err());
    }

    #[test]
    fn a_stereo_wav_of_any_rate_reads_as_mono() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut bytes = Cursor::new(Vec::new());
        let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
        for _ in 0..480 {
            writer.write_sample(16_384_i16).unwrap();
            writer.write_sample(0_i16).unwrap();
        }
        writer.finalize().unwrap();
        let (mono, rate) = decode_wav(bytes.get_ref()).unwrap();
        assert_eq!((mono.len(), rate), (480, 48_000));
        assert!((mono[0] - 0.25).abs() < 1e-4);
        assert_eq!(resample(&mono, 48_000, 24_000).len(), 240);
    }
}
