//! The audio of each piece on its way out: the silence the model leaves around the voice goes,
//! short fades keep the cuts from clicking, the pause after the piece follows, and a speed
//! other than 1 changes the time but not the pitch. The decoder can overshoot full scale a
//! little, so samples are clipped to it.

/// 10 ms at 24 kHz: the frame the loudness is measured in.
const FRAME: usize = 240;
/// 5 ms fades at both ends.
const FADE: usize = 120;
/// What stays of the quiet before the voice and after it, so no sound is cut short.
const KEEP_BEFORE: usize = FRAME;
const KEEP_AFTER: usize = 4 * FRAME;
/// A frame this much quieter than the loudest one is silence.
const QUIET: f32 = 0.03;
/// A frame below this level is silence, however quiet the voice is.
const FLOOR: f32 = 0.002;

/// The voice of one piece, trimmed and faded, stretched to `speed`, then the pause after it.
pub fn finish(samples: &[f32], rate: u32, pause_ms: u32, speed: f32) -> Vec<f32> {
    let voiced = trim(samples);
    let mut out = if (speed - 1.0).abs() < 0.01 {
        voiced.to_vec()
    } else {
        stretch(voiced, speed)
    };
    for sample in &mut out {
        *sample = sample.clamp(-1.0, 1.0);
    }
    fade(&mut out);
    let pause = f64::from(rate) * f64::from(pause_ms) / 1000.0 / f64::from(speed.max(0.1));
    out.resize(out.len() + pause as usize, 0.0);
    out
}

/// `samples` without the silence before the first voiced frame and after the last one.
pub fn trim(samples: &[f32]) -> &[f32] {
    let levels: Vec<f32> = samples
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
    let start = (first * FRAME).saturating_sub(KEEP_BEFORE);
    let end = ((last + 1) * FRAME + KEEP_AFTER).min(samples.len());
    &samples[start..end]
}

fn fade(samples: &mut [f32]) {
    let len = samples.len();
    let fade = FADE.min(len / 2);
    for index in 0..fade {
        let gain = index as f32 / fade as f32;
        samples[index] *= gain;
        samples[len - 1 - index] *= gain;
    }
}

/// 20 ms frames, half overlapped in the output.
const WINDOW: usize = 480;
const HOP: usize = WINDOW / 2;
/// How far from its place by the speed a frame may come, to match the frame before it.
const SEEK: usize = 120;

/// Plays `input` `speed` times as fast at the same pitch (WSOLA). Each output frame is taken
/// from near where the speed puts it, at the offset where it best continues the frame before.
pub fn stretch(input: &[f32], speed: f32) -> Vec<f32> {
    if input.len() < 2 * WINDOW + 2 * SEEK || !speed.is_finite() || speed <= 0.0 {
        return input.to_vec();
    }
    let window: Vec<f32> = (0..WINDOW)
        .map(|index| {
            let phase = std::f32::consts::TAU * index as f32 / WINDOW as f32;
            0.5 - 0.5 * phase.cos()
        })
        .collect();
    let length = (input.len() as f64 / f64::from(speed)) as usize;
    let mut out = vec![0.0_f32; length + WINDOW];
    let mut previous = 0_usize;
    let mut frame = 0_usize;
    loop {
        let at = frame * HOP;
        if at + WINDOW > out.len() {
            break;
        }
        let nominal = (at as f64 * f64::from(speed)) as usize;
        let start = if frame == 0 {
            0
        } else {
            best_start(input, previous + HOP, nominal)
        };
        if start + WINDOW > input.len() {
            break;
        }
        for (index, weight) in window.iter().enumerate() {
            out[at + index] += input[start + index] * weight;
        }
        previous = start;
        frame += 1;
    }
    out.truncate(length.min(frame * HOP + HOP));
    out
}

/// The start near `nominal` whose first half matches the samples at `natural` best: those are
/// what would follow the frame before if the time did not change.
fn best_start(input: &[f32], natural: usize, nominal: usize) -> usize {
    let low = nominal.saturating_sub(SEEK);
    let high = (nominal + SEEK).min(input.len().saturating_sub(WINDOW));
    if natural + HOP > input.len() || low > high {
        return nominal.min(high);
    }
    let target = &input[natural..natural + HOP];
    (low..=high)
        .max_by(|&a, &b| {
            let score = |start: usize| -> f32 {
                input[start..start + HOP]
                    .iter()
                    .zip(target)
                    .map(|(x, y)| x * y)
                    .sum()
            };
            score(a).total_cmp(&score(b))
        })
        .unwrap_or(nominal)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 24_000;

    fn tone(hz: f32, seconds: f32) -> Vec<f32> {
        (0..(RATE as f32 * seconds) as usize)
            .map(|index| {
                let t = index as f32 / RATE as f32;
                0.5 * (std::f32::consts::TAU * hz * t).sin()
            })
            .collect()
    }

    fn crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
            .count()
    }

    #[test]
    fn silence_around_the_voice_goes_but_the_edges_of_the_voice_stay() {
        let mut samples = vec![0.0005; 24_000];
        samples.extend(tone(220.0, 0.5));
        samples.extend(vec![0.0; 12_000]);
        let trimmed = trim(&samples);
        let expected = 12_000 + KEEP_BEFORE + KEEP_AFTER;
        assert!(
            trimmed.len().abs_diff(expected) <= FRAME,
            "{}",
            trimmed.len()
        );
        assert!(trim(&vec![0.0; 5_000]).is_empty());
    }

    #[test]
    fn a_piece_ends_in_its_pause_and_starts_and_ends_faded() {
        let out = finish(&tone(220.0, 0.5), RATE, 300, 1.0);
        assert!(out.len().abs_diff(12_000 + 7_200) <= KEEP_AFTER);
        assert_eq!(out[0], 0.0);
        assert!(out[out.len() - 7_200..].iter().all(|s| *s == 0.0));
        assert!(finish(&[], RATE, 100, 1.0).len() == 2_400);
    }

    #[test]
    fn a_stretch_changes_the_length_and_keeps_the_pitch() {
        let input = tone(200.0, 1.0);
        for speed in [0.5_f32, 0.8, 1.25, 2.0] {
            let out = stretch(&input, speed);
            let wanted = input.len() as f32 / speed;
            assert!(
                (out.len() as f32 - wanted).abs() < WINDOW as f32,
                "{speed}: {}",
                out.len()
            );
            // The same pitch: as many zero crossings for each second of output.
            let rate_in = crossings(&input) as f32 / input.len() as f32;
            let rate_out = crossings(&out[WINDOW..out.len() - WINDOW]) as f32
                / (out.len() - 2 * WINDOW) as f32;
            assert!(
                (rate_out / rate_in - 1.0).abs() < 0.03,
                "{speed}: {rate_out} {rate_in}"
            );
        }
        assert_eq!(stretch(&[0.1; 100], 2.0), vec![0.1; 100]);
    }
}
