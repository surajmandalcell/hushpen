//! The controls of a row: the push button, the switch, the pop-up button, the text field, and
//! the progress track.

use super::glyph;
use crate::hook;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable, Size};
use gpui_kit::gpui::accesskit::Toggled;
use gpui_kit::{
    App, ClickEvent, Div, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The accent fill: the one action of the page or row.
    Default,
    /// A neutral fill.
    Plain,
}

/// A push button, 22 px high. A disabled one keeps its focus handle so focus never falls out of
/// the window when the button turns off under the user's finger.
pub fn button<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    kind: Kind,
    focus: Option<&FocusHandle>,
    enabled: bool,
    press: F,
) -> impl IntoElement + use<F> {
    hook::mark_disabled(&id, !enabled);
    let (fill, text, hover) = match (kind, enabled) {
        (Kind::Default, true) => (
            palette.accent,
            palette.on_accent,
            theme::faded(palette.accent, 0.85),
        ),
        (Kind::Plain, true) => (palette.control, palette.label, palette.selection),
        (_, false) => (palette.control, palette.label_tertiary, palette.control),
    };
    // The ring is inside the 22 px so the button never shifts. On the accent fill it is the
    // text color, because an accent ring would not show.
    let ring = match kind {
        Kind::Default => palette.on_accent,
        Kind::Plain => palette.accent,
    };
    let mut element = div()
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .whitespace_nowrap()
        .h(px(size::BUTTON_HEIGHT))
        .px(px(size::BUTTON_PADDING - size::FOCUS_RING))
        .rounded(px(radius::BUTTON))
        .border_2()
        .border_color(gpui_kit::transparent_black())
        .bg(fill)
        .text_token(BUTTON)
        .text_color(text);
    if let Some(focus) = focus {
        element = element
            .track_focus(focus)
            .focus_visible(move |style| style.border_color(ring));
    }
    if enabled {
        let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| press(window, cx);
        element = element
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_click(on_click);
    }
    element.child(label)
}

/// A switch with its label for the accessibility tree. The whole control, ring included, is the
/// element with `id`. The track is the accent while it is on.
pub fn switch<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    on: bool,
    enabled: bool,
    focus: &FocusHandle,
    toggle: F,
) -> impl IntoElement + use<F> {
    hook::mark_disabled(&id, !enabled);
    let knob = div()
        .size(px(size::SWITCH_KNOB))
        .flex_none()
        .rounded(px(radius::FULL))
        .bg(palette.on_accent);
    let track = div()
        .w(px(size::SWITCH_WIDTH))
        .h(px(size::SWITCH_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .px(px((size::SWITCH_HEIGHT - size::SWITCH_KNOB) / 2.0))
        .rounded(px(radius::FULL))
        .bg(if on {
            palette.accent
        } else {
            palette.label_tertiary
        });
    let track = if on {
        track.justify_end().child(knob)
    } else {
        track.justify_start().child(knob)
    };
    let mut element = div()
        .id(id)
        .test_support()
        .role(Role::Switch)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .aria_label(format!("{label}, {}", if on { "on" } else { "off" }))
        .flex_none()
        .flex()
        .items_center()
        .p(px(space::XXS))
        .rounded(px(radius::FULL))
        .border_2()
        .border_color(gpui_kit::transparent_black())
        .opacity(if enabled { 1.0 } else { 0.5 })
        .child(track);
    if enabled {
        let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| toggle(window, cx);
        element = element
            .track_focus(focus)
            .cursor_pointer()
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_click(on_click);
    }
    element
}

/// A pop-up button: the current value and a chevron. It opens the list of options the view
/// draws under it.
pub fn menu_button<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: &str,
    current: String,
    enabled: bool,
    focus: &FocusHandle,
    toggle: F,
) -> impl IntoElement + use<F> {
    hook::mark_disabled(&id, !enabled);
    let mut element = div()
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label(format!("{label}: {current}"))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::XS))
        .max_w(px(size::MENU_MAX_WIDTH))
        .h(px(size::BUTTON_HEIGHT))
        .px(px(size::BUTTON_PADDING - size::FOCUS_RING))
        .rounded(px(radius::BUTTON))
        .border_2()
        .border_color(gpui_kit::transparent_black())
        .bg(palette.control)
        .text_token(BUTTON)
        .text_color(if enabled {
            palette.label
        } else {
            palette.label_tertiary
        })
        .child(div().min_w_0().truncate().child(current))
        .child(glyph(palette, IconName::ChevronDown));
    if enabled {
        let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| toggle(window, cx);
        element = element
            .track_focus(focus)
            .cursor_pointer()
            .hover(move |style| style.bg(palette.selection))
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_click(on_click);
    }
    element
}

/// A text field on the control fill, as high as a push button so one row of both aligns. Its
/// stroke shows only under keyboard focus, as the ring. The small kit padding (2 px) leaves the
/// 13 px text 18 px inside the 22 px.
pub fn field(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    state: &Entity<InputState>,
) -> Input {
    // Input's own `h` method sets a height that only a multi-line field reads, so the
    // single-line height goes in through the style, where it wins over the size default.
    let mut height = gpui_kit::StyleRefinement::default();
    height.size.height = Some(px(size::BUTTON_HEIGHT).into());
    Input::new(state)
        .id(id)
        .aria_label(label)
        .with_size(Size::Small)
        .refine_style(&height)
        .bg(palette.control)
        .rounded(px(radius::BUTTON))
        .border_color(gpui_kit::transparent_black())
}

/// A thin bar filled from the left to `fraction` (0 to 1): a download or a play position.
pub fn track(palette: Palette, fraction: f32) -> Div {
    div()
        .w_full()
        .h(px(size::TRACK_HEIGHT))
        .rounded(px(radius::FULL))
        .bg(palette.control)
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0.0, 1.0)))
                .rounded(px(radius::FULL))
                .bg(palette.label_secondary),
        )
}
