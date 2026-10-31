//! The processing animation of the flow bar: a small wave of beads that floats and flows
//! while the words are transcribed and cleaned up. Each stage has its own look, and a wait
//! that runs long turns once to a calmer look, so the pill never seems stuck. Every frame is a
//! pure function of the time, so the pixel tests draw any moment from plain data.

use std::f32::consts::{PI, TAU};

/// What the pipeline is busy with while the bar shows its processing pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Transcribing,
    Cleaning,
    /// The cleanup model loads, which can take seconds the first time.
    Preparing,
}

/// One look of the wave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Bars that rise and fall in a wave that travels from left to right.
    Flow,
    /// A dotted line that traces a moving sine.
    Ribbon,
    /// Rings that move out from the middle, slower than the flow.
    Ripple,
    /// The whole wave swells and settles, slowly.
    Breathe,
}

const STYLES: [Style; 4] = [Style::Flow, Style::Ribbon, Style::Ripple, Style::Breathe];

/// The beads of the wave.
pub const BEADS: usize = 9;
pub const BEAD_WIDTH: f32 = 3.0;
pub const BEAD_GAP: f32 = 3.0;
const REST: f32 = 3.0;
const TALL: f32 = 16.0;
/// How far the ribbon's dots move up and down.
const RIBBON_LIFT: f32 = 5.0;
/// After this long in one stage the wave turns to the calmer look of the stage.
pub const LONG_WAIT_MS: u64 = 2_000;
/// One look turns into the next over this time.
const BLEND_MS: f32 = 420.0;
/// The gentle float of the whole wave.
const BOB_MS: f32 = 2_600.0;
const BOB_PX: f32 = 1.5;

/// One bead on one frame: its height, how far it sits above the middle line, and its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bead {
    pub height: f32,
    pub lift: f32,
    pub glow: f32,
}

/// How much of each look the wave shows, in the order of [`STYLES`]. The parts add up to 1.
/// A new stage starts from the mix the wave showed last, so it never jumps, even when the
/// stage changes in the middle of a turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mix([f32; 4]);

impl Mix {
    pub fn of(style: Style) -> Self {
        let mut parts = [0.0; 4];
        parts[index(style)] = 1.0;
        Self(parts)
    }

    pub fn part(self, style: Style) -> f32 {
        self.0[index(style)]
    }

    /// `done` (0 to 1) of the way from this mix to `style`, eased.
    fn toward(self, style: Style, done: f32) -> Self {
        let goal = Self::of(style);
        let mix = smooth(done);
        Self(std::array::from_fn(|i| {
            self.0[i] + (goal.0[i] - self.0[i]) * mix
        }))
    }
}

impl From<Stage> for Mix {
    fn from(stage: Stage) -> Self {
        Self::of(base(stage))
    }
}

fn index(style: Style) -> usize {
    STYLES.iter().position(|each| *each == style).unwrap_or(0)
}

/// The look each stage starts with.
pub fn base(stage: Stage) -> Style {
    match stage {
        Stage::Transcribing => Style::Flow,
        Stage::Cleaning => Style::Ribbon,
        Stage::Preparing => Style::Breathe,
    }
}

/// The calmer look a stage turns to once it runs past [`LONG_WAIT_MS`].
pub fn long(stage: Stage) -> Style {
    match stage {
        Stage::Transcribing => Style::Ripple,
        Stage::Cleaning | Stage::Preparing => Style::Breathe,
    }
}

/// What the wave shows `ms` into `stage`. `from` is what it showed when the stage began.
pub fn mix_at(stage: Stage, from: Option<Mix>, ms: u64) -> Mix {
    let first = base(stage);
    let start = from.map_or(Mix::of(first), |from| {
        from.toward(first, ms as f32 / BLEND_MS)
    });
    if ms < LONG_WAIT_MS {
        return start;
    }
    start.toward(long(stage), (ms - LONG_WAIT_MS) as f32 / BLEND_MS)
}

/// The beads `ms` into `stage`. `phase_ms` is the time since the processing began: every
/// look keeps moving on that one clock, so a new stage does not restart a look that was
/// already showing.
pub fn beads(stage: Stage, from: Option<Mix>, ms: u64, phase_ms: u64) -> Vec<Bead> {
    blend(mix_at(stage, from, ms), phase_ms as f32)
}

