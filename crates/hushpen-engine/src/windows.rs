//! Audio is transcribed in overlapping windows while it is read.
//!
//! whisper computes the log-mel spectrogram of the whole input before its first abort check,
//! so one call over 10 minutes cannot be cancelled in under a second on a slow CPU. Windows
//! of 120 s with 10 s of overlap keep that unabortable step short and give the engine a
//! point to look at the cancel flag between windows. The input is cut while it is read, so an
//! import of any length holds about one window of samples, 8 MB.

use crate::asr::{CancelFlag, EngineError, PcmSource, Segment};

pub const SAMPLE_RATE: usize = 16_000;
pub const WINDOW: usize = 120 * SAMPLE_RATE;
pub const OVERLAP: usize = 10 * SAMPLE_RATE;

/// Silence added after the audio. whisper can drop the last word of a clip that stops on
/// speech, and a recording that ends the moment the user stops talking does exactly that.
pub const TAIL_PAD: usize = SAMPLE_RATE * 3 / 10;

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

/// Cuts a [`PcmSource`] into windows while it is read, with [`TAIL_PAD`] of silence after its
/// end. Neighbors overlap by [`OVERLAP`] and hand over at the middle of the overlap, so every
/// instant belongs to exactly one window. Only the current window and one read are held.
pub struct Windows<'a> {
    source: &'a mut dyn PcmSource,
    /// The input from `start` on.
    buffer: Vec<f32>,
    start: usize,
    /// Where the next window starts, once the current one is handed out.
    next_start: Option<usize>,
    audio_len: usize,
    ended: bool,
}

impl<'a> Windows<'a> {
    pub fn new(source: &'a mut dyn PcmSource) -> Self {
        Self {
            source,
            buffer: Vec::with_capacity(WINDOW + OVERLAP),
            start: 0,
            next_start: Some(0),
            audio_len: 0,
            ended: false,
        }
    }

    /// The next window and its samples, or `None` after the last one. `cancel` is checked
    /// between reads. A source with no samples at all is [`EngineError::NoAudio`].
    pub fn next(&mut self, cancel: &CancelFlag) -> Result<Option<(Window, &[f32])>, EngineError> {
        let Some(start) = self.next_start.take() else {
            return Ok(None);
        };
        self.buffer.drain(..start - self.start);
        self.start = start;
        // One sample past the window tells a full window from the last one.
        while !self.ended && self.buffer.len() <= WINDOW {
            if cancel.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            let before = self.buffer.len();
            self.ended = !self.source.read(&mut self.buffer)?;
            self.audio_len += self.buffer.len() - before;
            if self.ended {
                if self.audio_len == 0 {
                    return Err(EngineError::NoAudio);
                }
                self.buffer.resize(self.buffer.len() + TAIL_PAD, 0.0);
            }
        }
        let len = self.buffer.len().min(WINDOW);
        let end = start + len;
        let last = self.ended && self.buffer.len() <= WINDOW;
        if !last {
            self.next_start = Some(end - OVERLAP);
        }
        let window = Window {
            start,
            end,
            keep_from: if start == 0 { 0 } else { start + OVERLAP / 2 },
            keep_to: if last { usize::MAX } else { end - OVERLAP / 2 },
        };
        Ok(Some((window, &self.buffer[..len])))
    }

    /// Samples of audio read so far, without the pad.
    pub fn audio_len(&self) -> usize {
        self.audio_len
    }

