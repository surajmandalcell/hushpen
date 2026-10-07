//! Any input rate and channel count to 16 kHz mono.

use crate::error::CaptureError;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

pub const TARGET_RATE: u32 = 16_000;

/// Frames in each FFT chunk. About 23 ms at 44.1 kHz.
const CHUNK_FRAMES: usize = 1024;

/// Averages the channels of interleaved frames into `out`.
pub fn downmix(interleaved: &[f32], channels: usize, out: &mut Vec<f32>) {
    let channels = channels.max(1);
    if channels == 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    out.extend(
        interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32),
    );
}

/// Streaming resampler for one mono stream. Output length is exact: after
/// `finish` the total equals `round(input_frames * 16000 / input_rate)`.
pub struct MonoResampler {
    input_rate: u32,
    fft: Option<Fft<f32>>,
    pending: Vec<f32>,
    scratch: Vec<f32>,
    delay_left: usize,
    input_frames: u64,
    output_frames: u64,
}

impl MonoResampler {
    pub fn new(input_rate: u32) -> Result<Self, CaptureError> {
        if input_rate == 0 {
            return Err(CaptureError::failed("the input sample rate is 0"));
        }
        let (fft, delay_left, scratch) = if input_rate == TARGET_RATE {
            (None, 0, Vec::new())
        } else {
            let fft = Fft::<f32>::new(
                input_rate as usize,
                TARGET_RATE as usize,
                CHUNK_FRAMES,
                1,
                FixedSync::Input,
            )
            .map_err(|error| CaptureError::failed(format!("resampler: {error}")))?;
            let delay = fft.output_delay();
            let scratch = vec![0.0; fft.output_frames_max()];
            (Some(fft), delay, scratch)
        };
        Ok(Self {
            input_rate,
            fft,
            pending: Vec::new(),
            scratch,
            delay_left,
            input_frames: 0,
            output_frames: 0,
        })
    }

    pub fn process(&mut self, mono: &[f32], out: &mut Vec<f32>) {
        self.input_frames += mono.len() as u64;
        let Some(fft) = self.fft.as_mut() else {
            self.output_frames += mono.len() as u64;
            out.extend_from_slice(mono);
            return;
        };
        self.pending.extend_from_slice(mono);
        let mut used = 0;
        loop {
            let need = fft.input_frames_next();
            if self.pending.len() - used < need {
                break;
            }
            let chunk = &self.pending[used..used + need];
            used += need;
            Self::run_chunk(
                fft,
                chunk,
                None,
                &mut self.scratch,
                &mut self.delay_left,
                &mut self.output_frames,
                out,
            );
        }
        self.pending.drain(..used);
    }

    /// Flushes the filter delay and cuts the output to its exact length.
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        let Some(fft) = self.fft.as_mut() else {
            return;
        };
        let expected = (self.input_frames * u64::from(TARGET_RATE)
            + u64::from(self.input_rate) / 2)
            / u64::from(self.input_rate);
        let need = fft.input_frames_next();
        let mut chunk = std::mem::take(&mut self.pending);
        let mut partial = Some(chunk.len());
        chunk.resize(need, 0.0);
        // Zero-filled chunks push out what the filter still holds.
        while self.output_frames < expected {
            Self::run_chunk(
                fft,
                &chunk,
                partial.take(),
                &mut self.scratch,
                &mut self.delay_left,
                &mut self.output_frames,
                out,
            );
            chunk.fill(0.0);
            partial = Some(0);
        }
        let excess = (self.output_frames - expected) as usize;
        out.truncate(out.len() - excess);
        self.output_frames = expected;
    }

    fn run_chunk(
        fft: &mut Fft<f32>,
        chunk: &[f32],
        partial_len: Option<usize>,
        scratch: &mut [f32],
        delay_left: &mut usize,
        output_frames: &mut u64,
        out: &mut Vec<f32>,
    ) {
        let Ok(input) = InterleavedSlice::new(chunk, 1, chunk.len()) else {
            return;
        };
        let capacity = scratch.len();
        let Ok(mut output) = InterleavedSlice::new_mut(scratch, 1, capacity) else {
            return;
        };
        let indexing = partial_len.map(|len| Indexing {
            input_offset: 0,
            output_offset: 0,
            partial_len: Some(len),
            active_channels_mask: None,
        });
        let Ok((_, written)) = fft.process_into_buffer(&input, &mut output, indexing.as_ref())
        else {
            return;
        };
        let skip = (*delay_left).min(written);
        *delay_left -= skip;
        out.extend_from_slice(&scratch[skip..written]);
        *output_frames += (written - skip) as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, seconds: f32, hz: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin())
            .collect()
    }

    fn run(rate: u32, input: &[f32], chunk: usize) -> Vec<f32> {
        let mut resampler = MonoResampler::new(rate).unwrap();
        let mut out = Vec::new();
        for piece in input.chunks(chunk) {
            resampler.process(piece, &mut out);
        }
        resampler.finish(&mut out);
        out
    }

    fn peak_frequency(samples: &[f32], rate: f32) -> f32 {
        let mut best = (0.0f32, 0.0f32);
        for hz in (100..4000).step_by(10) {
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (i, s) in samples.iter().enumerate() {
                let phase = 2.0 * std::f32::consts::PI * hz as f32 * i as f32 / rate;
                re += s * phase.cos();
                im += s * phase.sin();
            }
            let power = re * re + im * im;
            if power > best.1 {
                best = (hz as f32, power);
            }
        }
        best.0
    }

    #[test]
    fn output_length_is_exact_for_common_rates() {
        for rate in [8_000, 22_050, 32_000, 44_100, 48_000, 96_000] {
            let input = sine(rate, 1.37, 440.0);
            let out = run(rate, &input, 441);
            let expected = (input.len() as u64 * 16_000 + u64::from(rate) / 2) / u64::from(rate);
            assert_eq!(out.len() as u64, expected, "rate {rate}");
        }
    }

    #[test]
    fn a_tone_keeps_its_pitch_and_level() {
        for rate in [44_100, 48_000] {
            let out = run(rate, &sine(rate, 1.0, 1000.0), 700);
            let middle = &out[4000..12000];
            assert_eq!(peak_frequency(middle, 16_000.0), 1000.0, "rate {rate}");
            let rms = (middle.iter().map(|s| s * s).sum::<f32>() / middle.len() as f32).sqrt();
            assert!((rms - 0.3536).abs() < 0.02, "rate {rate} rms {rms}");
        }
    }

    #[test]
    fn the_start_is_not_shifted_by_the_filter_delay() {
        let mut input = vec![0.0; 44_100];
        input[22_050..22_100].fill(0.8);
        let out = run(44_100, &input, 1000);
        let first = out.iter().position(|s| s.abs() > 0.2).unwrap();
        assert!((7_950..8_050).contains(&first), "burst starts at {first}");
    }

    #[test]
    fn the_same_rate_passes_through_unchanged() {
        let input = sine(16_000, 0.5, 300.0);
        assert_eq!(run(16_000, &input, 333), input);
    }

    #[test]
    fn an_empty_stream_gives_an_empty_output() {
        assert!(run(44_100, &[], 100).is_empty());
    }

    #[test]
    fn stereo_frames_are_averaged() {
        let mut out = Vec::new();
        downmix(&[1.0, 0.0, 0.5, 0.5], 2, &mut out);
        assert_eq!(out, vec![0.5, 0.5]);
        out.clear();
        downmix(&[0.1, 0.2], 1, &mut out);
        assert_eq!(out, vec![0.1, 0.2]);
    }
}
