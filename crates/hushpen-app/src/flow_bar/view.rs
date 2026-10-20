//! What the flow bar draws. [`Props`] holds everything a frame needs, so the pixel tests draw
//! each state from plain data and the live bar builds the same props from the pipeline.

use super::FlowBar;
use crate::hook;
use crate::theme::hud::{BODY_SM, LABEL_MD, color, radius};
use crate::theme::{StyledType, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Context, Div, Entity, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, rgb,
    svg,
};
use hushpen_core::flow_bar::{BarState, bar_heights};

const HEIGHT: f32 = 32.0;
pub const IDLE_SIZE: (f32, f32) = (64.0, 24.0);
/// Cancel, the waveform, and finish: 5 + 22 + 8 + 42 + 8 + 22 + 5.
pub const LISTENING_SIZE: (f32, f32) = (112.0, HEIGHT);
pub const TRANSCRIBING_SIZE: (f32, f32) = (136.0, HEIGHT);
pub const RESULT_SIZE: (f32, f32) = (112.0, HEIGHT);
pub const NOTICE_SIZE: (f32, f32) = (360.0, HEIGHT);
pub const UNDO_SIZE: (f32, f32) = (228.0, HEIGHT);
/// The transcribing pill while the cleanup model loads: its text is longer.
pub const PREPARING_SIZE: (f32, f32) = (208.0, HEIGHT);

/// The bars of the listening waveform.
pub const WAVE_BARS: usize = 11;
const IDLE_BARS: usize = 9;
const WAVE_REST: f32 = 3.0;
const WAVE_MAX: f32 = 18.0;
const BAR_WIDTH: f32 = 2.0;
const ROUND_BUTTON: f32 = 22.0;
/// How long one sweep of the transcribing dots takes.
pub const DOT_CYCLE_MS: u64 = 1_100;

#[derive(Debug, Clone, PartialEq)]
pub struct Props {
    pub state: BarState,
    /// The height of each waveform bar, 0 to 1. Only the listening state draws them.
    pub levels: Vec<f32>,
    /// Where the transcribing dots are in their sweep, 0 to 1.
    pub phase: f32,
    /// The text of the result, undo, error, and blocked states.
    pub message: String,
    pub open_history: bool,
    /// The cleanup model is loading while the transcribing state shows.
    pub preparing: bool,
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
            phase: 0.0,
            message: String::new(),
            open_history: false,
            preparing: false,
            frame: None,
            presence: 1.0,
            fade: 1.0,
        }
    }

    /// What the transcribing pill says.
    pub fn working_text(&self) -> &'static str {
        if self.preparing {
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
        BarState::Transcribing if props.preparing => PREPARING_SIZE,
        BarState::Transcribing => TRANSCRIBING_SIZE,
        BarState::Result => RESULT_SIZE,
        BarState::Undo => UNDO_SIZE,
        BarState::Error | BarState::Blocked => NOTICE_SIZE,
    }
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

