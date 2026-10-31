//! What the flow bar draws. [`Props`] holds everything a frame needs, so the pixel tests draw
//! each state from plain data and the live bar builds the same props from the pipeline.

use super::FlowBar;
use super::working::{self, BEAD_GAP, BEAD_WIDTH, Mix, Stage};
use crate::hook;
use crate::theme::hud::{BODY_SM, color, radius, size};
use crate::theme::{StyledType, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Div, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, rgb, svg,
};
use hushpen_core::flow_bar::{BarState, bar_heights};

const HEIGHT: f32 = 32.0;
pub const IDLE_SIZE: (f32, f32) = (64.0, 24.0);
/// Cancel, the waveform, and finish: 5 + 22 + 8 + 42 + 8 + 22 + 5.
pub const LISTENING_SIZE: (f32, f32) = (112.0, HEIGHT);
/// The processing wave keeps the listening size, so the pill does not jump when the words
/// stop.
pub const TRANSCRIBING_SIZE: (f32, f32) = LISTENING_SIZE;
pub const RESULT_SIZE: (f32, f32) = (112.0, HEIGHT);
/// The widest notice. A notice is as wide as its line, up to this; a longer line truncates.
pub const NOTICE_SIZE: (f32, f32) = (360.0, HEIGHT);
/// What a character of the notice text takes on average, and room on top for a short line
/// of wide letters, so a line fits its pill on Inter and on the system font.
const NOTICE_CHAR: f32 = 6.3;
const NOTICE_SLACK: f32 = 12.0;
/// The tick and "Pasted", or "Undo paste" while the pointer is on the button, then the round
/// undo button.
pub const UNDO_SIZE: (f32, f32) = (120.0, HEIGHT);
/// The processing pill while the cleanup model loads: it says why the wait is longer.
pub const PREPARING_SIZE: (f32, f32) = (216.0, HEIGHT);

/// The bars of the listening waveform.
pub const WAVE_BARS: usize = 11;
const IDLE_BARS: usize = 9;
const WAVE_REST: f32 = 3.0;
const WAVE_MAX: f32 = 18.0;
const BAR_WIDTH: f32 = 2.0;
const ROUND_BUTTON: f32 = 22.0;
pub const UNDO_LABEL: &str = "Undo paste";
pub const HISTORY_LABEL: &str = "Open history";

#[derive(Debug, Clone, PartialEq)]
pub struct Props {
    pub state: BarState,
    /// The height of each waveform bar, 0 to 1. Only the listening state draws them.
    pub levels: Vec<f32>,
    /// What the processing pill waits for, and what the wave showed when it began.
    pub stage: Stage,
    pub from: Option<Mix>,
    /// How long the stage and the whole processing have run: the stage picks the look, the
    /// whole processing moves it.
    pub stage_ms: u64,
    pub work_ms: u64,
    /// Reduce Motion: the wave is one still frame.
    pub still: bool,
    /// The text of the result, undo, error, and blocked states.
    pub message: String,
    pub open_history: bool,
    /// The pointer is on the round button of the undo or notice pill: the text names what it
    /// does, because the pop-up is too small for a tooltip.
    pub hover: bool,
    /// The window size on this frame while the bar moves; `None` is the size of the state.
    pub frame: Option<(f32, f32)>,
    /// How much of the bar is there: 0 is gone, 1 is fully shown.
    pub presence: f32,
    /// The opacity of the content of the state.
    pub fade: f32,
}

impl Props {
    pub fn new(state: BarState) -> Self {
        Self {
            state,
            levels: Vec::new(),
            stage: Stage::Transcribing,
            from: None,
            stage_ms: 0,
            work_ms: 0,
            still: false,
            message: String::new(),
            open_history: false,
            hover: false,
            frame: None,
            presence: 1.0,
            fade: 1.0,
        }
    }

    pub fn preparing(&self) -> bool {
        self.stage == Stage::Preparing
    }

    /// What the processing pill says: only the wait for the model shows its text, the others
    /// are the label of the pill.
    pub fn working_text(&self) -> &'static str {
        if self.preparing() {
            "Preparing the AI model"
        } else {
            "Transcribing"
        }
    }
}

