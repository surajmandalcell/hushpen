//! What the flow bar draws. [`Props`] holds everything a frame needs, so the pixel tests draw
//! each state from plain data and the live bar builds the same props from the pipeline.
//!
//! Every pill is balanced: it has a round circle at both ends or at neither, and its line sits
//! in the middle of the room between them, or of the whole pill. A pill is as wide as its
//! content with fixed insets: 4 pt edge, 20 pt circle, and 8 pt gap at each end, or 12 pt at
//! each end of a pill without circles.

use super::FlowBar;
use super::working::{self, BEAD_GAP, BEAD_WIDTH, Mix, Stage};
use crate::hook;
use crate::theme::hud::{BODY_SM, color, radius, size};
use crate::theme::{FONT_FAMILY, StyledType, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, TextRun,
    Window, WindowTextSystem, div, font, px, rgb, svg,
};
pub use hushpen_core::flow_bar::CANCELLED_TEXT;
use hushpen_core::flow_bar::{BarState, bar_heights};

const HEIGHT: f32 = 28.0;
pub const IDLE_SIZE: (f32, f32) = (67.0, 22.0);
/// Cancel, the waveform, and finish. The processing wave keeps this width, so the pill does
/// not move when the words stop.
pub const LISTENING_SIZE: (f32, f32) = (118.0, HEIGHT);
pub const TRANSCRIBING_SIZE: (f32, f32) = LISTENING_SIZE;
/// The widest pill. A longer line truncates.
pub const MAX_WIDTH: f32 = 366.0;
/// One end of a pill with circles: the edge, the circle, and the gap to the line.
pub const ROUND_END: f32 = size::BUTTON_INSET + ROUND_BUTTON + space::SM;

/// The bars of the listening waveform. The processing wave has the same bars in the same
/// places, so one turns into the other where it stands.
pub const WAVE_BARS: usize = 11;
const IDLE_BARS: usize = 9;
const WAVE_REST: f32 = 3.0;
const WAVE_MAX: f32 = 18.0;
const BAR_WIDTH: f32 = 2.0;
const _: () = assert!(working::BEADS == WAVE_BARS && BEAD_WIDTH == BAR_WIDTH);
const _: () = assert!(BEAD_GAP == BAR_WIDTH);
const ROUND_BUTTON: f32 = 20.0;
/// A quiet listening bar: the muted text color is the primary one at this opacity on the pill.
const QUIET: f32 = (0xA8 - 0x1F) as f32 / (0xEE - 0x1F) as f32;
/// How much lower the first, second, and third bar from each end of a wave stand, so the wave
/// tapers into the round ends.
const PINCH: [f32; 3] = [0.70, 0.85, 0.95];
pub const UNDO_LABEL: &str = "Undo paste";
/// What the undo pill says while the pointer is on its button. It is no longer than "Pasted",
/// so the pill keeps its size under the pointer.
pub const UNDO_HOVER: &str = "Undo";
pub const HISTORY_LABEL: &str = "Open history";
pub const RECOVER_LABEL: &str = "Recover";