/// The text of the result flash: what happened to the words.
pub fn result_text(copied: bool) -> &'static str {
    if copied { "Copied" } else { "Inserted" }
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
        Some(COMMAND_NO_SELECTION) => "Select text first",
        Some(COMMAND_NEEDS_AI) => "Command Mode needs AI cleanup or the endpoint",
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
pub fn content(props: &Props, bar: Option<&Entity<FlowBar>>) -> impl IntoElement + use<> {
    let (width, height) = props.frame.unwrap_or_else(|| size_of(props));
    let label: SharedString = match props.state {
        BarState::Idle => "Start dictation".into(),
        BarState::Listening => "Stop dictation".into(),
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
        .rounded(px(radius::FULL))
        .cursor_pointer();
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
    let base = div().flex().items_center().px(px(space::LG - 4.0));
    match props.state {
        BarState::Idle => base
            .justify_center()
            .gap(px(BAR_WIDTH))
            .children((0..IDLE_BARS).map(|_| wave_bar(WAVE_REST, color::TEXT_SUBTLE))),
        BarState::Listening => {
            let heights = bar_heights(&props.levels_padded(), WAVE_REST, WAVE_MAX);
            base.px(px(5.0))
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
        BarState::Transcribing => base
            .justify_center()
            .gap(px(space::SM))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::XS))
                    .children((0..3).map(|index| dot(dot_glow(props.phase, index)))),
            )
            .child(status_text(props.working_text(), color::TEXT_BODY)),
        BarState::Result => base
            .justify_center()
            .gap(px(space::SM))
            .child(icon(IconName::Check, color::TEXT_PRIMARY))
            .child(status_text(&props.message, color::TEXT_PRIMARY)),
        BarState::Undo => base
            .pr(px(4.0))
            .gap(px(space::SM))
            .child(icon(IconName::Check, color::TEXT_PRIMARY))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(status_text(&props.message, color::TEXT_PRIMARY)),
            )
            .child(text_button("undo", "Undo cleanup", bar, |bar, cx| {
                bar.press_undo(cx)
            })),
        BarState::Error | BarState::Blocked => {
            let icon_name = if props.state == BarState::Error {
                IconName::CircleAlert
            } else {
                IconName::Info
            };
            let mut notice = base
                .pr(px(4.0))
                .gap(px(space::SM))
                .child(icon(icon_name, tone(props.state)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(status_text(&props.message, color::TEXT_BODY)),
                );
            if props.open_history {
                notice = notice.child(text_button(
                    "open-history",
                    "Open history",
                    bar,
                    |bar, cx| bar.open_history(cx),
                ));
            }
            notice
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

/// How lit one of the three transcribing dots is: a soft glow that sweeps from left to right.
fn dot_glow(phase: f32, index: usize) -> f32 {
    let at = (phase - index as f32 * 0.18).rem_euclid(1.0);
    let wave = 0.5 - 0.5 * (at * std::f32::consts::TAU).cos();
    0.3 + 0.7 * wave * wave
}

fn wave_bar(height: f32, tone: u32) -> Div {
    div()
        .w(px(BAR_WIDTH))
        .h(px(height))
        .flex_none()
        .rounded(px(BAR_WIDTH / 2.0))
        .bg(rgb(tone))
}

fn dot(glow: f32) -> Div {
    div()
        .size(px(6.0))
        .flex_none()
        .rounded(px(radius::FULL))
        .bg(rgb(color::TEXT_PRIMARY))
        .opacity(glow)
}

fn icon(name: IconName, tone: u32) -> impl IntoElement {
    svg()
        .path(name.path())
        .size(px(14.0))
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
}

/// The round cross and tick of the listening state.
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
                .size(px(12.0))
                .flex_none()
                .text_color(rgb(glyph)),
        )
        // A press on the button is not a press on the bar: no drag and no toggle.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if let Some(bar) = bar {
        let bar = bar.clone();
        button = button.hover(move |style| style.bg(rgb(hover))).on_click(
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                bar.update(cx, |bar, cx| match kind {
                    Round::Cancel => bar.cancel(cx),
                    Round::Finish => bar.finish(cx),
                });
            },
        );
    }
    button
}

fn text_button(
    id: &str,
    label: &'static str,
    bar: Option<&Entity<FlowBar>>,
    press: fn(&mut FlowBar, &mut Context<FlowBar>),
) -> impl IntoElement {
    let mut button = div()
        .id(hook::id("flowbar", id))
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .h(px(24.0))
        .px(px(10.0))
        .rounded(px(radius::FULL))
        .border_1()
        .border_color(rgb(color::BORDER_CONTROL))
        .bg(rgb(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(rgb(color::TEXT_PRIMARY))
        .child(label)
        // A press on the button is not a press on the bar: no drag and no dictation.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if let Some(bar) = bar {
        let bar = bar.clone();
        button = button
            .hover(|style| style.bg(rgb(color::SURFACE_CONTROL_HOVER)))
            .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                bar.update(cx, press);
            });
    }
    button
}

/// Draws fixed props with no controls. For the pixel tests.
#[cfg(any(test, feature = "pixel-tests"))]
pub struct Preview(pub Props);

#[cfg(any(test, feature = "pixel-tests"))]
impl gpui_kit::Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        content(&self.0, None)
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
            (BarState::Error, NOTICE_SIZE),
            (BarState::Blocked, NOTICE_SIZE),
        ] {
            props.state = state;
            assert_eq!(size_of(&props), size);
            assert_eq!(size.1, HEIGHT, "{state:?}");
        }
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
    fn the_dot_glow_sweeps_and_stays_visible() {
        for step in 0..20 {
            let phase = step as f32 / 20.0;
            for index in 0..3 {
                let glow = dot_glow(phase, index);
                assert!((0.3..=1.0).contains(&glow), "{phase} {index} {glow}");
            }
        }
        assert!(dot_glow(0.5, 0) > dot_glow(0.5, 2));
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
        assert_eq!(result_text(false), "Inserted");
    }
}