/// The size the bar settles at for these props.
pub fn size_of(props: &Props) -> (f32, f32) {
    match props.state {
        BarState::Idle => IDLE_SIZE,
        BarState::Listening => LISTENING_SIZE,
        BarState::Transcribing if props.preparing() => PREPARING_SIZE,
        BarState::Transcribing => TRANSCRIBING_SIZE,
        BarState::Result => RESULT_SIZE,
        BarState::Undo => UNDO_SIZE,
        BarState::Error | BarState::Blocked => notice_size(props),
    }
}

/// The inset, the icon, the line, then the round button or the inset again.
fn notice_size(props: &Props) -> (f32, f32) {
    let mut chars = props.message.chars().count();
    if props.open_history {
        chars = chars.max(HISTORY_LABEL.len());
    }
    let end = if props.open_history {
        space::SM + ROUND_BUTTON + size::LISTENING_INSET
    } else {
        size::INSET
    };
    let width =
        size::INSET + size::ICON + space::SM + chars as f32 * NOTICE_CHAR + NOTICE_SLACK + end;
    (width.ceil().min(NOTICE_SIZE.0), HEIGHT)
}

/// Where each waveform bar wants to be, from the meter (oldest value first). The newest level
/// is in the middle and older ones spread out to both sides, so a word ripples outward.
pub fn wave_goal(meter: &[f32]) -> Vec<f32> {
    let center = WAVE_BARS / 2;
    (0..WAVE_BARS)
        .map(|bar| {
            let age = bar.abs_diff(center);
            let level = meter
                .len()
                .checked_sub(age + 1)
                .map_or(0.0, |index| meter[index]);
            // The outer bars are a little shorter, which gives the wave its round shape.
            level.clamp(0.0, 1.0) * (1.0 - 0.07 * age as f32)
        })
        .collect()
}

/// The text of the result flash: what happened to the words. History names the same outcomes
/// "Pasted" and "Copied only".
pub fn result_text(copied: bool) -> &'static str {
    if copied { "Copied" } else { "Pasted" }
}

/// A short line for a failure code; the full text is in History.
pub fn failure_text(code: Option<&str>) -> &'static str {
    use hushpen_core::error::*;
    match code {
        Some(ENGINE_NO_SPEECH) => "No speech heard",
        Some(INSERT_KEYBOARD_GRABBED) => "Another app holds the keyboard",
        Some(INSERT_SECURE_FIELD) => "Secure field. Text not pasted",
        Some(INSERT_NO_RECEIPT) => "The app did not take the text",
        Some(INSERT_NO_PERMISSION) => "Not allowed to paste. Text copied",
        Some(CAPTURE_FAILED) => "Recording stopped",
        Some(ENGINE_NO_MODEL | ENGINE_LOAD_FAILED) => "Speech model not ready",
        Some(COMMAND_NO_SELECTION | READ_NO_SELECTION) => "Select text first",
        Some(COMMAND_NEEDS_AI) => "Command mode needs AI cleanup or the endpoint",
        Some(COMMAND_NEEDS_GENERAL_MODEL) => "Command mode needs Qwen3.5 2B",
        Some(LLM_TIMEOUT) => "AI edit timed out. Text unchanged",
        Some(
            LLM_GUARD_REJECTED | LLM_UNAVAILABLE | LLM_LOAD_FAILED | LLM_NO_MODEL | LLM_PROTOCOL
            | LLM_CRASHED | LLM_PROMPT_TOO_LONG | LLM_GENERATE_FAILED | ENDPOINT_FAILED,
        ) => "AI edit failed. Text unchanged",
        _ => "Transcription failed",
    }
}

fn tone(state: BarState) -> u32 {
    match state {
        BarState::Error => color::STATUS_ERROR,
        BarState::Blocked => color::STATUS_WARNING,
        _ => color::TEXT_PRIMARY,
    }
}