/// One bar of a waveform on one frame: its height, how far it sits above the middle line, and
/// its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveBar {
    pub height: f32,
    pub lift: f32,
    pub alpha: f32,
}

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
    /// The text of the result, undo, cancelled, error, and blocked states.
    pub message: String,
    pub open_history: bool,
    /// The pointer is on the round button of the undo, cancelled, or notice pill: the text
    /// names what it does, because the pop-up is too small for a tooltip.
    pub hover: bool,
    /// The width of the widest line the pill can show, from [`measure`].
    pub text_width: f32,
    /// The window size on this frame while the bar moves; `None` is the size of the state.
    pub frame: Option<(f32, f32)>,
    /// The opacity of the whole bar: 0 is gone, 1 is fully shown.
    pub presence: f32,
    /// The scale of the pill in its window while it comes and goes.
    pub scale: f32,
    /// The opacity of the content of the state.
    pub fade: f32,
    /// The content of the state before, while it fades out, and its opacity.
    pub prev: Option<Box<Props>>,
    pub prev_fade: f32,
    /// How far the marks and round buttons of the state are in (their opacity), and their
    /// scale.
    pub marks: f32,
    pub mark_scale: f32,
    /// The waveform the wave turns from, and how far it is (0 to 1).
    pub morph: Option<(Vec<WaveBar>, f32)>,
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
            text_width: 0.0,
            frame: None,
            presence: 1.0,
            scale: 1.0,
            fade: 1.0,
            prev: None,
            prev_fade: 0.0,
            marks: 1.0,
            mark_scale: 1.0,
            morph: None,
        }
    }

    /// A bar that is gone: nothing of it draws, so a window kept for the next dictation holds
    /// a clear frame.
    pub fn gone() -> Self {
        Self {
            presence: 0.0,
            fade: 0.0,
            ..Self::new(BarState::Idle)
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

    /// The lines the pill can show: the room for the line is as wide as the widest, so the
    /// pill keeps its size when the pointer changes the line.
    pub fn lines(&self) -> Vec<&str> {
        match self.state {
            BarState::Idle | BarState::Listening => Vec::new(),
            BarState::Transcribing if self.preparing() => vec![self.working_text()],
            BarState::Transcribing => Vec::new(),
            BarState::Result => vec![&self.message],
            BarState::Undo => vec![&self.message, UNDO_HOVER],
            BarState::Cancelled => vec![&self.message, RECOVER_LABEL],
            BarState::Error | BarState::Blocked if self.open_history => {
                vec![&self.message, HISTORY_LABEL]
            }
            BarState::Error | BarState::Blocked => vec![&self.message],
        }
    }

    /// The levels with one entry for every waveform bar: missing bars rest.
    fn levels_padded(&self) -> Vec<f32> {
        let mut levels = self.levels.clone();
        levels.resize(WAVE_BARS, 0.0);
        levels
    }
}

/// Sets `text_width` from the font the pill draws its line in.
pub fn measure(props: &mut Props, text: &WindowTextSystem) {
    let font = gpui_kit::Font {
        weight: FontWeight(BODY_SM.weight),
        ..font(FONT_FAMILY)
    };
    props.text_width = props
        .lines()
        .into_iter()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let run = TextRun {
                len: line.len(),
                font: font.clone(),
                color: gpui_kit::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(text.layout_line(line, px(BODY_SM.size), &[run], None).width)
        })
        .fold(0.0, f32::max);
}

/// [`measure`] without a window, for a frame that is sized before its window opens.
pub fn measure_in(props: &mut Props, cx: &App) {
    measure(props, &WindowTextSystem::new(cx.text_system().clone()));
}

/// The size the bar settles at for these props.
pub fn size_of(props: &Props) -> (f32, f32) {
    let line = props.text_width;
    let width = match props.state {
        BarState::Idle => return IDLE_SIZE,
        BarState::Listening => LISTENING_SIZE.0,
        BarState::Transcribing if props.preparing() => {
            2.0 * size::INSET + working::width() + space::SM + line
        }
        BarState::Transcribing => TRANSCRIBING_SIZE.0,
        BarState::Undo | BarState::Cancelled => 2.0 * ROUND_END + line,
        BarState::Error | BarState::Blocked if props.open_history => 2.0 * ROUND_END + line,
        BarState::Result | BarState::Error | BarState::Blocked => 2.0 * size::INSET + line,
    };
    (width.ceil().min(MAX_WIDTH), HEIGHT)
}

