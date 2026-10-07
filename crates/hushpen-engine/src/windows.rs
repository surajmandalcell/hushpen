//! Long audio is decoded in overlapping windows.
//!
//! whisper computes the log-mel spectrogram of the whole input before its first abort check,
//! so one call over 10 minutes cannot be cancelled in under a second on a slow CPU. Windows
//! of 120 s with 10 s of overlap keep that unabortable step short and give the engine a
//! point to look at the cancel flag between windows.

use crate::asr::Segment;

pub const SAMPLE_RATE: usize = 16_000;
pub const WINDOW: usize = 120 * SAMPLE_RATE;
pub const OVERLAP: usize = 10 * SAMPLE_RATE;

/// One slice of the input, in samples, and the span of it whose segments are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
    /// Segments whose midpoint (in samples from the start of the input) falls in
    /// `keep_from..keep_to` belong to this window.
    pub keep_from: usize,
    pub keep_to: usize,
}

/// Split `len` samples into windows. Neighbors overlap by [`OVERLAP`] and hand over at the
/// middle of the overlap, so every instant belongs to exactly one window.
pub fn plan(len: usize) -> Vec<Window> {
    let mut windows = Vec::new();
    let mut start = 0;
    loop {
        let end = (start + WINDOW).min(len);
        let last = end == len;
        windows.push(Window {
            start,
            end,
            keep_from: if start == 0 { 0 } else { start + OVERLAP / 2 },
            keep_to: if last { usize::MAX } else { end - OVERLAP / 2 },
        });
        if last {
            return windows;
        }
        start = end - OVERLAP;
    }
}

/// Move the window-relative segments to input time and keep the ones this window owns.
pub fn own_segments(window: &Window, segments: Vec<Segment>) -> Vec<Segment> {
    let offset_ms = (window.start * 1000 / SAMPLE_RATE) as u64;
    segments
        .into_iter()
        .map(|s| Segment {
            start_ms: s.start_ms + offset_ms,
            end_ms: s.end_ms + offset_ms,
            text: s.text,
        })
        .filter(|s| {
            let mid_samples = ((s.start_ms + s.end_ms) / 2) as usize * SAMPLE_RATE / 1000;
            (window.keep_from..window.keep_to).contains(&mid_samples)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start_ms: u64, end_ms: u64, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    #[test]
    fn short_input_is_one_window() {
        let windows = plan(WINDOW);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start, 0);
        assert_eq!(windows[0].end, WINDOW);
        assert_eq!(windows[0].keep_from, 0);
        assert_eq!(windows[0].keep_to, usize::MAX);
    }

    #[test]
    fn long_input_overlaps_by_ten_seconds() {
        let windows = plan(600 * SAMPLE_RATE);
        assert_eq!(windows.len(), 6);
        for pair in windows.windows(2) {
            assert_eq!(pair[0].end - pair[1].start, OVERLAP);
        }
        assert_eq!(windows.last().map(|w| w.end), Some(600 * SAMPLE_RATE));
    }

    #[test]
    fn every_instant_has_one_owner() {
        let windows = plan(300 * SAMPLE_RATE + 123);
        assert_eq!(windows[0].keep_from, 0);
        for pair in windows.windows(2) {
            assert_eq!(pair[0].keep_to, pair[1].keep_from);
        }
        assert_eq!(windows.last().map(|w| w.keep_to), Some(usize::MAX));
    }

    #[test]
    fn the_last_window_can_be_short() {
        let len = WINDOW + 20 * SAMPLE_RATE;
        let windows = plan(len);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].start, WINDOW - OVERLAP);
        assert_eq!(windows[1].end, len);
    }

    #[test]
    fn segments_move_to_input_time_and_the_overlap_is_split_once() {
        let windows = plan(300 * SAMPLE_RATE);
        let second = windows[1];
        // The second window starts at 110 s and owns segments from 115 s.
        let kept = own_segments(
            &second,
            vec![
                seg(0, 2_000, "in the overlap, first half"),
                seg(6_000, 8_000, "in the overlap, second half"),
                seg(30_000, 32_000, "later"),
            ],
        );
        let texts: Vec<_> = kept.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["in the overlap, second half", "later"]);
        assert_eq!(kept[0].start_ms, 116_000);
        assert_eq!(kept[1].end_ms, 142_000);
    }
}