/// The whole bar. `bar` is the live entity the controls call; the pixel tests pass `None`.
pub fn content(
    props: &Props,
    bar: Option<&Entity<FlowBar>>,
    composited: bool,
) -> impl IntoElement + use<> {
    let (width, height) = props.frame.unwrap_or_else(|| size_of(props));
    let label: SharedString = match props.state {
        BarState::Idle => "Start dictation".into(),
        BarState::Listening => "Stop dictation".into(),
        BarState::Transcribing => props.working_text().into(),
        _ if props.message.is_empty() => props.state.key().into(),
        _ => props.message.clone().into(),
    };
    let mut root = div()
        .id(hook::id("flowbar", "bar"))
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .w(px(width))
        .h(px(height))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .opacity(props.presence)
        .bg(rgb(color::SURFACE_ELEVATED))
        .border_1()
        .border_color(rgb(color::DIVIDER))
        .cursor_pointer();
    // Without a compositor the window cannot be see-through and pill corners would render
    // black, so the bar fills its window as a rectangle.
    root = if composited {
        root.rounded(px(radius::FULL))
    } else {
        root.rounded_none()
    };
    if let Some(bar) = bar {
        // The bar tells a click from a drag itself, from the pointer on the screen: a pop-up
        // that moves under the pointer cannot rely on the positions it is sent.
        let (down, up, up_outside) = (bar.clone(), bar.clone(), bar.clone());
        root = root
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                down.update(cx, |bar, cx| bar.press(window, cx));
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                up.update(cx, |bar, cx| bar.release(cx));
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                up_outside.update(cx, |bar, cx| bar.release(cx));
            });
    }
    // The row keeps the size of its state while the frame grows or shrinks around it, so the
    // frame reveals the content from the middle instead of squeezing it.
    let (row_width, row_height) = size_of(props);
    root.child(
        pill_row(props, bar)
            .w(px(row_width))
            .h(px(row_height))
            .flex_none()
            .opacity(props.fade),
    )
}

fn pill_row(props: &Props, bar: Option<&Entity<FlowBar>>) -> Div {
    let base = div().flex().items_center().px(px(size::INSET));
    match props.state {
        BarState::Idle => base
            .justify_center()
            .gap(px(BAR_WIDTH))
            .children((0..IDLE_BARS).map(|_| wave_bar(WAVE_REST, color::TEXT_SUBTLE))),
        BarState::Listening => {
            let heights = bar_heights(&props.levels_padded(), WAVE_REST, WAVE_MAX);
            base.px(px(size::LISTENING_INSET))
                .justify_between()
                .child(round_button(Round::Cancel, bar))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(BAR_WIDTH))
                        .children(heights.into_iter().map(|height| {
                            let tone = if height > WAVE_REST + 0.5 {
                                color::TEXT_PRIMARY
                            } else {
                                color::TEXT_MUTED
                            };
                            wave_bar(height, tone)
                        })),
                )
                .child(round_button(Round::Finish, bar))
        }
        BarState::Transcribing => {
            let row = base
                .justify_center()
                .gap(px(space::SM))
                .child(working_wave(props));
            if props.preparing() {
                row.child(status_text(props.working_text(), color::TEXT_BODY))
            } else {
                row
            }
        }
        BarState::Result => base
            .justify_center()
            .gap(px(space::SM))
            .child(icon(IconName::Check, color::TEXT_PRIMARY))
            .child(status_text(&props.message, color::TEXT_PRIMARY)),
        BarState::Undo => {
            let said = div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(space::SM));
            let said = if props.hover {
                said.child(status_text(UNDO_LABEL, color::TEXT_PRIMARY))
            } else {
                said.child(icon(IconName::Check, color::TEXT_PRIMARY))
                    .child(status_text(&props.message, color::TEXT_PRIMARY))
            };
            base.pr(px(size::LISTENING_INSET))
                .gap(px(space::XS))
                .child(said)
                .child(round_button(Round::Undo, bar))
        }
        BarState::Error | BarState::Blocked => {
            let (line, tone_of_line) = if props.open_history && props.hover {
                (HISTORY_LABEL, color::TEXT_PRIMARY)
            } else {
                (props.message.as_str(), color::TEXT_BODY)
            };
            let notice = base
                .gap(px(space::SM))
                .child(icon(IconName::CircleAlert, tone(props.state)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(status_text(line, tone_of_line)),
                );
            if props.open_history {
                notice
                    .pr(px(size::LISTENING_INSET))
                    .child(round_button(Round::History, bar))
            } else {
                notice
            }
        }
    }
}