/// How wide the line may be in the settled pill: wider lines truncate.
fn line_room(props: &Props) -> f32 {
    let (width, _) = size_of(props);
    let ends = match props.state {
        BarState::Undo | BarState::Cancelled => 2.0 * ROUND_END,
        BarState::Error | BarState::Blocked if props.open_history => 2.0 * ROUND_END,
        BarState::Transcribing => 2.0 * size::INSET + working::width() + space::SM,
        _ => 2.0 * size::INSET,
    };
    (width - ends).max(0.0)
}

/// How much of its height bar `index` of `count` keeps: the three bars nearest each end stand
/// a little lower.
pub fn pinch(index: usize, count: usize) -> f32 {
    let from_end = index.min(count.saturating_sub(index + 1));
    PINCH.get(from_end).copied().unwrap_or(1.0)
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

/// Whether the look has a waveform in the middle of the pill, which turns into the waveform
/// of the next such look where it stands.
pub fn morphs(props: &Props) -> bool {
    props.state == BarState::Listening
        || (props.state == BarState::Transcribing && !props.preparing())
}

/// The waveform of the look on its own, before any turn from another one.
fn own_wave(props: &Props) -> Option<Vec<WaveBar>> {
    match props.state {
        BarState::Listening => {
            let heights = bar_heights(&props.levels_padded(), WAVE_REST, WAVE_MAX);
            let count = heights.len();
            Some(
                heights
                    .into_iter()
                    .enumerate()
                    .map(|(index, height)| WaveBar {
                        height: (height * pinch(index, count)).max(BAR_WIDTH),
                        lift: 0.0,
                        alpha: if height > WAVE_REST + 0.5 { 1.0 } else { QUIET },
                    })
                    .collect(),
            )
        }
        BarState::Transcribing => {
            let (beads, bob) = if props.still {
                (working::still(props.stage), 0.0)
            } else {
                (
                    working::beads(props.stage, props.from, props.stage_ms, props.work_ms),
                    working::bob(props.work_ms),
                )
            };
            let count = beads.len();
            Some(
                beads
                    .into_iter()
                    .enumerate()
                    .map(|(index, bead)| {
                        let pinch = pinch(index, count);
                        WaveBar {
                            height: (bead.height * pinch).max(BEAD_WIDTH),
                            lift: bead.lift * pinch + bob,
                            alpha: bead.glow,
                        }
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// The waveform as this frame draws it, partway through a turn from another one.
pub fn drawn_wave(props: &Props) -> Option<Vec<WaveBar>> {
    let own = own_wave(props)?;
    let Some((from, done)) = props.morph.as_ref().filter(|(_, done)| *done < 1.0) else {
        return Some(own);
    };
    let mix = |a: f32, b: f32| a + (b - a) * done;
    Some(
        own.iter()
            .enumerate()
            .map(|(index, to)| {
                let from = from.get(index).copied().unwrap_or(*to);
                WaveBar {
                    height: mix(from.height, to.height),
                    lift: mix(from.lift, to.lift),
                    alpha: mix(from.alpha, to.alpha),
                }
            })
            .collect(),
    )
}

/// The whole bar. `bar` is the live entity the controls call; the pixel tests pass `None`.
pub fn content(
    props: &Props,
    bar: Option<&Entity<FlowBar>>,
    composited: bool,
) -> impl IntoElement + use<> {
    let (width, height) = props.frame.unwrap_or_else(|| size_of(props));
    // Without a compositor the window cannot be see-through, so the pill fills it.
    let scale = if composited { props.scale } else { 1.0 };
    let (width, height) = (width * scale, height * scale);
    let label: SharedString = match props.state {
        BarState::Idle => "Start dictation".into(),
        BarState::Listening => "Stop dictation".into(),
        BarState::Transcribing => props.working_text().into(),
        _ if props.message.is_empty() => props.state.key().into(),
        _ => props.message.clone().into(),
    };
    let mut pill = div()
        .id(hook::id("flowbar", "bar"))
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .w(px(width))
        .h(px(height))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .opacity(props.presence)
        .font_family(FONT_FAMILY)
        .bg(rgb(color::SURFACE_ELEVATED))
        .border(px(0.5))
        .border_color(rgb(color::BAR_EDGE))
        .cursor_pointer();
    // Without a compositor pill corners would render black, so the bar is a rectangle.
    pill = if composited {
        pill.rounded(px(radius::FULL))
    } else {
        pill.rounded_none()
    };
    if let Some(bar) = bar {
        // The bar tells a click from a drag itself, from the pointer on the screen: a pop-up
        // that moves under the pointer cannot rely on the positions it is sent.
        let (down, up, up_outside) = (bar.clone(), bar.clone(), bar.clone());
        pill = pill
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
    // The old content fades out before the new one fades in, so only one row draws. A wave
    // that turns into the next wave stays, and only the circles around it come and go.
    let row = match &props.prev {
        Some(prev) if props.morph.is_none() && props.prev_fade > 0.0 => {
            pill_row(prev, None, None).opacity(props.prev_fade)
        }
        _ if props.morph.is_some() => pill_row(props, bar, props.prev.as_deref()),
        _ => pill_row(props, bar, None).opacity(props.fade),
    };
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(pill.child(row.w(px(width)).h(px(height)).flex_none()))
}

/// The content of one look. Circles stay 4 pt in from the ends of the pill as it changes size,
/// and the line stays in the middle. `ghost` is the look a wave turns from: its circles fade
/// out around the wave.
fn pill_row(props: &Props, bar: Option<&Entity<FlowBar>>, ghost: Option<&Props>) -> Div {
    let base = div().flex().items_center();
    let centered = || {
        div()
            .flex()
            .items_center()
            .px(px(size::INSET))
            .justify_center()
    };
    let room = line_room(props);
    let appear = Appear {
        alpha: props.marks,
        scale: props.mark_scale,
    };
    match props.state {
        BarState::Idle => base
            .justify_center()
            .gap(px(BAR_WIDTH))
            .children((0..IDLE_BARS).map(|_| idle_bar())),
        BarState::Listening => base
            .px(px(size::BUTTON_INSET))
            .justify_between()
            .child(round_button(Round::Cancel, bar, appear))
            .child(wave(props))
            .child(round_button(Round::Finish, bar, appear)),
        BarState::Transcribing if props.preparing() => centered()
            .gap(px(space::SM))
            .child(wave(props))
            .child(status_text(props.working_text(), color::TEXT_BODY, room)),
        BarState::Transcribing => match ghost.filter(|ghost| ghost.state == BarState::Listening) {
            Some(_) => {
                let going = Appear {
                    alpha: props.prev_fade,
                    scale: 1.0,
                };
                base.px(px(size::BUTTON_INSET))
                    .justify_between()
                    .child(ghost_circle(Round::Cancel, going))
                    .child(wave(props))
                    .child(ghost_circle(Round::Finish, going))
            }
            None => centered().child(wave(props)),
        },
        BarState::Result => {
            centered().child(status_text(&props.message, color::TEXT_PRIMARY, room))
        }
        BarState::Undo => {
            let line = if props.hover {
                UNDO_HOVER
            } else {
                &props.message
            };
            balanced(
                Mark::Done,
                &props.message,
                (line, color::TEXT_PRIMARY, room),
                Round::Undo,
                bar,
                appear,
            )
        }
        BarState::Cancelled => {
            let line = if props.hover {
                (RECOVER_LABEL, color::TEXT_PRIMARY, room)
            } else {
                (props.message.as_str(), color::TEXT_BODY, room)
            };
            balanced(
                Mark::Cancelled,
                CANCELLED_TEXT,
                line,
                Round::Recover,
                bar,
                appear,
            )
        }
        BarState::Error | BarState::Blocked if props.open_history => {
            let line = if props.hover {
                (HISTORY_LABEL, color::TEXT_PRIMARY, room)
            } else {
                (props.message.as_str(), color::TEXT_BODY, room)
            };
            let (mark, said) = if props.state == BarState::Error {
                (Mark::Error, "Error")
            } else {
                (Mark::Blocked, "Not pasted")
            };
            balanced(mark, said, line, Round::History, bar, appear)
        }
        BarState::Error | BarState::Blocked => {
            centered().child(status_text(&props.message, color::TEXT_BODY, room))
        }
    }
}

/// A status mark at the left end, the line in the middle of the room between the two circles,
/// and one round button at the right end.
fn balanced(
    mark: Mark,
    said: &str,
    (line, tone, room): (&str, u32, f32),
    round: Round,
    bar: Option<&Entity<FlowBar>>,
    appear: Appear,
) -> Div {
    div()
        .flex()
        .items_center()
        .px(px(size::BUTTON_INSET))
        .gap(px(space::SM))
        .child(status_mark(mark, said, appear))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .child(status_text(line, tone, room)),
        )
        .child(round_button(round, bar, appear))
}

/// The waveform in the middle of the pill: the listening level meter or the processing wave,
/// or one turning into the other.
fn wave(props: &Props) -> impl IntoElement {
    let bars = drawn_wave(props).unwrap_or_default();
    div()
        .id(hook::id("flowbar", "wave"))
        .test_support()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(working::width()))
        .h(px(HEIGHT))
        .gap(px(BAR_WIDTH))
        .children(bars.into_iter().map(|bar| {
            div()
                .w(px(BAR_WIDTH))
                .h(px(bar.height))
                .flex_none()
                .relative()
                .top(px(-bar.lift))
                .rounded(px(BAR_WIDTH / 2.0))
                .bg(rgb(color::TEXT_PRIMARY))
                .opacity(bar.alpha)
        }))
}

fn idle_bar() -> Div {
    div()
        .w(px(BAR_WIDTH))
        .h(px(WAVE_REST))
        .flex_none()
        .rounded(px(BAR_WIDTH / 2.0))
        .bg(rgb(color::TEXT_SUBTLE))
}

fn glyph(name: IconName, tone: u32, scale: f32) -> AnyElement {
    svg()
        .path(name.path())
        .size(px(size::ROUND_GLYPH * scale))
        .flex_none()
        .text_color(rgb(tone))
        .into_any_element()
}

/// The "!" of an error or blocked mark. The icon set has no bare one, only one in a circle,
/// which would be a circle in a circle here.
fn exclamation(tone: u32) -> AnyElement {
    let stroke = 1.5;
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(stroke))
        .child(
            div()
                .w(px(stroke))
                .h(px(5.0))
                .rounded(px(stroke / 2.0))
                .bg(rgb(tone)),
        )
        .child(
            div()
                .size(px(stroke))
                .rounded(px(stroke / 2.0))
                .bg(rgb(tone)),
        )
        .into_any_element()
}

/// The line, never wider than `room`: a longer one truncates.
fn status_text(text: &str, tone: u32, room: f32) -> impl IntoElement {
    div()
        .id(hook::id("flowbar", "status"))
        .test_support()
        .role(Role::Status)
        .aria_label(text.to_owned())
        .flex_none()
        .max_w(px(room))
        .truncate()
        .text_token(BODY_SM)
        .text_color(rgb(tone))
        .child(text.to_owned())
}

/// How far a mark or a round button is in: its opacity and its scale.
#[derive(Clone, Copy)]
struct Appear {
    alpha: f32,
    scale: f32,
}

/// The 20 pt place of a circle. The circle grows inside it, so nothing around it moves.
fn circle_place(appear: Appear) -> Div {
    div()
        .size(px(ROUND_BUTTON))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .opacity(appear.alpha)
}

fn circle(fill: u32, scale: f32) -> Div {
    div()
        .size(px(ROUND_BUTTON * scale))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::FULL))
        .bg(rgb(fill))
}

#[derive(Clone, Copy)]
enum Mark {
    Done,
    Cancelled,
    Error,
    Blocked,
}

/// The round mark at the left end of a pill that has a button at the right end, so both ends
/// match. It only shows a status: no hover, no press, no pointer, and it takes no focus.
fn status_mark(mark: Mark, said: &str, appear: Appear) -> impl IntoElement {
    let (fill, inner) = match mark {
        Mark::Done => (
            color::MARK_DONE,
            glyph(IconName::Check, color::STATUS_SUCCESS, appear.scale),
        ),
        Mark::Cancelled => (
            color::MARK_CANCELLED,
            glyph(IconName::X, color::TEXT_MUTED, appear.scale),
        ),
        Mark::Error => (color::MARK_ERROR, exclamation(color::STATUS_ERROR)),
        Mark::Blocked => (color::MARK_BLOCKED, exclamation(color::STATUS_WARNING)),
    };
    circle_place(appear)
        .id(hook::id("flowbar", "mark"))
        .test_support()
        .role(Role::Image)
        .aria_label(said.to_owned())
        .cursor_default()
        // A press on the mark is not a press on the bar either: the mark does nothing.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(circle(fill, appear.scale).child(inner))
}

#[derive(Clone, Copy)]
enum Round {
    Cancel,
    Finish,
    Undo,
    Recover,
    History,
}

impl Round {
    fn look(self) -> (&'static str, &'static str, IconName, u32, u32, u32) {
        match self {
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
            Round::Recover => (
                "recover",
                RECOVER_LABEL,
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
        }
    }
}

/// A circle of a look that is going: it only draws, and takes no press.
fn ghost_circle(kind: Round, appear: Appear) -> Div {
    let (_, _, name, fill, _, tone) = kind.look();
    circle_place(appear).child(circle(fill, appear.scale).child(glyph(name, tone, appear.scale)))
}

/// The round cross and tick of the listening state, and the one button of the undo, the
/// cancelled, and the notice pills.
fn round_button(kind: Round, bar: Option<&Entity<FlowBar>>, appear: Appear) -> impl IntoElement {
    let (id, label, name, fill, hover, tone) = kind.look();
    let mut face = circle(fill, appear.scale).child(glyph(name, tone, appear.scale));
    if bar.is_some() {
        face = face.hover(move |style| style.bg(rgb(hover)));
    }
    let mut button = circle_place(appear)
        .id(hook::id("flowbar", id))
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        // A press on the button is not a press on the bar: no drag and no toggle.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(face);
    if let Some(bar) = bar {
        let bar = bar.clone();
        if matches!(kind, Round::Undo | Round::Recover | Round::History) {
            // The pop-up is only as big as the pill, so a tooltip would be cut off: the
            // pill's own text names the button while the pointer is on it.
            let hovered = bar.clone();
            button = button.on_hover(move |inside: &bool, _: &mut Window, cx: &mut App| {
                hovered.update(cx, |bar, cx| bar.hover_button(*inside, cx));
            });
        }
        button = button.on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            bar.update(cx, |bar, cx| match kind {
                Round::Cancel => bar.cancel(cx),
                Round::Finish => bar.finish(cx),
                Round::Undo => bar.press_undo(cx),
                Round::Recover => bar.press_recover(cx),
                Round::History => bar.open_history(cx),
            });
        });
    }
    button
}

/// Draws fixed props with no controls. For the pixel tests. Props that were not measured are
/// measured with the window's fonts first.
#[cfg(any(test, feature = "pixel-tests"))]
pub struct Preview(pub Props);

#[cfg(any(test, feature = "pixel-tests"))]
impl gpui_kit::Render for Preview {
    fn render(&mut self, window: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        if self.0.text_width == 0.0 && !self.0.lines().is_empty() {
            measure(&mut self.0, window.text_system());
        }
        content(&self.0, None, true)
    }
}

#[cfg(test)]
mod tests;
