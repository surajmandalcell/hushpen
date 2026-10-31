//! The processing animation of the flow bar: a small wave of beads that floats and flows
//! while the words are transcribed and cleaned up. Each stage has its own look and speed, and
//! a wait that runs long moves through the other looks, so the pill never seems stuck. Every
//! frame is a pure function of the time, so the pixel tests draw any moment from plain data.

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
    /// Rings that move out from the middle.
    Ripple,
    /// The whole wave swells and settles, slowly.
    Breathe,
}

/// The beads of the wave.
pub const BEADS: usize = 9;
pub const BEAD_WIDTH: f32 = 3.0;
pub const BEAD_GAP: f32 = 3.0;
const REST: f32 = 3.0;
const TALL: f32 = 16.0;
/// How far the ribbon's dots move up and down.
const RIBBON_LIFT: f32 = 5.0;
/// After this long in one stage the wave starts to change its look.
pub const LONG_WAIT_MS: u64 = 2_000;
/// How long each look shows during a long wait.
const TURN_MS: u64 = 2_400;
/// One look turns into the next over this time.
const BLEND_MS: f32 = 420.0;
/// The gentle float of the whole wave.
const BOB_MS: f32 = 2_600.0;
const BOB_PX: f32 = 1.5;
/// The order of the looks in a long wait. Each one has its own speed.
const CYCLE: [Style; 4] = [Style::Flow, Style::Ripple, Style::Ribbon, Style::Breathe];

/// One bead on one frame: its height, how far it sits above the middle line, and its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bead {
    pub height: f32,
    pub lift: f32,
    pub glow: f32,
}

/// The look each stage starts with.
pub fn base(stage: Stage) -> Style {
    match stage {
        Stage::Transcribing => Style::Flow,
        Stage::Cleaning => Style::Ribbon,
        Stage::Preparing => Style::Breathe,
    }
}

/// The look `ms` into a stage, and the one it is turning from with how far the turn is (0 to
/// 1), while a turn runs.
pub fn style_at(stage: Stage, from: Option<Stage>, ms: u64) -> (Style, Option<(Style, f32)>) {
    let first = base(stage);
    if ms < LONG_WAIT_MS {
        let turn = from
            .map(base)
            .filter(|style| *style != first)
            .map(|style| (style, ms as f32 / BLEND_MS))
            .filter(|(_, done)| *done < 1.0);
        return (first, turn);
    }
    let start = CYCLE.iter().position(|style| *style == first).unwrap_or(0);
    let turns = (ms - LONG_WAIT_MS) / TURN_MS;
    let pick = |turn: u64| CYCLE[(start + 1 + turn as usize) % CYCLE.len()];
    let now = pick(turns);
    let previous = turns.checked_sub(1).map_or(first, pick);
    let into = ((ms - LONG_WAIT_MS) % TURN_MS) as f32 / BLEND_MS;
    (now, (into < 1.0).then_some((previous, into)))
}

/// The beads `ms` into `stage`. `from` is the stage before it, so the wave turns from that
/// look instead of jumping.
pub fn beads(stage: Stage, from: Option<Stage>, ms: u64) -> Vec<Bead> {
    let t = ms as f32;
    match style_at(stage, from, ms) {
        (style, None) => frame(style, t),
        (style, Some((previous, done))) => {
            let mix = smooth(done);
            frame(previous, t)
                .into_iter()
                .zip(frame(style, t))
                .map(|(a, b)| Bead {
                    height: a.height + (b.height - a.height) * mix,
                    lift: a.lift + (b.lift - a.lift) * mix,
                    glow: a.glow + (b.glow - a.glow) * mix,
                })
                .collect()
        }
    }
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
const RIPPLE_MS: f32 = 900.0;
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

    fn every_frame(stage: Stage, from: Option<Stage>, until_ms: u64) -> Vec<Vec<Bead>> {
        (0..until_ms)
            .step_by(16)
            .map(|ms| beads(stage, from, ms))
            .collect()
    }

    #[test]
    fn each_stage_starts_with_its_own_look() {
        assert_eq!(
            style_at(Stage::Transcribing, None, 100),
            (Style::Flow, None)
        );
        assert_eq!(
            style_at(Stage::Cleaning, None, 1_000),
            (Style::Ribbon, None)
        );
        assert_eq!(
            style_at(Stage::Preparing, None, 1_000),
            (Style::Breathe, None)
        );
    }

    #[test]
    fn a_long_wait_moves_through_every_other_look() {
        let mut seen = Vec::new();
        for ms in (LONG_WAIT_MS..LONG_WAIT_MS + 4 * TURN_MS).step_by(TURN_MS as usize) {
            seen.push(style_at(Stage::Transcribing, None, ms + 500).0);
        }
        assert_eq!(
            seen,
            [Style::Ripple, Style::Ribbon, Style::Breathe, Style::Flow]
        );
        let (now, turn) = style_at(Stage::Cleaning, None, LONG_WAIT_MS + 100);
        assert_eq!(now, Style::Breathe);
        assert_eq!(turn.map(|(from, _)| from), Some(Style::Ribbon));
    }

    #[test]
    fn a_new_stage_turns_from_the_look_of_the_last_one() {
        let (now, turn) = style_at(Stage::Cleaning, Some(Stage::Transcribing), 0);
        assert_eq!((now, turn), (Style::Ribbon, Some((Style::Flow, 0.0))));
        assert_eq!(
            beads(Stage::Cleaning, Some(Stage::Transcribing), 0),
            beads(Stage::Transcribing, None, 0),
            "the first frame of the turn is the old look"
        );
        assert_eq!(
            style_at(Stage::Cleaning, Some(Stage::Transcribing), 500).1,
            None
        );
    }

    #[test]
    fn no_frame_jumps_and_every_bead_stays_inside_the_pill() {
        for (stage, from) in [
            (Stage::Transcribing, None),
            (Stage::Cleaning, Some(Stage::Transcribing)),
            (Stage::Preparing, None),
        ] {
            let frames = every_frame(stage, from, LONG_WAIT_MS + 5 * TURN_MS);
            for pair in frames.windows(2) {
                for (a, b) in pair[0].iter().zip(&pair[1]) {
                    assert!((a.height - b.height).abs() < 3.0, "{stage:?} {a:?} {b:?}");
                    assert!((a.lift - b.lift).abs() < 2.0, "{stage:?} {a:?} {b:?}");
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
            beads(Stage::Transcribing, None, 0),
            beads(Stage::Transcribing, None, 300)
        );
        let still = still(Stage::Transcribing);
        assert_eq!(still.len(), BEADS);
        let middle = still[BEADS / 2].height;
        assert!(still.iter().all(|bead| bead.height <= middle + 0.01));
        assert!(still[0].height < middle / 2.0);
        assert!((bob(0)).abs() < 0.01 && bob(650) > 1.4);
    }
}
