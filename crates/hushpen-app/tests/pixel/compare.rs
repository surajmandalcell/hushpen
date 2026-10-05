//! Baseline comparison. A pixel counts as different when any channel moves by
//! more than `CHANNEL_TOLERANCE`; an image fails when more than
//! `MAX_DIFFERENT_PIXELS` differ. Both are small on purpose: lavapipe output
//! is deterministic, so the slack only covers rounding in glyph edges.

use image::{Rgba, RgbaImage};

pub const CHANNEL_TOLERANCE: u8 = 3;
pub const MAX_DIFFERENT_PIXELS: usize = 80;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Same,
    SizeChanged {
        actual: (u32, u32),
        baseline: (u32, u32),
    },
    Different {
        pixels: usize,
        largest_step: u8,
    },
}

impl Outcome {
    pub fn passed(&self) -> bool {
        matches!(self, Outcome::Same)
    }
}

fn step(a: &Rgba<u8>, b: &Rgba<u8>) -> u8 {
    a.0.iter()
        .zip(b.0.iter())
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap_or(0)
}

pub fn compare(actual: &RgbaImage, baseline: &RgbaImage) -> Outcome {
    if actual.dimensions() != baseline.dimensions() {
        return Outcome::SizeChanged {
            actual: actual.dimensions(),
            baseline: baseline.dimensions(),
        };
    }
    let mut pixels = 0;
    let mut largest_step = 0;
    for (a, b) in actual.pixels().zip(baseline.pixels()) {
        let moved = step(a, b);
        if moved > CHANNEL_TOLERANCE {
            pixels += 1;
            largest_step = largest_step.max(moved);
        }
    }
    if pixels > MAX_DIFFERENT_PIXELS {
        Outcome::Different {
            pixels,
            largest_step,
        }
    } else {
        Outcome::Same
    }
}

/// The baseline dimmed to a quarter, with every differing pixel in red.
pub fn diff_image(actual: &RgbaImage, baseline: &RgbaImage) -> RgbaImage {
    let (width, height) = baseline.dimensions();
    RgbaImage::from_fn(width, height, |x, y| {
        let expected = baseline.get_pixel(x, y);
        match actual.get_pixel_checked(x, y) {
            Some(got) if step(got, expected) <= CHANNEL_TOLERANCE => {
                let [r, g, b, _] = expected.0;
                Rgba([r / 4, g / 4, b / 4, 255])
            }
            _ => Rgba([255, 0, 0, 255]),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba(color))
    }

    #[test]
    fn equal_images_are_the_same() {
        let image = flat(40, 40, [47, 47, 47, 255]);
        assert_eq!(compare(&image, &image.clone()), Outcome::Same);
    }

    #[test]
    fn a_step_within_the_channel_tolerance_is_ignored_everywhere() {
        let baseline = flat(40, 40, [47, 47, 47, 255]);
        let actual = flat(40, 40, [50, 44, 47, 255]);
        assert!(compare(&actual, &baseline).passed());
    }

    #[test]
    fn a_recolored_block_is_found() {
        let baseline = flat(200, 200, [47, 47, 47, 255]);
        let mut actual = baseline.clone();
        for y in 10..30 {
            for x in 10..30 {
                actual.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        assert_eq!(
            compare(&actual, &baseline),
            Outcome::Different {
                pixels: 400,
                largest_step: 208
            }
        );
    }

    #[test]
    fn a_few_stray_pixels_pass() {
        let baseline = flat(100, 100, [47, 47, 47, 255]);
        let mut actual = baseline.clone();
        for x in 0..MAX_DIFFERENT_PIXELS as u32 {
            actual.put_pixel(x, 0, Rgba([255, 255, 255, 255]));
        }
        assert!(compare(&actual, &baseline).passed());
    }

    #[test]
    fn a_size_change_fails() {
        let outcome = compare(&flat(10, 10, [0; 4]), &flat(20, 10, [0; 4]));
        assert_eq!(
            outcome,
            Outcome::SizeChanged {
                actual: (10, 10),
                baseline: (20, 10)
            }
        );
    }

    #[test]
    fn the_diff_marks_only_the_changed_pixels() {
        let baseline = flat(4, 1, [40, 40, 40, 255]);
        let mut actual = baseline.clone();
        actual.put_pixel(2, 0, Rgba([255, 255, 255, 255]));
        let diff = diff_image(&actual, &baseline);
        assert_eq!(diff.get_pixel(0, 0), &Rgba([10, 10, 10, 255]));
        assert_eq!(diff.get_pixel(2, 0), &Rgba([255, 0, 0, 255]));
    }
}