impl Props {
    /// The levels with one entry for every waveform bar: missing bars rest.
    fn levels_padded(&self) -> Vec<f32> {
        let mut levels = self.levels.clone();
        levels.resize(WAVE_BARS, 0.0);
        levels
    }
}

/// The processing wave: beads that flow, floating a little up and down as a whole.
fn working_wave(props: &Props) -> Div {
    let (beads, bob) = if props.still {
        (working::still(props.stage), 0.0)
    } else {
        (
            working::beads(props.stage, props.from, props.stage_ms, props.work_ms),
            working::bob(props.work_ms),
        )
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .w(px(working::width()))
        .h(px(HEIGHT))
        .gap(px(BEAD_GAP))
        .relative()
        .top(px(-bob))
        .children(beads.into_iter().map(|bead| {
            div()
                .w(px(BEAD_WIDTH))
                .h(px(bead.height))
                .flex_none()
                .relative()
                .top(px(-bead.lift))
                .rounded(px(BEAD_WIDTH / 2.0))
                .bg(rgb(color::TEXT_PRIMARY))
                .opacity(bead.glow)
        }))
}

fn wave_bar(height: f32, tone: u32) -> Div {
    div()
        .w(px(BAR_WIDTH))
        .h(px(height))
        .flex_none()
        .rounded(px(BAR_WIDTH / 2.0))
        .bg(rgb(tone))
}

fn icon(name: IconName, tone: u32) -> impl IntoElement {
    svg()
        .path(name.path())
        .size(px(size::ICON))
        .flex_none()
        .text_color(rgb(tone))
}

fn status_text(text: &str, tone: u32) -> impl IntoElement {
    div()
        .id(hook::id("flowbar", "status"))
        .test_support()
        .role(Role::Status)
        .aria_label(text.to_owned())
        .truncate()
        .text_token(BODY_SM)
        .text_color(rgb(tone))
        .child(text.to_owned())
}

#[derive(Clone, Copy)]
enum Round {
    Cancel,
    Finish,
    Undo,
    History,
}

/// The round cross and tick of the listening state, and the one button of the undo and the
/// notice pills.
fn round_button(kind: Round, bar: Option<&Entity<FlowBar>>) -> impl IntoElement {
    let (id, label, name, fill, hover, glyph) = match kind {
        Round::Cancel => (
            "cancel",
            "Cancel dictation",
            IconName::X,
            color::ROUND_CANCEL,
            color::ROUND_CANCEL_HOVER,
            color::TEXT_PRIMARY,
        ),
        Round::Finish => (
            "finish",
            "Finish dictation",
            IconName::Check,
            color::ROUND_FINISH,
            color::ROUND_FINISH_HOVER,
            color::ON_ROUND_FINISH,
        ),
        Round::Undo => (
            "undo",
            UNDO_LABEL,
            IconName::Undo2,
            color::ROUND_CANCEL,
            color::ROUND_CANCEL_HOVER,
            color::TEXT_PRIMARY,
        ),
        Round::History => (
            "open-history",
            HISTORY_LABEL,
            IconName::Clock,
            color::ROUND_CANCEL,
            color::ROUND_CANCEL_HOVER,
            color::TEXT_PRIMARY,
        ),
    };
    let mut button = div()
        .id(hook::id("flowbar", id))
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .size(px(ROUND_BUTTON))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::FULL))
        .bg(rgb(fill))
        .child(
            svg()
                .path(name.path())
                .size(px(size::ROUND_GLYPH))
                .flex_none()
                .text_color(rgb(glyph)),
        )
        // A press on the button is not a press on the bar: no drag and no toggle.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if let Some(bar) = bar {
        let bar = bar.clone();
        if matches!(kind, Round::Undo | Round::History) {
            // The pop-up is only as big as the pill, so a tooltip would be cut off: the
            // pill's own text names the button while the pointer is on it.
            let hovered = bar.clone();
            button = button.on_hover(move |inside: &bool, _: &mut Window, cx: &mut App| {
                hovered.update(cx, |bar, cx| bar.hover_button(*inside, cx));
            });
        }
        button = button.hover(move |style| style.bg(rgb(hover))).on_click(
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                bar.update(cx, |bar, cx| match kind {
                    Round::Cancel => bar.cancel(cx),
                    Round::Finish => bar.finish(cx),
                    Round::Undo => bar.press_undo(cx),
                    Round::History => bar.open_history(cx),
                });
            },
        );
    }
    button
}

