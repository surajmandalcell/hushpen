//! Audio enhance for the microphone, at 16 kHz: a low cut, a bass and a treble shelf, and an
//! optional noise reduction, then the input boost with its soft knee last, so nothing clips.
//!
//! On the owner's own dictations, scored against a large model, a low cut at 80 Hz with the bass
//! 6 dB down gave the fewest wrong words. Louder treble, loudness normalizing, and noise
//! reduction all gave more, so [`Enhance::AUTO`] is the low cut and the bass cut alone.

use crate::capture::boost;
use nnnoiseless::DenoiseState;
use std::f64::consts::PI;
use std::sync::LazyLock;

/// The rate the filters run at: the rate of the session WAV.
const RATE: f64 = crate::resample::TARGET_RATE as f64;
/// The corner of the bass shelf.
pub const BASS_HZ: f64 = 200.0;
/// The corner of the treble shelf.
pub const TREBLE_HZ: f64 = 3_000.0;
/// The highest low cut, and the most a shelf moves, that a setting may ask for.
const MAX_LOW_CUT_HZ: u16 = 1_000;
const MAX_SHELF_DB: f32 = 24.0;

/// What the enhance does to the microphone. [`Enhance::OFF`] changes nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enhance {
    /// The corner of the low cut (a 4th-order Butterworth high-pass) in Hz; 0 is off.
    pub low_cut_hz: u16,
    /// The shelf below [`BASS_HZ`], in dB.
    pub bass_db: f32,
    /// The shelf above [`TREBLE_HZ`], in dB.
    pub treble_db: f32,
    pub noise_reduction: bool,
}

impl Enhance {
    pub const OFF: Self = Self {
        low_cut_hz: 0,
        bass_db: 0.0,
        treble_db: 0.0,
        noise_reduction: false,
    };

    pub const AUTO: Self = Self {
        low_cut_hz: 80,
        bass_db: -6.0,
        treble_db: 0.0,
        noise_reduction: false,
    };

    /// The same setting inside the ranges the filters take, with a broken number as 0.
    pub fn sanitized(self) -> Self {
        let db = |value: f32| {
            if value.is_finite() {
                // Tenths of a dB are the finest step the bits keep.
                ((value * 10.0).round() / 10.0).clamp(-MAX_SHELF_DB, MAX_SHELF_DB)
            } else {
                0.0
            }
        };
        Self {
            low_cut_hz: self.low_cut_hz.min(MAX_LOW_CUT_HZ),
            bass_db: db(self.bass_db),
            treble_db: db(self.treble_db),
            noise_reduction: self.noise_reduction,
        }
    }

    /// The setting in one word, so a running capture takes a change without a lock: the low
    /// cut in bits 0 to 15, the bass and the treble in tenths of a dB in bits 16 to 31 and 32 to
    /// 47, and the noise reduction in bit 48.
    pub fn to_bits(self) -> u64 {
        let clean = self.sanitized();
        let tenths = |db: f32| u64::from((db * 10.0).round() as i16 as u16);
        u64::from(clean.low_cut_hz)
            | tenths(clean.bass_db) << 16
            | tenths(clean.treble_db) << 32
            | u64::from(clean.noise_reduction) << 48
    }

    pub fn from_bits(bits: u64) -> Self {
        let db = |shift: u32| f32::from((bits >> shift) as u16 as i16) / 10.0;
        Self {
            low_cut_hz: bits as u16,
            bass_db: db(16),
            treble_db: db(32),
            noise_reduction: bits >> 48 & 1 == 1,
        }
        .sanitized()
    }
}

/// Runs `samples` (16 kHz mono) through `enhance` and the boost `gain` in one go, as a capture
/// does while it records: a preview of a kept recording sounds as a dictation would.
pub fn apply(samples: &[f32], enhance: Enhance, gain: f32) -> Vec<f32> {
    let mut enhancer = Enhancer::new();
    let mut out = Vec::with_capacity(samples.len());
    enhancer.process(enhance, gain, samples, &mut out);
    enhancer.finish(gain, &mut out);
    out
}

