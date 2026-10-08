//! What the flow bar draws. [`Props`] holds everything a frame needs, so the pixel tests draw
//! each state from plain data and the live bar builds the same props from the pipeline.

use super::FlowBar;
use crate::hook;
use crate::theme::{self, BODY_SM, LABEL_CAPS, LABEL_MD, StyledType, color, radius, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Div, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
};
use hushpen_core::flow_bar::{BarState, bar_heights};
use hushpen_core::language;

pub const IDLE_SIZE: (f32, f32) = (128.0, 28.0);
pub const LISTENING_SIZE: (f32, f32) = (160.0, 32.0);
pub const TRANSCRIBING_SIZE: (f32, f32) = (148.0, 32.0);
pub const RESULT_SIZE: (f32, f32) = (124.0, 32.0);
pub const NOTICE_SIZE: (f32, f32) = (380.0, 40.0);
const PICKER_WIDTH: f32 = 224.0;
const OPTION_HEIGHT: f32 = 28.0;
const LIST_MAX_HEIGHT: f32 = 168.0;
/// Two lines of the reason that the language list is off.
const REASON_HEIGHT: f32 = 56.0;

const WAVE_REST: f32 = 3.0;
const WAVE_MAX: f32 = 20.0;
const BAR_WIDTH: f32 = 2.0;
const IDLE_BARS: usize = 14;

/// The language list or the reason it is off. `below` puts it under the pill, for a bar in
/// the upper half of the screen.
#[derive(Debug, Clone, PartialEq)]
pub enum Picker {
    Closed,
    List {
        codes: Vec<&'static str>,
        selected: String,
        below: bool,
    },
    Reason {
        text: String,
        below: bool,
    },
}

impl Picker {
    fn below(&self) -> bool {
        match self {
            Picker::Closed => false,
            Picker::List { below, .. } | Picker::Reason { below, .. } => *below,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Language {
    /// `AUTO`, `EN`, `ES`.
    pub label: String,
    pub enabled: bool,
    /// Why the picker is off; the tree shows it as the text of the picker button.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Props {
    pub state: BarState,
    /// The newest meter values, oldest first. Only the listening state draws them.
    pub levels: Vec<f32>,
    /// Which of the three dots of the transcribing state is lit.
    pub dot: usize,
    pub language: Language,
    pub picker: Picker,
    /// The text of the result, error, and blocked states.
    pub message: String,
    pub open_history: bool,
}

impl Props {
    pub fn new(state: BarState) -> Self {
        Self {
            state,
            levels: Vec::new(),
            dot: 0,
            language: Language {
                label: "AUTO".into(),
                enabled: true,
                reason: None,
            },
            picker: Picker::Closed,
            message: String::new(),
            open_history: false,
        }
    }
}

/// The window size for these props.
pub fn size_of(props: &Props) -> (f32, f32) {
    let pill = match props.state {
        BarState::Idle => IDLE_SIZE,
        BarState::Listening => LISTENING_SIZE,
        BarState::Transcribing => TRANSCRIBING_SIZE,
        BarState::Result => RESULT_SIZE,
        BarState::Error | BarState::Blocked => NOTICE_SIZE,
    };
    match &props.picker {
        Picker::Closed => pill,
        Picker::List { codes, .. } => (PICKER_WIDTH, IDLE_SIZE.1 + list_height(codes.len())),
        Picker::Reason { .. } => (PICKER_WIDTH, IDLE_SIZE.1 + REASON_HEIGHT),
    }
}

fn list_height(rows: usize) -> f32 {
    (rows as f32 * OPTION_HEIGHT + 8.0).min(LIST_MAX_HEIGHT)
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
    let (width, height) = size_of(props);
    let card = props.picker != Picker::Closed;
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
        .flex_col()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE_ELEVATED))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .rounded(px(if card { radius::MD } else { radius::FULL }))
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
    let below = props.picker.below();
    let menu = match &props.picker {
        Picker::Closed => None,
        Picker::List {
            codes, selected, ..
        } => Some(option_list(codes, selected, below, bar).into_any_element()),
        Picker::Reason { text, .. } => Some(
            div()
                .id(hook::id("flowbar", "language.reason"))
                .test_support()
                .role(Role::Status)
                .aria_label(text.clone())
                .h(px(REASON_HEIGHT))
                .px(px(space::MD))
                .py(px(space::SM))
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(color::TEXT_MUTED))
                .child(text.clone())
                .into_any_element(),
        ),
    };
    let pill = pill_row(props, bar, card);
    if below {
        root.child(pill).children(menu)
    } else {
        root.children(menu).child(pill)
    }
}

