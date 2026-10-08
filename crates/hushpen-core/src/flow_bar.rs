//! The flow bar's rules without any window: what it shows for each pipeline state, where it
//! sits on the screen, how tall the waveform bars are, and what a press and release mean.

use crate::dictation::State;
use crate::error::{INSERT_KEYBOARD_GRABBED, INSERT_SECURE_FIELD};

/// How long an error or a blocked notice stays on the bar after the pipeline went idle again.
/// The pipeline itself clears a failure after two seconds, which is too short to read the
/// message and click "Open history".
pub const ERROR_HOLD_MS: u64 = 8_000;
/// How far the pointer must move after a press before the press is a drag and not a click.
pub const DRAG_THRESHOLD: f32 = 4.0;
/// The gap between the bar and the screen edge for the two presets.
pub const EDGE_MARGIN_TOP: f32 = 40.0;
pub const EDGE_MARGIN_BOTTOM: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarState {
    /// The small resting pill.
    Idle,
    /// The waveform.
    Listening,
    Transcribing,
    /// A short flash after the text went in.
    Result,
    Error,
    /// The text was not inserted on purpose: a secure field or another app holds the keyboard.
    Blocked,
}

impl BarState {
    pub fn key(self) -> &'static str {
        match self {
            BarState::Idle => "idle",
            BarState::Listening => "listening",
            BarState::Transcribing => "transcribing",
            BarState::Result => "result",
            BarState::Error => "error",
            BarState::Blocked => "blocked",
        }
    }

    /// States that stay on the bar until the user acts or the hold runs out.
    pub fn is_failure(self) -> bool {
        matches!(self, BarState::Error | BarState::Blocked)
    }
}

/// What the bar shows for a pipeline state. `failure` is the code of the failure that ended
/// the run, when there is one.
pub fn state_for(pipeline: State, failure: Option<&str>) -> BarState {
    match pipeline {
        State::Idle | State::Cancelled => BarState::Idle,
        State::Listening => BarState::Listening,
        State::Transcribing | State::Cleaning | State::Inserting => BarState::Transcribing,
        State::Done => BarState::Result,
        State::Failed => match failure {
            Some(INSERT_KEYBOARD_GRABBED | INSERT_SECURE_FIELD) => BarState::Blocked,
            _ => BarState::Error,
        },
    }
}

/// Keeps a failure on the bar for [`ERROR_HOLD_MS`] after the pipeline went idle.
#[derive(Debug, Default)]
pub struct Display {
    held: Option<(BarState, u64)>,
}

impl Display {
    /// What to draw now, given what the pipeline says at `now_ms` (any monotonic scale).
    pub fn update(&mut self, derived: BarState, now_ms: u64) -> BarState {
        if derived.is_failure() {
            if self.held.is_none_or(|(state, _)| state != derived) {
                self.held = Some((derived, now_ms));
            }
            return derived;
        }
        if derived != BarState::Idle {
            self.held = None;
            return derived;
        }
        match self.held {
            Some((state, since)) if now_ms.saturating_sub(since) < ERROR_HOLD_MS => state,
            _ => {
                self.held = None;
                BarState::Idle
            }
        }
    }

    /// The failure goes away at once, for a click that dismisses it.
    pub fn clear(&mut self) {
        self.held = None;
    }
}

/// Where the user wants the bar: one of the two edge presets, or a spot they dragged it to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spot {
    Top,
    Bottom,
    /// The horizontal center of the bar and the edge that touches the screen side it is on:
    /// the bottom edge in the lower half of the screen, the top edge in the upper half. A bar
    /// that changes size keeps that edge and that center.
    Custom {
        center_x: f32,
        edge_y: f32,
    },
}

impl Spot {
    /// Reads `overlay.position` and `overlay.customPos`.
    pub fn from_settings(position: &str, custom: Option<(f32, f32)>) -> Spot {
        match custom {
            Some((center_x, edge_y)) => Spot::Custom { center_x, edge_y },
            None if position == "top" => Spot::Top,
            None => Spot::Bottom,
        }
    }

    /// Whether a menu on the bar opens downward: the bar sits in the upper half of the screen.
    pub fn opens_downward(self, screen: (f32, f32)) -> bool {
        match self {
            Spot::Top => true,
            Spot::Bottom => false,
            Spot::Custom { edge_y, .. } => edge_y < screen.1 / 2.0,
        }
    }

    /// The custom spot for a bar whose top left corner is at `origin` with `size`.
    pub fn dragged(origin: (f32, f32), size: (f32, f32), screen: (f32, f32)) -> Spot {
        let center_x = origin.0 + size.0 / 2.0;
        let center_y = origin.1 + size.1 / 2.0;
        let edge_y = if center_y >= screen.1 / 2.0 {
            origin.1 + size.1
        } else {
            origin.1
        };
        Spot::Custom { center_x, edge_y }
    }
}