/// The enhance of one stream. The filters keep their state across calls, so audio can arrive in
/// chunks of any size, and a changed setting applies from the next chunk without a click.
pub struct Enhancer {
    config: Enhance,
    low_cut: [Biquad; 2],
    bass: Biquad,
    treble: Biquad,
    denoiser: Option<Box<Denoiser>>,
    filtered: Vec<f32>,
}

impl Default for Enhancer {
    fn default() -> Self {
        Self::new()
    }
}

impl Enhancer {
    pub fn new() -> Self {
        Self {
            config: Enhance::OFF,
            low_cut: [Biquad::IDENTITY; 2],
            bass: Biquad::IDENTITY,
            treble: Biquad::IDENTITY,
            denoiser: None,
            filtered: Vec::new(),
        }
    }

    /// Appends `input` with `enhance` and then `gain` applied to `out`. With noise reduction on,
    /// the last few milliseconds stay inside until a later call or [`Enhancer::finish`], so the
    /// output in total is exactly as long as the input and keeps its timing.
    pub fn process(&mut self, enhance: Enhance, gain: f32, input: &[f32], out: &mut Vec<f32>) {
        self.configure(enhance.sanitized(), gain, out);
        let start = out.len();
        let mut filtered = std::mem::take(&mut self.filtered);
        filtered.clear();
        filtered.extend(input.iter().map(|sample| self.filter(*sample)));
        match self.denoiser.as_mut() {
            Some(denoiser) => denoiser.process(&filtered, out),
            None => out.extend_from_slice(&filtered),
        }
        self.filtered = filtered;
        boost(&mut out[start..], gain);
    }

    /// Appends what the noise reduction still holds, at the end of a stream. A later call to
    /// [`Enhancer::process`] starts a new stream.
    pub fn finish(&mut self, gain: f32, out: &mut Vec<f32>) {
        if let Some(mut denoiser) = self.denoiser.take() {
            let start = out.len();
            denoiser.finish(out);
            boost(&mut out[start..], gain);
        }
        self.config.noise_reduction = false;
    }

    fn configure(&mut self, config: Enhance, gain: f32, out: &mut Vec<f32>) {
        if config == self.config {
            return;
        }
        if config.low_cut_hz != self.config.low_cut_hz && config.low_cut_hz > 0 {
            let corner = f64::from(config.low_cut_hz);
            // The two sections of a 4th-order Butterworth: -3 dB at the corner, 24 dB an octave.
            self.low_cut[0].set(Biquad::high_pass(corner, 0.541_196_100_146_197));
            self.low_cut[1].set(Biquad::high_pass(corner, 1.306_562_964_876_376_5));
        }
        if config.bass_db != self.config.bass_db {
            self.bass.set(Biquad::shelf(
                Shelf::Low,
                BASS_HZ,
                f64::from(config.bass_db),
            ));
        }
        if config.treble_db != self.config.treble_db {
            self.treble.set(Biquad::shelf(
                Shelf::High,
                TREBLE_HZ,
                f64::from(config.treble_db),
            ));
        }
        match (config.noise_reduction, self.denoiser.is_some()) {
            (true, false) => self.denoiser = Some(Box::new(Denoiser::new())),
            (false, true) => self.finish(gain, out),
            _ => {}
        }
        self.config = config;
    }

    fn filter(&mut self, sample: f32) -> f32 {
        // A broken sample would stay in the filter state for good.
        let mut x = if sample.is_finite() {
            f64::from(sample)
        } else {
            0.0
        };
        if self.config.low_cut_hz > 0 {
            x = self.low_cut[0].run(x);
            x = self.low_cut[1].run(x);
        }
        if self.config.bass_db != 0.0 {
            x = self.bass.run(x);
        }
        if self.config.treble_db != 0.0 {
            x = self.treble.run(x);
        }
        x as f32
    }
}

enum Shelf {
    Low,
    High,
}