fn pill_row(props: &Props, bar: Option<&Entity<FlowBar>>, card: bool) -> Div {
    // In the card the pill row is always the idle pill, whatever the pipeline is doing.
    let state = if card { BarState::Idle } else { props.state };
    let base = div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .px(px(space::LG - 4.0));
    match state {
        BarState::Idle => base
            .gap(px(space::SM))
            .px(px(10.0))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(3.0))
                    .children((0..IDLE_BARS).map(|_| wave_bar(WAVE_REST, color::TEXT_SUBTLE))),
            )
            .child(language_chip(&props.language, bar)),
        BarState::Listening => {
            let heights = bar_heights(&props.levels_padded(), WAVE_REST, WAVE_MAX);
            base.justify_center()
                .gap(px(BAR_WIDTH))
                .children(heights.into_iter().map(|height| {
                    let tone = if height > WAVE_REST {
                        color::TEXT_PRIMARY
                    } else {
                        color::TEXT_SUBTLE
                    };
                    wave_bar(height, tone)
                }))
        }
        BarState::Transcribing => base
            .justify_center()
            .gap(px(space::SM))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::XS))
                    .children((0..3).map(|index| dot(index == props.dot))),
            )
            .child(status_text("Transcribing", color::TEXT_BODY)),
        BarState::Result => base
            .justify_center()
            .gap(px(space::SM))
            .child(icon(IconName::Check, color::TEXT_PRIMARY))
            .child(status_text(&props.message, color::TEXT_PRIMARY)),
        BarState::Error | BarState::Blocked => {
            let icon_name = if state == BarState::Error {
                IconName::CircleAlert
            } else {
                IconName::Info
            };
            let mut notice = base
                .gap(px(space::SM))
                .child(icon(icon_name, tone(state)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(status_text(&props.message, color::TEXT_BODY)),
                );
            if props.open_history {
                notice = notice.child(open_history_button(bar));
            }
            notice
        }
    }
}

impl Props {
    /// The meter values padded at the front with rest, so the waveform has a bar for every slot.
    fn levels_padded(&self) -> Vec<f32> {
        const SLOTS: usize = 32;
        let mut padded = vec![0.0; SLOTS.saturating_sub(self.levels.len())];
        let skip = self.levels.len().saturating_sub(SLOTS);
        padded.extend(self.levels.iter().skip(skip).copied());
        padded
    }
}

fn wave_bar(height: f32, tone: u32) -> Div {
    div()
        .w(px(BAR_WIDTH))
        .h(px(height))
        .flex_none()
        .rounded(px(BAR_WIDTH / 2.0))
        .bg(theme::rgb_of(tone))
}

fn dot(lit: bool) -> Div {
    div()
        .size(px(6.0))
        .flex_none()
        .rounded(px(radius::FULL))
        .bg(theme::rgb_of(if lit {
            color::TEXT_PRIMARY
        } else {
            color::TEXT_SUBTLE
        }))
}

fn icon(name: IconName, tone: u32) -> impl IntoElement {
    svg()
        .path(name.path())
        .size(px(14.0))
        .flex_none()
        .text_color(theme::rgb_of(tone))
}

fn status_text(text: &str, tone: u32) -> impl IntoElement {
    div()
        .id(hook::id("flowbar", "status"))
        .test_support()
        .role(Role::Status)
        .aria_label(text.to_owned())
        .truncate()
        .text_token(BODY_SM)
        .text_color(theme::rgb_of(tone))
        .child(text.to_owned())
}

fn language_chip(language: &Language, bar: Option<&Entity<FlowBar>>) -> impl IntoElement {
    let id = hook::id("flowbar", "language");
    hook::mark_disabled(&id, !language.enabled);
    let label = language
        .reason
        .clone()
        .unwrap_or_else(|| language.label.clone());
    let tone = if language.enabled {
        color::TEXT_BODY
    } else {
        color::TEXT_DISABLED
    };
    let mut chip = div()
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(radius::FULL))
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_CAPS)
        .text_color(theme::rgb_of(tone))
        .child(language.label.clone())
        // A press on the chip is not a press on the bar: no drag and no dictation.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if let Some(bar) = bar {
        let bar = bar.clone();
        chip = chip
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                bar.update(cx, |bar, cx| bar.toggle_picker(cx));
            });
    }
    chip
}