    pub fn fraction_read(&self) -> f32 {
        self.source.fraction_read()
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

/// Takes the pad out of the result again: a segment that starts inside it is dropped (whisper
/// can invent words for silence) and one that runs into it ends where the audio ends.
pub fn clip_to_audio(segments: Vec<Segment>, audio_samples: usize) -> Vec<Segment> {
    let audio_ms = (audio_samples * 1000 / SAMPLE_RATE) as u64;
    segments
        .into_iter()
        .filter(|s| s.start_ms < audio_ms)
        .map(|s| Segment {
            end_ms: s.end_ms.min(audio_ms),
            ..s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::SliceSource;

    fn seg(start_ms: u64, end_ms: u64, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    /// The sample at input index `index`. Never zero, so the pad shows, and exact in `f32`.
    fn value(index: usize) -> f32 {
        (index % 10_007 + 1) as f32
    }

    /// `len` samples of [`value`], given in reads whose sizes cycle through `reads`.
    struct Ramp {
        len: usize,
        given: usize,
        reads: Vec<usize>,
        calls: usize,
    }

    impl Ramp {
        fn new(len: usize, reads: &[usize]) -> Self {
            Self {
                len,
                given: 0,
                reads: reads.to_vec(),
                calls: 0,
            }
        }
    }

    impl PcmSource for Ramp {
        fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, EngineError> {
            let size = self.reads[self.calls % self.reads.len()];
            self.calls += 1;
            let end = (self.given + size).min(self.len);
            out.extend((self.given..end).map(value));
            self.given = end;
            Ok(self.given < self.len)
        }

        fn fraction_read(&self) -> f32 {
            self.given as f32 / self.len as f32
        }
    }

    /// Every window of `source`, after checking that each one holds exactly the input at its
    /// span followed by silence.
    fn cut(source: &mut dyn PcmSource, audio_len: usize) -> Vec<Window> {
        let mut windows = Windows::new(source);
        let mut all = Vec::new();
        let cancel = CancelFlag::new();
        while let Some((window, samples)) = windows.next(&cancel).unwrap() {
            assert_eq!(samples.len(), window.end - window.start);
            for (offset, sample) in samples.iter().enumerate() {
                let index = window.start + offset;
                let expected = if index < audio_len { value(index) } else { 0.0 };
                assert_eq!(*sample, expected, "sample {index}");
            }
            all.push(window);
        }
        all
    }

    fn ramp(len: usize, reads: &[usize]) -> Vec<Window> {
        cut(&mut Ramp::new(len, reads), len)
    }

    #[test]
    fn input_that_fills_one_window_with_its_pad_is_one_window() {
        let windows = ramp(WINDOW - TAIL_PAD, &[4_096]);
        assert_eq!(
            windows,
            [Window {
                start: 0,
                end: WINDOW,
                keep_from: 0,
                keep_to: usize::MAX,
            }]
        );
    }

    #[test]
    fn long_input_overlaps_by_ten_seconds() {
        let len = 600 * SAMPLE_RATE;
        let windows = ramp(len, &[1_152]);
        assert_eq!(windows.len(), 6);
        for pair in windows.windows(2) {
            assert_eq!(pair[0].end - pair[1].start, OVERLAP);
        }
        assert_eq!(windows.last().map(|w| w.end), Some(len + TAIL_PAD));
    }

    #[test]
    fn every_instant_has_one_owner() {
        let windows = ramp(300 * SAMPLE_RATE + 123, &[1, 7, 1_152, 4_096]);
        assert_eq!(windows[0].keep_from, 0);
        for pair in windows.windows(2) {
            assert_eq!(pair[0].keep_to, pair[1].keep_from);
        }
        assert_eq!(windows.last().map(|w| w.keep_to), Some(usize::MAX));
    }

    #[test]
    fn the_last_window_can_be_short() {
        let len = WINDOW + 20 * SAMPLE_RATE;
        let windows = ramp(len, &[4_096]);
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].start, WINDOW - OVERLAP);
        assert_eq!(windows[1].end, len + TAIL_PAD);
    }

    #[test]
    fn the_cut_does_not_depend_on_the_read_sizes() {
        let len = 250 * SAMPLE_RATE + 9;
        let reference = ramp(len, &[len]);
        for reads in [&[1_usize, 4_095][..], &[OVERLAP - 1], &[WINDOW + 1]] {
            assert_eq!(ramp(len, reads), reference, "{reads:?}");
        }
        let pcm: Vec<f32> = (0..len).map(value).collect();
        assert_eq!(cut(&mut SliceSource::new(&pcm), len), reference);
    }

    #[test]
    fn an_hour_is_cut_without_holding_more_than_a_window() {
        let mut source = Ramp::new(3_600 * SAMPLE_RATE, &[1_152]);
        let mut windows = Windows::new(&mut source);
        let cancel = CancelFlag::new();
        let mut count = 0;
        while windows.next(&cancel).unwrap().is_some() {
            count += 1;
            assert!(windows.buffer.capacity() <= WINDOW + OVERLAP);
        }
        assert_eq!(count, 33);
    }

    #[test]
    fn a_cancel_stops_the_reads_mid_stream() {
        let mut source = Ramp::new(600 * SAMPLE_RATE, &[1_152]);
        let cancel = CancelFlag::new();
        let mut windows = Windows::new(&mut source);
        assert!(windows.next(&cancel).unwrap().is_some());
        cancel.cancel();
        assert_eq!(windows.next(&cancel), Err(EngineError::Cancelled));
        drop(windows);
        assert!(source.given < 2 * WINDOW, "read {} samples", source.given);
    }

    #[test]
    fn a_source_with_no_samples_is_no_audio() {
        let mut source = SliceSource::new(&[]);
        let mut windows = Windows::new(&mut source);
        assert_eq!(windows.next(&CancelFlag::new()), Err(EngineError::NoAudio));
    }

    #[test]
    fn the_pad_never_shows_in_the_segments() {
        let audio = 2 * SAMPLE_RATE;
        let kept = clip_to_audio(
            vec![
                seg(0, 1_000, "inside"),
                seg(1_500, 2_200, "runs into the pad"),
                seg(2_000, 2_300, "only in the pad"),
            ],
            audio,
        );
        let spans: Vec<_> = kept.iter().map(|s| (s.start_ms, s.end_ms)).collect();
        assert_eq!(spans, [(0, 1_000), (1_500, 2_000)]);
    }

    #[test]
    fn segments_move_to_input_time_and_the_overlap_is_split_once() {
        let second = ramp(300 * SAMPLE_RATE, &[4_096])[1];
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