/// The top left corner of a bar of `size` on a screen of `screen`. The bar always lies fully
/// on the screen.
pub fn place(screen: (f32, f32), size: (f32, f32), spot: Spot) -> (f32, f32) {
    let (x, y) = match spot {
        Spot::Top => ((screen.0 - size.0) / 2.0, EDGE_MARGIN_TOP),
        Spot::Bottom => (
            (screen.0 - size.0) / 2.0,
            screen.1 - EDGE_MARGIN_BOTTOM - size.1,
        ),
        Spot::Custom { center_x, edge_y } => {
            let y = if edge_y >= screen.1 / 2.0 {
                edge_y - size.1
            } else {
                edge_y
            };
            (center_x - size.0 / 2.0, y)
        }
    };
    (
        x.clamp(0.0, (screen.0 - size.0).max(0.0)),
        y.clamp(0.0, (screen.1 - size.1).max(0.0)),
    )
}

/// What a pointer release meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Click,
    Dragged,
}

/// Tells a click from a drag. Pointer positions are relative to the bar window, which moves
/// while it is dragged, so the tracker hands back how far to move the window and never a
/// screen position.
#[derive(Debug, Default)]
pub struct Drag {
    pressed_at: Option<(f32, f32)>,
    dragging: bool,
}

impl Drag {
    pub fn press(&mut self, at: (f32, f32)) {
        self.pressed_at = Some(at);
        self.dragging = false;
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed_at.is_some()
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// How far to move the window so that the pressed point is under the pointer again.
    /// `None` while the press is still a click candidate or nothing is pressed.
    pub fn moved(&mut self, at: (f32, f32)) -> Option<(f32, f32)> {
        let start = self.pressed_at?;
        let delta = (at.0 - start.0, at.1 - start.1);
        if !self.dragging {
            if delta.0.hypot(delta.1) < DRAG_THRESHOLD {
                return None;
            }
            self.dragging = true;
        }
        Some(delta)
    }

    pub fn release(&mut self) -> Option<Release> {
        self.pressed_at.take()?;
        Some(if std::mem::take(&mut self.dragging) {
            Release::Dragged
        } else {
            Release::Click
        })
    }
}

/// The height of each waveform bar: `rest` for silence, `max` for the loudest level.
pub fn bar_heights(levels: &[f32], rest: f32, max: f32) -> Vec<f32> {
    levels
        .iter()
        .map(|level| rest + (max - rest) * level.clamp(0.0, 1.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (f32, f32) = (1280.0, 800.0);

    #[test]
    fn each_pipeline_state_maps_to_a_bar_state() {
        assert_eq!(state_for(State::Idle, None), BarState::Idle);
        assert_eq!(state_for(State::Cancelled, None), BarState::Idle);
        assert_eq!(state_for(State::Listening, None), BarState::Listening);
        for state in [State::Transcribing, State::Cleaning, State::Inserting] {
            assert_eq!(state_for(state, None), BarState::Transcribing);
        }
        assert_eq!(state_for(State::Done, None), BarState::Result);
    }

    #[test]
    fn a_blocked_insert_is_not_a_plain_error() {
        for code in ["INSERT_KEYBOARD_GRABBED", "INSERT_SECURE_FIELD"] {
            assert_eq!(state_for(State::Failed, Some(code)), BarState::Blocked);
        }
        for code in ["ENGINE_NO_SPEECH", "ENGINE_CRASHED", "INSERT_NO_RECEIPT"] {
            assert_eq!(state_for(State::Failed, Some(code)), BarState::Error);
        }
        assert_eq!(state_for(State::Failed, None), BarState::Error);
    }

    #[test]
    fn a_failure_stays_after_the_pipeline_is_idle_and_then_goes() {
        let mut display = Display::default();
        assert_eq!(display.update(BarState::Error, 10_000), BarState::Error);
        assert_eq!(display.update(BarState::Idle, 12_000), BarState::Error);
        assert_eq!(display.update(BarState::Idle, 17_999), BarState::Error);
        assert_eq!(display.update(BarState::Idle, 18_000), BarState::Idle);
        assert_eq!(display.update(BarState::Idle, 18_100), BarState::Idle);
    }

    #[test]
    fn the_hold_counts_from_the_first_frame_of_the_failure() {
        let mut display = Display::default();
        display.update(BarState::Blocked, 1_000);
        // The pipeline keeps reporting the failure for two seconds; that must not extend it.
        display.update(BarState::Blocked, 2_900);
        assert_eq!(display.update(BarState::Idle, 8_999), BarState::Blocked);
        assert_eq!(display.update(BarState::Idle, 9_000), BarState::Idle);
    }

    #[test]
    fn a_new_run_ends_the_hold() {
        let mut display = Display::default();
        display.update(BarState::Error, 0);
        assert_eq!(
            display.update(BarState::Listening, 3_000),
            BarState::Listening
        );
        assert_eq!(display.update(BarState::Idle, 3_500), BarState::Idle);
    }

    #[test]
    fn clearing_removes_the_failure_at_once() {
        let mut display = Display::default();
        display.update(BarState::Error, 0);
        display.clear();
        assert_eq!(display.update(BarState::Idle, 100), BarState::Idle);
    }

    #[test]
    fn the_default_spot_is_bottom_center_and_top_is_top_center() {
        let size = (120.0, 36.0);
        let (x, y) = place(SCREEN, size, Spot::from_settings("bottom", None));
        assert_eq!(x + size.0 / 2.0, 640.0);
        assert!(
            y >= 600.0 && y + size.1 <= 800.0,
            "bottom quarter, got y={y}"
        );
        let (x, y) = place(SCREEN, size, Spot::from_settings("top", None));
        assert_eq!(x + size.0 / 2.0, 640.0);
        assert!(y >= 0.0 && y + size.1 <= 200.0, "top quarter, got y={y}");
    }

    #[test]
    fn a_taller_bar_keeps_the_edge_of_its_preset() {
        let small = place(SCREEN, (120.0, 36.0), Spot::Bottom);
        let tall = place(SCREEN, (240.0, 300.0), Spot::Bottom);
        assert_eq!(small.1 + 36.0, tall.1 + 300.0);
        assert_eq!(small.0 + 60.0, tall.0 + 120.0);
        let small = place(SCREEN, (120.0, 36.0), Spot::Top);
        let tall = place(SCREEN, (240.0, 300.0), Spot::Top);
        assert_eq!(small.1, tall.1);
    }

    #[test]
    fn a_dragged_spot_comes_back_at_the_same_place() {
        let size = (120.0, 36.0);
        let origin = (340.0, 560.0);
        let spot = Spot::dragged(origin, size, SCREEN);
        assert_eq!(place(SCREEN, size, spot), origin);
        let high = Spot::dragged((340.0, 100.0), size, SCREEN);
        assert_eq!(place(SCREEN, size, high), (340.0, 100.0));
    }

    #[test]
    fn a_dragged_bar_that_grows_keeps_its_center_and_the_edge_on_its_side() {
        let spot = Spot::dragged((340.0, 560.0), (120.0, 36.0), SCREEN);
        let (x, y) = place(SCREEN, (240.0, 300.0), spot);
        assert_eq!(x + 120.0, 400.0);
        assert_eq!(
            y + 300.0,
            596.0,
            "the bottom edge stays put in the lower half"
        );
        let spot = Spot::dragged((340.0, 100.0), (120.0, 36.0), SCREEN);
        let (_, y) = place(SCREEN, (240.0, 300.0), spot);
        assert_eq!(y, 100.0, "the top edge stays put in the upper half");
    }

    #[test]
    fn a_bar_never_leaves_the_screen() {
        let size = (120.0, 36.0);
        let far = Spot::Custom {
            center_x: -500.0,
            edge_y: 5_000.0,
        };
        assert_eq!(place(SCREEN, size, far), (0.0, 764.0));
        let far = Spot::Custom {
            center_x: 9_000.0,
            edge_y: -10.0,
        };
        assert_eq!(place(SCREEN, size, far), (1160.0, 0.0));
    }

    #[test]
    fn a_saved_custom_position_wins_over_the_preset() {
        let spot = Spot::from_settings("top", Some((400.0, 596.0)));
        assert_eq!(
            spot,
            Spot::Custom {
                center_x: 400.0,
                edge_y: 596.0
            }
        );
    }

    #[test]
    fn a_menu_opens_away_from_the_screen_edge_the_bar_is_near() {
        assert!(Spot::Top.opens_downward(SCREEN));
        assert!(!Spot::Bottom.opens_downward(SCREEN));
        let high = Spot::Custom {
            center_x: 300.0,
            edge_y: 120.0,
        };
        let low = Spot::Custom {
            center_x: 300.0,
            edge_y: 600.0,
        };
        assert!(high.opens_downward(SCREEN));
        assert!(!low.opens_downward(SCREEN));
    }

    #[test]
    fn a_press_that_barely_moves_is_a_click() {
        let mut drag = Drag::default();
        drag.press((50.0, 18.0));
        assert_eq!(drag.moved((52.0, 19.0)), None);
        assert_eq!(drag.release(), Some(Release::Click));
        assert_eq!(drag.release(), None, "a second release has nothing to end");
    }

    #[test]
    fn a_press_that_moves_past_the_threshold_is_a_drag_and_gives_the_offset() {
        let mut drag = Drag::default();
        drag.press((50.0, 18.0));
        assert_eq!(drag.moved((10.0, 4.0)), Some((-40.0, -14.0)));
        assert!(drag.is_dragging());
        // Back near the start: still a drag, and the window follows.
        assert_eq!(drag.moved((51.0, 18.0)), Some((1.0, 0.0)));
        assert_eq!(drag.release(), Some(Release::Dragged));
        assert!(!drag.is_pressed());
    }

    #[test]
    fn a_move_with_nothing_pressed_does_nothing() {
        let mut drag = Drag::default();
        assert_eq!(drag.moved((300.0, 300.0)), None);
        assert_eq!(drag.release(), None);
    }

    #[test]
    fn silence_gives_flat_bars_and_a_loud_level_gives_the_tallest() {
        let heights = bar_heights(&[0.0, 0.5, 1.0, 7.0, -1.0], 3.0, 23.0);
        assert_eq!(heights, vec![3.0, 13.0, 23.0, 23.0, 3.0]);
    }
}
