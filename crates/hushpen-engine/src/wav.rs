//! Reads the WAV files the app hands to the engine child.

use hound::{SampleFormat, WavReader};
use std::path::Path;

pub const SAMPLE_RATE: u32 = 16_000;

/// Reads a 16 kHz WAV as mono `f32`. More than one channel is averaged. Another sample rate
/// is refused: capture and import resample before the engine sees the file.
pub fn read_mono_16k(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = WavReader::open(path).map_err(|error| error.to_string())?;
    let spec = reader.spec();
    if spec.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "sample rate is {} Hz, expected {SAMPLE_RATE} Hz",
            spec.sample_rate
        ));
    }
    let channels = usize::from(spec.channels.max(1));
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?,
        (SampleFormat::Int, bits @ 8..=32) => {
            let scale = 2_f32.powi(i32::from(bits) - 1);
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|error| error.to_string())?
        }
        (format, bits) => return Err(format!("unsupported sample format {format:?} {bits} bit")),
    };
    if channels == 1 {
        return Ok(samples);
    }
    Ok(samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{WavSpec, WavWriter};

    fn write(path: &Path, rate: u32, channels: u16, samples: &[i16]) {
        let spec = WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec).unwrap();
        for sample in samples {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn mono_16k_samples_are_scaled_to_unit_range() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write(&path, 16_000, 1, &[0, 16384, -32768]);
        assert_eq!(read_mono_16k(&path).unwrap(), vec![0.0, 0.5, -1.0]);
    }

    #[test]
    fn channels_are_averaged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write(&path, 16_000, 2, &[16384, 0, -16384, -16384]);
        assert_eq!(read_mono_16k(&path).unwrap(), vec![0.25, -0.5]);
    }

    #[test]
    fn another_rate_or_a_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write(&path, 44_100, 1, &[1, 2]);
        assert!(read_mono_16k(&path).unwrap_err().contains("44100"));
        assert!(read_mono_16k(&dir.path().join("none.wav")).is_err());
    }

    #[test]
    fn a_file_that_is_not_a_wav_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        std::fs::write(&path, b"this is not audio").unwrap();
        assert!(read_mono_16k(&path).is_err());
    }
}
