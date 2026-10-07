//! Level meter values for the waveform and the Home meter.

/// Quietest level the meter shows. Anything below reads as rest.
const FLOOR_DB: f32 = -60.0;

/// How many meter values per second.
pub const LEVELS_PER_SECOND: u32 = 20;

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    (sum / samples.len() as f64).sqrt() as f32
}

/// Maps an RMS value (0.0 to 1.0 of full scale) to a meter value from 0.0 to
/// 1.0 on a decibel scale, so speech moves the meter and room noise does not.
pub fn level_from_rms(rms: f32) -> f32 {
    if rms <= 0.0 {
        return 0.0;
    }
    ((20.0 * rms.log10() - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// Cuts a sample stream into windows of `1 / LEVELS_PER_SECOND` seconds and
/// reports one meter value for each full window.
pub struct LevelMeter {
    window: usize,
    sum_squares: f64,
    count: usize,
}

impl LevelMeter {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            window: (sample_rate / LEVELS_PER_SECOND).max(1) as usize,
            sum_squares: 0.0,
            count: 0,
        }
    }

    pub fn push(&mut self, samples: &[f32], mut emit: impl FnMut(f32)) {
        for sample in samples {
            self.sum_squares += f64::from(*sample) * f64::from(*sample);
            self.count += 1;
            if self.count == self.window {
                let rms = (self.sum_squares / self.count as f64).sqrt() as f32;
                emit(level_from_rms(rms));
                self.sum_squares = 0.0;
                self.count = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_at_rest() {
        assert_eq!(level_from_rms(rms(&[0.0; 800])), 0.0);
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn a_full_scale_square_wave_is_at_the_top() {
        assert_eq!(level_from_rms(rms(&[1.0, -1.0, 1.0, -1.0])), 1.0);
    }

    #[test]
    fn levels_follow_decibels() {
        // -30 dBFS is half way up a 60 dB scale.
        let level = level_from_rms(10f32.powf(-30.0 / 20.0));
        assert!((level - 0.5).abs() < 1e-4, "{level}");
        assert_eq!(level_from_rms(10f32.powf(-80.0 / 20.0)), 0.0);
    }

    #[test]
    fn the_meter_emits_one_value_per_window_across_pushes() {
        let mut meter = LevelMeter::new(16_000);
        let mut values = Vec::new();
        meter.push(&[0.5; 500], |v| values.push(v));
        assert!(values.is_empty());
        meter.push(&[0.5; 1200], |v| values.push(v));
        assert_eq!(values.len(), 2, "1700 samples hold two 800-sample windows");
        assert!(values.iter().all(|v| *v > 0.5));
    }
}