/// Draws fixed props with no controls. For the pixel tests.
#[cfg(any(test, feature = "pixel-tests"))]
pub struct Preview(pub Props);

#[cfg(any(test, feature = "pixel-tests"))]
impl gpui_kit::Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        content(&self.0, None, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_state_has_its_own_size_and_every_pill_but_idle_is_one_height() {
        let mut props = Props::new(BarState::Idle);
        assert_eq!(size_of(&props), IDLE_SIZE);
        for (state, size) in [
            (BarState::Listening, LISTENING_SIZE),
            (BarState::Transcribing, TRANSCRIBING_SIZE),
            (BarState::Result, RESULT_SIZE),
            (BarState::Undo, UNDO_SIZE),
        ] {
            props.state = state;
            assert_eq!(size_of(&props), size);
            assert_eq!(size.1, HEIGHT, "{state:?}");
        }
    }

    #[test]
    fn a_notice_is_as_wide_as_its_line_and_never_wider_than_the_widest() {
        let mut props = Props::new(BarState::Blocked);
        props.message = "Select text first".into();
        let short = size_of(&props);
        assert!(short.0 < 200.0, "{short:?}");
        assert_eq!(short.1, HEIGHT);
        props.open_history = true;
        assert!(size_of(&props).0 > short.0, "room for the button");
        props.message = "x".repeat(200);
        assert_eq!(size_of(&props), NOTICE_SIZE);
    }

    #[test]
    fn the_listening_pill_fits_the_cross_the_wave_and_the_tick() {
        let wave = WAVE_BARS as f32 * BAR_WIDTH + (WAVE_BARS - 1) as f32 * BAR_WIDTH;
        let inner = 2.0 * 5.0 + 2.0 * ROUND_BUTTON + 2.0 * space::SM + wave;
        assert!(inner <= LISTENING_SIZE.0, "{inner}");
        assert!(ROUND_BUTTON < LISTENING_SIZE.1);
    }

    #[test]
    fn the_newest_level_is_in_the_middle_and_older_ones_spread_out() {
        let mut meter = vec![0.0; 32];
        meter[31] = 1.0;
        meter[30] = 0.5;
        let goal = wave_goal(&meter);
        assert_eq!(goal.len(), WAVE_BARS);
        let center = WAVE_BARS / 2;
        assert_eq!(goal[center], 1.0);
        assert_eq!(goal[center - 1], goal[center + 1]);
        assert!(goal[center - 1] > 0.4 && goal[center - 1] < 0.5);
        assert_eq!(goal[0], 0.0);
        assert_eq!(wave_goal(&[]), vec![0.0; WAVE_BARS]);
    }

    #[test]
    fn the_processing_wave_fits_the_listening_pill_and_only_the_model_wait_has_text() {
        assert!(working::width() + 2.0 * size::INSET <= TRANSCRIBING_SIZE.0);
        let mut props = Props::new(BarState::Transcribing);
        assert_eq!(size_of(&props), LISTENING_SIZE);
        assert_eq!(props.working_text(), "Transcribing");
        props.stage = Stage::Preparing;
        assert_eq!(size_of(&props), PREPARING_SIZE);
        assert_eq!(props.working_text(), "Preparing the AI model");
    }

    #[test]
    fn failures_have_short_lines() {
        assert_eq!(failure_text(Some("ENGINE_NO_SPEECH")), "No speech heard");
        assert_eq!(
            failure_text(Some("INSERT_KEYBOARD_GRABBED")),
            "Another app holds the keyboard"
        );
        assert_eq!(failure_text(Some("ENGINE_CRASHED")), "Transcription failed");
        assert_eq!(failure_text(None), "Transcription failed");
        assert_eq!(result_text(true), "Copied");
        assert_eq!(result_text(false), "Pasted");
    }
}
