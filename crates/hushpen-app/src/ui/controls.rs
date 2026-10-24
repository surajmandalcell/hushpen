//! The controls of a row: the push button, the switch, the slider, the pop-up button, the text
//! field, and the progress track.

use super::{glyph, icon};
use crate::hook;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Sizable, Size};
use gpui_kit::gpui::accesskit::Toggled;
use gpui_kit::{
    App, Bounds, ClickEvent, Div, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use std::cell::Cell;
use std::rc::Rc;

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

/// A button that is one 16 px icon with no fill until hovered, as high as a push button. The
/// tooltip names it; `label` is its name in the accessibility tree.
pub fn icon_button<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    name: IconName,
    label: String,
    tooltip: &'static str,
    focus: &FocusHandle,
    press: F,
) -> impl IntoElement + use<F> {
    let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| press(window, cx);
    div()
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(size::BUTTON_HEIGHT))
        .rounded(px(radius::BUTTON))
        .border_2()
        .border_color(gpui_kit::transparent_black())
        .track_focus(focus)
        .focus_visible(move |style| style.border_color(palette.accent))
        .cursor_pointer()
        .hover(move |style| style.bg(palette.selection))
        .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
        .on_click(on_click)
        .child(icon(palette, name))
}

/// The ring of a switch or a slider and its gap sit this far outside the track. The control is
/// drawn this far right of its place, so its track ends on the same line as the buttons in the
/// rows around it.
const RING_OUTSET: f32 = size::FOCUS_RING + space::XXS;

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
        .relative()
        .left(px(RING_OUTSET))
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

/// How far Left or Right moves a slider: one 5% step.
const SLIDER_STEP: f32 = 0.05;

/// A slider from 0 to 1: the accent fills the track up to a round knob. A press or a drag along
/// it sets the value; Left, Right, Home, and End move it from the keyboard.
pub fn slider<F: Fn(f32, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    value: f32,
    focus: &FocusHandle,
    set: F,
) -> impl IntoElement + use<F> {
    let value = value.clamp(0.0, 1.0);
    let travel = size::SLIDER_WIDTH - size::SWITCH_KNOB;
    let rail = div()
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .w(px(size::SLIDER_WIDTH))
        .h(px(size::SWITCH_KNOB))
        .child(
            div()
                .w_full()
                .h(px(size::TRACK_HEIGHT))
                .rounded(px(radius::FULL))
                .bg(palette.control)
                .child(
                    div()
                        .h_full()
                        .w(relative(value))
                        .rounded(px(radius::FULL))
                        .bg(palette.accent),
                ),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .left(px(value * travel))
                .size(px(size::SWITCH_KNOB))
                .rounded(px(radius::FULL))
                .border_1()
                .border_color(palette.separator)
                .bg(palette.on_accent),
        );
    // The rail's place on screen, from the frame that drew it, maps a pointer to a value.
    let rail_bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let at = {
        let rail_bounds = Rc::clone(&rail_bounds);
        Rc::new(move |x: Pixels| {
            let rail = rail_bounds.get();
            ((f32::from(x - rail.origin.x) - size::SWITCH_KNOB / 2.0) / travel).clamp(0.0, 1.0)
        })
    };
    let set = Rc::new(set);
    div()
        .on_children_prepainted(move |children, _, _| {
            if let Some(rail) = children.first() {
                rail_bounds.set(*rail);
            }
        })
        .id(id)
        .test_support()
        .role(Role::Slider)
        .aria_label(label)
        .aria_numeric_value(f64::from(value))
        .aria_min_numeric_value(0.0)
        .aria_max_numeric_value(1.0)
        .track_focus(focus)
        .flex_none()
        .p(px(space::XXS))
        .relative()
        .left(px(RING_OUTSET))
        .rounded(px(radius::BUTTON))
        .border_2()
        .border_color(gpui_kit::transparent_black())
        .cursor_pointer()
        .focus_visible(move |style| style.border_color(palette.accent))
        .on_mouse_down(MouseButton::Left, {
            let (set, at) = (Rc::clone(&set), Rc::clone(&at));
            move |event: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                set(at(event.position.x), cx)
            }
        })
        .on_mouse_move({
            let set = Rc::clone(&set);
            move |event: &MouseMoveEvent, _: &mut Window, cx: &mut App| {
                if event.pressed_button == Some(MouseButton::Left) {
                    set(at(event.position.x), cx);
                }
            }
        })
        .on_key_down(move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let next = match event.keystroke.key.as_str() {
                "left" => value - SLIDER_STEP,
                "right" => value + SLIDER_STEP,
                "home" => 0.0,
                "end" => 1.0,
                _ => return,
            };
            set(next.clamp(0.0, 1.0), cx);
            cx.stop_propagation();
        })
        .child(rail)
}

/// A segmented control: one segment for each `(key, label)` choice on the control fill, the
/// chosen one raised on the segment fill. The control is one tab stop; Left and Right move the
/// choice. Each segment takes `<id>.<key>`.
pub fn segmented<F: Fn(&'static str, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    choices: &[(&'static str, &'static str)],
    chosen: &str,
    focus: &FocusHandle,
    choose: F,
) -> impl IntoElement + use<F> {
    let choose = Rc::new(choose);
    let keys: Vec<&'static str> = choices.iter().map(|(key, _)| *key).collect();
    let at = keys.iter().position(|key| *key == chosen).unwrap_or(0);
    let segments: Vec<_> = choices
        .iter()
        .map(|&(key, text)| {
            let selected = key == chosen;
            let segment = div()
                .id(SharedString::from(format!("{id}.{key}")))
                .test_support()
                .role(Role::Tab)
                .aria_label(text)
                .aria_selected(selected)
                .flex()
                .items_center()
                .justify_center()
                .w(px(size::SEGMENT_WIDTH))
                .h_full()
                .rounded(px(radius::BUTTON - size::FOCUS_RING))
                .text_token(BUTTON)
                .text_color(palette.label)
                .child(text);
            if selected {
                segment.bg(palette.segment)
            } else {
                let choose = Rc::clone(&choose);
                segment
                    .cursor_pointer()
                    .hover(move |style| style.bg(palette.selection))
                    .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| choose(key, cx))
            }
        })
        .collect();
    div()
        .id(id)
        .test_support()
        .role(Role::TabList)
        .aria_label(label)
        .track_focus(focus)
        .flex_none()
        .flex()
        .h(px(size::BUTTON_HEIGHT))
        .rounded(px(radius::BUTTON))
        .border_2()
        .border_color(gpui_kit::transparent_black())
        .bg(palette.control)
        .focus_visible(move |style| style.border_color(palette.accent))
        .on_key_down(move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let next = match event.keystroke.key.as_str() {
                "left" => at.saturating_sub(1),
                "right" => at + 1,
                _ => return,
            };
            if let Some(&key) = keys.get(next) {
                choose(key, cx);
            }
            cx.stop_propagation();
        })
        .children(segments)
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