fn open_history_button(bar: Option<&Entity<FlowBar>>) -> impl IntoElement {
    let mut button = div()
        .id(hook::id("flowbar", "open-history"))
        .test_support()
        .role(Role::Button)
        .aria_label("Open history")
        .flex_none()
        .flex()
        .items_center()
        .h(px(24.0))
        .px(px(10.0))
        .rounded(px(radius::FULL))
        .border_1()
        .border_color(theme::rgb_of(color::BORDER_CONTROL))
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
        .child("Open history")
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if let Some(bar) = bar {
        let bar = bar.clone();
        button = button
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                bar.update(cx, |bar, cx| bar.open_history(cx));
            });
    }
    button
}

fn option_list(
    codes: &[&'static str],
    selected: &str,
    below: bool,
    bar: Option<&Entity<FlowBar>>,
) -> impl IntoElement {
    let list = div()
        .id(hook::id("flowbar", "language.list"))
        .test_support()
        .role(Role::ListBox)
        .flex()
        .flex_col()
        .h(px(list_height(codes.len())))
        .flex_none()
        .overflow_y_scroll()
        .p(px(space::XS))
        .border_color(theme::rgb_of(color::DIVIDER));
    let list = if below {
        list.border_t_1()
    } else {
        list.border_b_1()
    };
    list.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .children(
            codes
                .iter()
                .map(|code| option_row(code, *code == selected, bar)),
        )
}

fn option_row(
    code: &'static str,
    selected: bool,
    bar: Option<&Entity<FlowBar>>,
) -> impl IntoElement {
    let name = language::label(code);
    let mut row = div()
        .id(hook::id("flowbar", &format!("language.option.{code}")))
        .test_support()
        .role(Role::ListBoxOption)
        .aria_label(name)
        .aria_selected(selected)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .h(px(OPTION_HEIGHT))
        .px(px(space::SM))
        .rounded(px(radius::CONTROL))
        .border_2()
        .border_color(transparent_black())
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
        .child(name);
    if selected {
        row = row.bg(theme::rgb_of(color::SURFACE_SELECTED)).child(
            svg()
                .path(IconName::Check.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::FOCUS)),
        );
    }
    if let Some(bar) = bar {
        let bar = bar.clone();
        row = row
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
            .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                bar.update(cx, |bar, cx| {
                    if let Err(reason) = bar.choose_language(code, cx) {
                        log::warn!("the flow bar could not set the language: {reason}");
                    }
                });
            });
    }
    row
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
    fn each_state_has_its_own_size_and_the_card_is_taller() {
        let mut props = Props::new(BarState::Idle);
        assert_eq!(size_of(&props), IDLE_SIZE);
        props.state = BarState::Listening;
        assert_eq!(size_of(&props), LISTENING_SIZE);
        props.state = BarState::Error;
        assert_eq!(size_of(&props), NOTICE_SIZE);
        props.state = BarState::Idle;
        props.picker = Picker::List {
            codes: vec!["auto"; 20],
            selected: "auto".into(),
            below: false,
        };
        let (width, height) = size_of(&props);
        assert_eq!(width, PICKER_WIDTH);
        assert_eq!(height, IDLE_SIZE.1 + LIST_MAX_HEIGHT);
    }

    #[test]
    fn the_idle_center_is_not_on_the_language_chip() {
        // The hook clicks the center of the bar, which must start a dictation.
        let pill_center = IDLE_SIZE.0 / 2.0;
        let chip_start = IDLE_SIZE.0 - 8.0 - 40.0;
        assert!(pill_center < chip_start);
    }

    #[test]
    fn the_waveform_pads_with_rest_and_keeps_the_newest_values() {
        let mut props = Props::new(BarState::Listening);
        props.levels = vec![0.5; 3];
        let padded = props.levels_padded();
        assert_eq!(padded.len(), 32);
        assert_eq!(&padded[..29], &[0.0; 29]);
        assert_eq!(&padded[29..], &[0.5; 3]);
        props.levels = (0..40).map(|n| n as f32 / 40.0).collect();
        let padded = props.levels_padded();
        assert_eq!(padded.len(), 32);
        assert_eq!(padded[31], 39.0 / 40.0);
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