fn blend(mix: Mix, t: f32) -> Vec<Bead> {
    let mut beads = vec![
        Bead {
            height: 0.0,
            lift: 0.0,
            glow: 0.0,
        };
        BEADS
    ];
    for style in STYLES {
        let part = mix.part(style);
        if part <= 0.0 {
            continue;
        }
        for (bead, look) in beads.iter_mut().zip(frame(style, t)) {
            bead.height += look.height * part;
            bead.lift += look.lift * part;
            bead.glow += look.glow * part;
        }
    }
    beads
}
/// The whole wave's float above the middle line, `ms` into the processing.
pub fn bob(ms: u64) -> f32 {
    BOB_PX * (TAU * ms as f32 / BOB_MS).sin()
}

/// The wave with Reduce Motion: one still frame of the stage's look, a calm hump in the
/// middle.
pub fn still(stage: Stage) -> Vec<Bead> {
    let style = base(stage);
    frame(style, still_ms(style))
}

fn still_ms(style: Style) -> f32 {
    match style {
        // The crest of the travelling wave sits on the middle bead.
        Style::Flow => 0.3 * FLOW_MS,
        Style::Ribbon => 0.25 * RIBBON_MS,
        Style::Ripple => 0.0,
        Style::Breathe => 0.5 * BREATHE_MS,
    }
}

const FLOW_MS: f32 = 1_300.0;
const RIBBON_MS: f32 = 1_700.0;
const RIPPLE_MS: f32 = 1_800.0;
// The long wait is the calmer look: its ripple must be slower than the flow it follows.
const _: () = assert!(RIPPLE_MS > FLOW_MS);
const BREATHE_MS: f32 = 2_200.0;

fn frame(style: Style, t: f32) -> Vec<Bead> {
    (0..BEADS)
        .map(|index| {
            let x = index as f32 / (BEADS - 1) as f32;
            // The middle beads reach higher than the ends, which rounds the wave.
            let hump = 0.35 + 0.65 * (PI * x).sin();
            match style {
                Style::Flow => {
                    let wave = 0.5 + 0.5 * (TAU * (1.1 * x - t / FLOW_MS)).sin();
                    Bead {
                        height: REST + (TALL - REST) * hump * wave,
                        lift: 0.0,
                        glow: 0.55 + 0.45 * wave,
                    }
                }
                Style::Ribbon => {
                    let angle = TAU * (0.9 * x - t / RIBBON_MS);
                    Bead {
                        height: REST,
                        lift: RIBBON_LIFT * (0.45 + 0.55 * (PI * x).sin()) * angle.sin(),
                        glow: 0.6 + 0.4 * (0.5 + 0.5 * angle.cos()),
                    }
                }
                Style::Ripple => {
                    let away = (2.0 * x - 1.0).abs();
                    let wave = 0.5 + 0.5 * (TAU * (0.9 * away - t / RIPPLE_MS)).cos();
                    Bead {
                        height: REST + (TALL - REST) * (1.0 - 0.55 * away) * wave,
                        lift: 0.0,
                        glow: 0.5 + 0.5 * wave,
                    }
                }
                Style::Breathe => {
                    let swell = 0.5 - 0.5 * (TAU * t / BREATHE_MS).cos();
                    // A fixed ripple in the outline, so the swell reads as a wave and not as a
                    // block.
                    let outline = hump * (0.8 + 0.2 * (3.0 * TAU * x).cos());
                    Bead {
                        height: REST + (TALL - REST) * (0.2 + 0.8 * swell) * outline,
                        lift: 0.0,
                        glow: 0.55 + 0.45 * swell,
                    }
                }
            }
        })
        .collect()
}

fn smooth(done: f32) -> f32 {
    let done = done.clamp(0.0, 1.0);
    done * done * (3.0 - 2.0 * done)
}