/// One second-order section in the transposed direct form II, from the RBJ audio EQ cookbook.
/// The state runs in f64, so a low corner at 16 kHz stays exact.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
        z1: 0.0,
        z2: 0.0,
    };

    fn normalized(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn high_pass(corner: f64, q: f64) -> Self {
        let w0 = 2.0 * PI * corner / RATE;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        Self::normalized(
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    /// A shelf with a slope of 1: the gain is half way at the corner.
    fn shelf(kind: Shelf, corner: f64, db: f64) -> Self {
        let a = 10f64.powf(db / 40.0);
        let w0 = 2.0 * PI * corner / RATE;
        let (sin, cos) = w0.sin_cos();
        let root = 2.0 * a.sqrt() * sin / 2.0 * std::f64::consts::SQRT_2;
        let (p, m) = (a + 1.0, a - 1.0);
        match kind {
            Shelf::Low => Self::normalized(
                [
                    a * (p - m * cos + root),
                    2.0 * a * (m - p * cos),
                    a * (p - m * cos - root),
                ],
                [p + m * cos + root, -2.0 * (m + p * cos), p + m * cos - root],
            ),
            Shelf::High => Self::normalized(
                [
                    a * (p + m * cos + root),
                    -2.0 * a * (m + p * cos),
                    a * (p + m * cos - root),
                ],
                [p - m * cos + root, 2.0 * (m - p * cos), p - m * cos - root],
            ),
        }
    }

    /// Takes the coefficients of `other` and keeps the state, so a change does not click.
    fn set(&mut self, other: Self) {
        *self = Self {
            z1: self.z1,
            z2: self.z2,
            ..other
        };
    }

    fn run(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        // A state that decays toward zero in silence would reach the slow subnormal numbers.
        if self.z1.abs() < 1e-30 {
            self.z1 = 0.0;
        }
        if self.z2.abs() < 1e-30 {
            self.z2 = 0.0;
        }
        y
    }
}

/// The noise reduction model runs at 48 kHz.
const FACTOR: usize = 3;
/// Taps of the low-pass that takes 16 kHz up to 48 kHz and back down.
const TAPS: usize = 121;
/// The delay of each low-pass at 48 kHz.
const FIR_DELAY: usize = (TAPS - 1) / 2;
/// The model works in frames of 10 ms and gives each frame back one frame later.
const FRAME: usize = DenoiseState::FRAME_SIZE;
/// The delay of the noise reduction at 16 kHz: the two low-passes and one frame. It is a whole
/// number of 16 kHz samples, so the output can be moved back into place exactly.
pub const DENOISE_DELAY: usize = (2 * FIR_DELAY + FRAME) / FACTOR;
const _: () = assert!((2 * FIR_DELAY + FRAME).is_multiple_of(FACTOR));

/// A Blackman-windowed sinc low-pass at 48 kHz with its corner at 7 kHz, below the 8 kHz limit
/// of 16 kHz audio, summing to 1.
static LOW_PASS: LazyLock<[f64; TAPS]> = LazyLock::new(|| {
    let cut = 7_000.0 / (RATE * FACTOR as f64);
    let last = (TAPS - 1) as f64;
    let mut taps = [0.0; TAPS];
    for (index, tap) in taps.iter_mut().enumerate() {
        let n = index as f64;
        let x = n - FIR_DELAY as f64;
        let sinc = if x == 0.0 {
            2.0 * cut
        } else {
            (2.0 * PI * cut * x).sin() / (PI * x)
        };
        let window = 0.42 - 0.5 * (2.0 * PI * n / last).cos() + 0.08 * (4.0 * PI * n / last).cos();
        *tap = sinc * window;
    }
    let sum: f64 = taps.iter().sum();
    taps.iter_mut().for_each(|tap| *tap /= sum);
    taps
});

/// The last `N` samples of a stream, oldest first, without moving them on every push.
struct History<const N: usize> {
    /// Each sample is kept twice, `N` apart, so the window is always one slice.
    data: Vec<f64>,
    next: usize,
}

impl<const N: usize> History<N> {
    fn new() -> Self {
        Self {
            data: vec![0.0; 2 * N],
            next: 0,
        }
    }

    fn push(&mut self, sample: f64) {
        self.data[self.next] = sample;
        self.data[self.next + N] = sample;
        self.next = (self.next + 1) % N;
    }

    fn window(&self) -> &[f64] {
        &self.data[self.next..self.next + N]
    }
}

/// RNNoise at 48 kHz for a 16 kHz stream: up by three through the low-pass, the model frame by
/// frame, and down by three through the low-pass again. The first [`DENOISE_DELAY`] outputs come
/// from before the stream and are dropped, and [`Denoiser::finish`] pushes the rest out.
struct Denoiser {
    model: Box<DenoiseState<'static>>,
    up: History<{ TAPS.div_ceil(FACTOR) }>,
    down: History<TAPS>,
    frame: Vec<f32>,
    frame_out: Vec<f32>,
    /// The 48 kHz samples that went down so far, which sets the phase of the next output.
    phase: usize,
    skip: usize,
    taken: u64,
    given: u64,
}

impl Denoiser {
    fn new() -> Self {
        Self {
            model: DenoiseState::new(),
            up: History::new(),
            down: History::new(),
            frame: Vec::with_capacity(FRAME),
            frame_out: vec![0.0; FRAME],
            phase: 0,
            skip: DENOISE_DELAY,
            taken: 0,
            given: 0,
        }
    }

    fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.taken += input.len() as u64;
        for sample in input {
            self.push(f64::from(*sample), out);
        }
    }

    fn finish(&mut self, out: &mut Vec<f32>) {
        let start = out.len() as u64;
        while self.given < self.taken {
            self.push(0.0, out);
        }
        let excess = (self.given - self.taken) as usize;
        let keep = (out.len() - excess).max(start as usize);
        out.truncate(keep);
        self.given = self.taken;
    }

    fn push(&mut self, sample: f64, out: &mut Vec<f32>) {
        self.up.push(sample);
        let taps = &*LOW_PASS;
        let recent = self.up.window();
        let newest = recent.len() - 1;
        for phase in 0..FACTOR {
            let value: f64 = taps[phase..]
                .iter()
                .step_by(FACTOR)
                .enumerate()
                .map(|(back, tap)| tap * recent[newest - back])
                .sum();
            // The model reads 16-bit sample values; zero stuffing lost two thirds of the level.
            self.frame
                .push((value * FACTOR as f64 * f64::from(i16::MAX)) as f32);
        }
        if self.frame.len() < FRAME {
            return;
        }
        self.model.process_frame(&mut self.frame_out, &self.frame);
        self.frame.clear();
        for index in 0..FRAME {
            self.down
                .push(f64::from(self.frame_out[index]) / f64::from(i16::MAX));
            let due = self.phase == 0;
            self.phase = (self.phase + 1) % FACTOR;
            if !due {
                continue;
            }
            let value: f64 = taps
                .iter()
                .zip(self.down.window())
                .map(|(tap, sample)| tap * sample)
                .sum();
            if self.skip > 0 {
                self.skip -= 1;
            } else {
                out.push(value as f32);
                self.given += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, seconds: f64, level: f64) -> Vec<f32> {
        (0..(RATE * seconds) as usize)
            .map(|i| (level * (2.0 * PI * hz * i as f64 / RATE).sin()) as f32)
            .collect()
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    }

    /// The gain in dB of `enhance` for a steady tone, measured after the filters settle.
    fn gain_db(enhance: Enhance, hz: f64) -> f64 {
        let input = sine(hz, 1.0, 0.1);
        let out = apply(&input, enhance, 1.0);
        let settled = input.len() / 2;
        20.0 * (rms(&out[settled..]) / rms(&input[settled..])).log10()
    }

    fn near(actual: f64, expected: f64, within: f64) -> bool {
        (actual - expected).abs() <= within
    }

    #[test]
    fn the_low_cut_is_3_db_down_at_its_corner_and_steep_below() {
        let cut = Enhance {
            low_cut_hz: 80,
            ..Enhance::OFF
        };
        let at = gain_db(cut, 80.0);
        assert!(near(at, -3.0, 0.3), "80 Hz: {at}");
        let octave = gain_db(cut, 40.0);
        assert!(near(octave, -24.1, 1.0), "40 Hz: {octave}");
        let speech = gain_db(cut, 1_000.0);
        assert!(near(speech, 0.0, 0.05), "1 kHz: {speech}");
    }

    #[test]
    fn the_bass_shelf_moves_the_lows_and_leaves_the_voice() {
        let bass = Enhance {
            bass_db: -6.0,
            ..Enhance::OFF
        };
        let low = gain_db(bass, 30.0);
        assert!(near(low, -6.0, 0.5), "30 Hz: {low}");
        let corner = gain_db(bass, BASS_HZ);
        assert!(near(corner, -3.0, 0.3), "200 Hz: {corner}");
        let voice = gain_db(bass, 2_500.0);
        assert!(near(voice, 0.0, 0.2), "2.5 kHz: {voice}");
        let raised = gain_db(
            Enhance {
                bass_db: 3.0,
                ..Enhance::OFF
            },
            30.0,
        );
        assert!(near(raised, 3.0, 0.4), "+3 dB at 30 Hz: {raised}");
    }

    #[test]
    fn the_treble_shelf_moves_the_highs_and_leaves_the_lows() {
        let treble = Enhance {
            treble_db: 6.0,
            ..Enhance::OFF
        };
        let high = gain_db(treble, 7_500.0);
        assert!(near(high, 6.0, 0.6), "7.5 kHz: {high}");
        let corner = gain_db(treble, TREBLE_HZ);
        assert!(near(corner, 3.0, 0.5), "3 kHz: {corner}");
        let low = gain_db(treble, 200.0);
        assert!(near(low, 0.0, 0.1), "200 Hz: {low}");
    }

    #[test]
    fn auto_is_the_measured_low_cut_and_bass_cut_without_noise_reduction() {
        assert_eq!(
            Enhance::AUTO,
            Enhance {
                low_cut_hz: 80,
                bass_db: -6.0,
                treble_db: 0.0,
                noise_reduction: false,
            }
        );
        let rumble = gain_db(Enhance::AUTO, 50.0);
        assert!(rumble < -14.0, "50 Hz: {rumble}");
        let voice = gain_db(Enhance::AUTO, 1_500.0);
        assert!(near(voice, 0.0, 0.2), "1.5 kHz: {voice}");
    }

    #[test]
    fn off_at_full_level_changes_no_sample() {
        let input: Vec<f32> = (0..4_000).map(|i| ((i % 97) as f32 / 48.0) - 1.0).collect();
        assert_eq!(apply(&input, Enhance::OFF, 1.0), input);
    }

    #[test]
    fn the_boost_comes_last_and_nothing_reaches_full_scale() {
        let loud = sine(4_000.0, 0.5, 0.9);
        let out = apply(
            &loud,
            Enhance {
                treble_db: 12.0,
                ..Enhance::OFF
            },
            2.0,
        );
        assert!(out.iter().all(|s| s.abs() <= 1.0), "the knee bends it");
        assert!(out.iter().any(|s| s.abs() > 0.95));
    }

    #[test]
    fn broken_and_tiny_input_never_breaks_the_filters() {
        let mut enhancer = Enhancer::new();
        let mut out = Vec::new();
        let mut input = vec![1e-35_f32; 16_000];
        input[10] = f32::NAN;
        input[20] = f32::INFINITY;
        input.extend(vec![0.0; 32_000]);
        enhancer.process(Enhance::AUTO, 1.5, &input, &mut out);
        assert_eq!(out.len(), input.len());
        assert!(out.iter().all(|s| s.is_finite()));
        assert!(
            out[40_000..].iter().all(|s| *s == 0.0),
            "silence settles at 0"
        );
        let states = enhancer
            .low_cut
            .iter()
            .chain([&enhancer.bass])
            .all(|q| q.z1 == 0.0 && q.z2 == 0.0);
        assert!(states, "no subnormal state is left");
        let after = sine(1_000.0, 0.2, 0.1);
        let mut more = Vec::new();
        enhancer.process(Enhance::AUTO, 1.0, &after, &mut more);
        assert!(rms(&more[1_000..]) > 0.05, "a tone still gets through");
    }

    #[test]
    fn a_change_applies_from_the_next_chunk() {
        let tone = sine(7_500.0, 0.5, 0.1);
        let mut enhancer = Enhancer::new();
        let mut first = Vec::new();
        enhancer.process(Enhance::OFF, 1.0, &tone, &mut first);
        let mut second = Vec::new();
        let treble = Enhance {
            treble_db: 6.0,
            ..Enhance::OFF
        };
        enhancer.process(treble, 1.0, &tone, &mut second);
        assert!(near(rms(&first) / rms(&tone), 1.0, 1e-6));
        let raised = 20.0 * (rms(&second[2_000..]) / rms(&tone[2_000..])).log10();
        assert!(near(raised, 6.0, 0.6), "{raised}");
    }

    #[test]
    fn a_setting_survives_its_bits() {
        let setting = Enhance {
            low_cut_hz: 120,
            bass_db: -9.0,
            treble_db: 3.5,
            noise_reduction: true,
        };
        assert_eq!(Enhance::from_bits(setting.to_bits()), setting);
        assert_eq!(Enhance::from_bits(Enhance::AUTO.to_bits()), Enhance::AUTO);
        assert_eq!(Enhance::from_bits(0), Enhance::OFF);
        let wild = Enhance {
            low_cut_hz: 9_000,
            bass_db: f32::NAN,
            treble_db: 99.0,
            noise_reduction: false,
        };
        assert_eq!(
            Enhance::from_bits(wild.to_bits()),
            Enhance {
                low_cut_hz: MAX_LOW_CUT_HZ,
                bass_db: 0.0,
                treble_db: MAX_SHELF_DB,
                noise_reduction: false,
            }
        );
    }

    /// A voice-like test signal: pulses at a wandering pitch near 140 Hz, shaped, rising and
    /// falling four times a second. The pitch wanders so no shift of one period matches it.
    fn voiced(seconds: f64) -> Vec<f32> {
        let mut out = Vec::new();
        let mut formants = [
            Biquad::shelf(Shelf::High, 700.0, 6.0),
            Biquad::high_pass(120.0, 0.7),
        ];
        let (mut seed, mut next) = (7_u32, 0);
        for i in 0..(RATE * seconds) as usize {
            let t = i as f64 / RATE;
            let pulse = if i == next {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                next = i + 100 + (seed >> 16) as usize % 31;
                1.0
            } else {
                0.0
            };
            let envelope = 0.5 - 0.5 * (2.0 * PI * 4.0 * t).cos();
            let shaped = formants.iter_mut().fold(pulse, |x, f| f.run(x));
            out.push((0.6 * envelope * shaped) as f32);
        }
        out
    }

    #[test]
    fn noise_reduction_keeps_the_length_whatever_the_chunks() {
        let input = voiced(1.3);
        for chunk in [1, 160, 333, 1_024, input.len()] {
            let mut enhancer = Enhancer::new();
            let mut out = Vec::new();
            let on = Enhance {
                noise_reduction: true,
                ..Enhance::OFF
            };
            for piece in input.chunks(chunk) {
                enhancer.process(on, 1.0, piece, &mut out);
            }
            enhancer.finish(1.0, &mut out);
            assert_eq!(out.len(), input.len(), "chunks of {chunk}");
            assert!(out.iter().all(|s| s.is_finite()));
        }
    }

    #[test]
    fn noise_reduction_keeps_the_voice_in_place() {
        let input = voiced(2.0);
        let out = apply(
            &input,
            Enhance {
                noise_reduction: true,
                ..Enhance::OFF
            },
            1.0,
        );
        let lag_score = |lag: isize| -> f64 {
            (4_000..28_000)
                .map(|i| f64::from(input[i]) * f64::from(out[(i as isize + lag) as usize]))
                .sum()
        };
        let best = (-400..=400).max_by(|a, b| lag_score(*a).total_cmp(&lag_score(*b)));
        assert_eq!(best, Some(0), "the voice is where it was");
        assert!(rms(&out[4_000..28_000]) > 0.3 * rms(&input[4_000..28_000]));
    }

    #[test]
    fn turning_noise_reduction_off_mid_stream_loses_nothing() {
        let input = voiced(1.0);
        let mut enhancer = Enhancer::new();
        let mut out = Vec::new();
        let on = Enhance {
            noise_reduction: true,
            ..Enhance::OFF
        };
        enhancer.process(on, 1.0, &input[..8_000], &mut out);
        enhancer.process(Enhance::OFF, 1.0, &input[8_000..], &mut out);
        enhancer.finish(1.0, &mut out);
        assert_eq!(out.len(), input.len());
        assert_eq!(
            out[8_000..],
            input[8_000..],
            "after it, audio passes as it was"
        );
    }
}