/// The width of the wave.
pub fn width() -> f32 {
    BEADS as f32 * BEAD_WIDTH + (BEADS - 1) as f32 * BEAD_GAP
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frames the live bar draws: `first` until `switch_ms`, then `then`, which starts
    /// from what the wave showed at the switch. The clock of the looks never restarts.
    fn live_frames(first: Stage, then: Stage, switch_ms: u64, until_ms: u64) -> Vec<Vec<Bead>> {
        let from = mix_at(first, None, switch_ms);
        (0..until_ms)
            .step_by(16)
            .map(|ms| {
                if ms < switch_ms {
                    beads(first, None, ms, ms)
                } else {
                    beads(then, Some(from), ms - switch_ms, ms)
                }
            })
            .collect()
    }

    #[test]
    fn each_stage_starts_with_its_own_look() {
        assert_eq!(mix_at(Stage::Transcribing, None, 100), Mix::of(Style::Flow));
        assert_eq!(mix_at(Stage::Cleaning, None, 1_000), Mix::of(Style::Ribbon));
        assert_eq!(
            mix_at(Stage::Preparing, None, 1_000),
            Mix::of(Style::Breathe)
        );
    }

    #[test]
    fn a_long_wait_turns_once_to_a_calmer_look_and_stays() {
        for (stage, calm) in [
            (Stage::Transcribing, Style::Ripple),
            (Stage::Cleaning, Style::Breathe),
            (Stage::Preparing, Style::Breathe),
        ] {
            let turning = mix_at(stage, None, LONG_WAIT_MS + 100).part(calm);
            if calm != base(stage) {
                assert!(turning > 0.0 && turning < 1.0, "{stage:?} {turning}");
            }
            for ms in [LONG_WAIT_MS + 500, LONG_WAIT_MS + 30_000] {
                assert_eq!(mix_at(stage, None, ms), Mix::of(calm), "{stage:?} {ms}");
            }
        }
    }

    #[test]
    fn a_new_stage_turns_from_what_the_wave_showed() {
        // The switch lands in the middle of the transcription's turn to the ripple.
        let switch = LONG_WAIT_MS + 150;
        let from = mix_at(Stage::Transcribing, None, switch);
        assert!(from.part(Style::Flow) > 0.0 && from.part(Style::Ripple) > 0.0);
        assert_eq!(
            beads(Stage::Cleaning, Some(from), 0, switch),
            beads(Stage::Transcribing, None, switch, switch),
            "the first frame of the new stage is the last frame of the old one"
        );
        assert_eq!(
            mix_at(Stage::Cleaning, Some(from), 500),
            Mix::of(Style::Ribbon)
        );
        assert_eq!(Mix::from(Stage::Cleaning), Mix::of(Style::Ribbon));
    }

    #[test]
    fn no_frame_jumps_and_every_bead_stays_inside_the_pill() {
        let end = LONG_WAIT_MS * 4;
        let mut runs = vec![
            live_frames(Stage::Transcribing, Stage::Transcribing, 0, end),
            live_frames(Stage::Preparing, Stage::Preparing, 0, end),
        ];
        for switch in [300, LONG_WAIT_MS + 150, 5_000] {
            runs.push(live_frames(
                Stage::Transcribing,
                Stage::Cleaning,
                switch,
                end,
            ));
            runs.push(live_frames(
                Stage::Transcribing,
                Stage::Preparing,
                switch,
                end,
            ));
        }
        runs.push(live_frames(Stage::Preparing, Stage::Cleaning, 200, end));
        for frames in &runs {
            for pair in frames.windows(2) {
                for (a, b) in pair[0].iter().zip(&pair[1]) {
                    assert!((a.height - b.height).abs() < 2.5, "{a:?} {b:?}");
                    assert!((a.lift - b.lift).abs() < 1.0, "{a:?} {b:?}");
                    assert!((a.glow - b.glow).abs() < 0.1, "{a:?} {b:?}");
                }
            }
            for bead in frames.iter().flatten() {
                assert!(
                    (REST - 0.01..=TALL + 0.01).contains(&bead.height),
                    "{bead:?}"
                );
                let top = bead.lift.abs() + bead.height / 2.0 + BOB_PX;
                assert!(
                    top <= 13.0,
                    "{bead:?} reaches {top} from the middle of a 32 px pill"
                );
                assert!((0.3..=1.0).contains(&bead.glow), "{bead:?}");
            }
        }
    }

    #[test]
    fn the_wave_moves_and_the_still_frame_is_a_calm_hump() {
        assert_ne!(
            beads(Stage::Transcribing, None, 0, 0),
            beads(Stage::Transcribing, None, 300, 300)
        );
        let still = still(Stage::Transcribing);
        assert_eq!(still.len(), BEADS);
        let middle = still[BEADS / 2].height;
        assert!(still.iter().all(|bead| bead.height <= middle + 0.01));
        assert!(still[0].height < middle / 2.0);
        assert!((bob(0)).abs() < 0.01 && bob(650) > 1.4);
    }
}
