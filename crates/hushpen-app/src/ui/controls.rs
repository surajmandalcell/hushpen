//! The controls of a row: the push button, the switch, the slider, the pop-up button, the text
//! field, and the progress track.

use super::glyph;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size, space};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable, Size};
use gpui_kit::gpui::accesskit::Toggled;
use gpui_kit::{
    App, Bounds, ClickEvent, Div, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative, svg,
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
    icon_button_enabled(palette, id, name, label, tooltip, focus, true, press)
}

/// An [`icon_button`] that can be off. An off one draws its icon in the tertiary label color and
/// takes no click and no focus; its tooltip still names it.
#[expect(
    clippy::too_many_arguments,
    reason = "the arguments of icon_button and its state"
)]
pub fn icon_button_enabled<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    name: IconName,
    label: String,
    tooltip: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    press: F,
) -> impl IntoElement + use<F> {
    let tone = if enabled {
        palette.label_secondary
    } else {
        palette.label_tertiary
    };
    icon_frame(palette, id, label, tooltip, focus, enabled, press).child(
        svg()
            .path(name.path())
            .size(px(size::NAV_ICON))
            .flex_none()
            .text_color(tone),
    )
}

/// An icon button for one of two states, such as Play and Pause: it shows `icons[1]` while
/// `on`. A change cross-fades the two icons, the new one growing into place.
#[expect(
    clippy::too_many_arguments,
    reason = "the arguments of icon_button and its state"
)]
pub fn swap_icon_button<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    icons: [IconName; 2],
    on: bool,
    label: &'static str,
    tooltip: &'static str,
    focus: &FocusHandle,
    press: F,
) -> impl IntoElement + use<F> {
    let place = SharedString::from(format!("{id}.icon"));
    let swap = motion::moving(
        place.clone(),
        if on { 1.0 } else { 0.0 },
        motion::FAST,
        move |at| {
            // A settled frame draws only the icon of the state, as `icon_button` draws it.
            let layer = move |index: usize, shown: f32| {
                (shown > 0.0).then(|| {
                    let scale = SWAP_ICON_FROM + (1.0 - SWAP_ICON_FROM) * shown;
                    let icon = div()
                        .id(SharedString::from(format!("{place}.{index}")))
                        .test_support()
                        .flex_none()
                        .size(px(size::NAV_ICON * scale))
                        .child(
                            svg()
                                .path(icons[index].path())
                                .size_full()
                                .text_color(palette.label_secondary),
                        );
                    let icon = if shown < 1.0 {
                        icon.opacity(shown)
                    } else {
                        icon
                    };
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon)
                })
            };
            div()
                .relative()
                .flex_none()
                .size(px(size::NAV_ICON))
                .children(layer(0, 1.0 - at))
                .children(layer(1, at))
        },
    );
    icon_frame(palette, id, label.to_owned(), tooltip, focus, true, press).child(swap)
}

/// The scale an icon of a [`swap_icon_button`] grows from as it comes in.
const SWAP_ICON_FROM: f32 = 0.25;

/// The 22 px square of an icon button, with its ring, hover fill, tooltip, and click.
fn icon_frame<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: String,
    tooltip: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    press: F,
) -> impl IntoElement + ParentElement + use<F> {
    hook::mark_disabled(&id, !enabled);
    let element = div()
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
        .border_color(gpui_kit::transparent_black());
    let element = super::tooltip(element, tooltip);
    if enabled {
        let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| press(window, cx);
        element
            .track_focus(focus)
            .focus_visible(move |style| style.border_color(palette.accent))
            .cursor_pointer()
            .hover(move |style| style.bg(palette.selection))
            .on_click(on_click)
    } else {
        element
    }
}

/// The ring of a switch or a slider and its gap sit this far outside the track. The control is
/// drawn this far right of its place, so its track ends on the same line as the buttons in the
/// rows around it.
const RING_OUTSET: f32 = size::FOCUS_RING + space::XXS;

/// A switch with its label for the accessibility tree. The whole control, ring included, is the
/// element with `id`. The track is the accent while it is on. A change slides the knob across
/// and blends the track into its new fill.
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
    let focus = focus.clone();
    let place = SharedString::from(format!("{id}.knob"));
    let target = if on { 1.0 } else { 0.0 };
    motion::moving(place, target, motion::SLIDE, move |at: f32| {
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
            .rounded(px(radius::FULL));
        // At rest the switch is drawn as it always was. On the way, the accent fades in over the
        // off fill and the knob stands part of the way across.
        let track = if at >= 1.0 {
            track.bg(palette.accent).justify_end().child(knob)
        } else if at <= 0.0 {
            track.bg(palette.label_tertiary).justify_start().child(knob)
        } else {
            track
                .relative()
                .bg(palette.label_tertiary)
                .justify_start()
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(px(radius::FULL))
                        .bg(palette.accent)
                        .opacity(at),
                )
                .child(knob.ml(px(at * (size::SWITCH_WIDTH - size::SWITCH_HEIGHT))))
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
            let on_click =
                move |_: &ClickEvent, window: &mut Window, cx: &mut App| toggle(window, cx);
            element = element
                .track_focus(&focus)
                .cursor_pointer()
                .focus_visible(move |style| style.border_color(palette.accent))
                .on_click(on_click);
        }
        element
    })
}

/// How far Left or Right moves a slider: one 5% step.
const SLIDER_STEP: f32 = 0.05;

/// A slider and what its value reads.
pub struct Slider<'a> {
    /// The slider. [`super::slider_row`] puts the reading at `<id>.value` and the row at
    /// `<id>.row`.
    pub id: SharedString,
    /// The title of the row and the name of the slider in the accessibility tree.
    pub label: &'static str,
    /// Where the knob stands, from 0 to 1.
    pub position: f32,
    /// The value in words, such as `150%` or `1.25×`. It is the accessibility value too.
    pub reading: String,
    pub focus: &'a FocusHandle,
}

/// A slider from 0 to 1: the accent fills the track up to a round knob. A press or a drag along
/// it sets the value; Left, Right, Home, and End move it from the keyboard.
pub(super) fn slider<F: Fn(f32, &mut App) + 'static>(
    palette: Palette,
    slider: Slider,
    set: F,
) -> impl IntoElement + use<F> {
    let Slider {
        id,
        label,
        position,
        reading,
        focus,
    } = slider;
    let value = position.clamp(0.0, 1.0);
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
        .aria_value(reading)
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
/// choice, and the fill slides to it. Each segment takes `<id>.<key>` and is `width` wide.
#[expect(clippy::too_many_arguments, reason = "the parts of one control")]
pub fn segmented<F: Fn(&'static str, &mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    choices: &[(&'static str, &'static str)],
    width: f32,
    chosen: &str,
    focus: &FocusHandle,
    choose: F,
) -> impl IntoElement + use<F> {
    let choose = Rc::new(choose);
    let choices = choices.to_vec();
    let keys: Vec<&'static str> = choices.iter().map(|(key, _)| *key).collect();
    let at = keys.iter().position(|key| *key == chosen).unwrap_or(0);
    let focus = focus.clone();
    let place = SharedString::from(format!("{id}.fill"));
    motion::moving(place, at as f32, motion::SLIDE, move |fill: f32| {
        // At rest the chosen segment carries the fill, as it always did. On the way, the fill is a
        // pill of its own under the segments, part of the way across.
        let sliding = fill != at as f32;
        let segments: Vec<_> = choices
            .iter()
            .enumerate()
            .map(|(index, &(key, text))| {
                let selected = index == at;
                let segment = div()
                    .id(SharedString::from(format!("{id}.{key}")))
                    .test_support()
                    .role(Role::Tab)
                    .aria_label(text)
                    .aria_selected(selected)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(width))
                    .h_full()
                    .rounded(px(radius::BUTTON - size::FOCUS_RING))
                    .text_token(BUTTON)
                    .text_color(palette.label)
                    .child(text);
                if selected && !sliding {
                    segment.bg(palette.segment)
                } else if selected {
                    segment
                } else {
                    let choose = Rc::clone(&choose);
                    segment
                        .cursor_pointer()
                        .hover(move |style| style.bg(palette.selection))
                        .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                            choose(key, window, cx)
                        })
                }
            })
            .collect();
        let pill = sliding.then(|| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(fill * width))
                .w(px(width))
                .rounded(px(radius::BUTTON - size::FOCUS_RING))
                .bg(palette.segment)
        });
        div()
            .id(id)
            .test_support()
            .role(Role::TabList)
            .aria_label(label)
            .track_focus(&focus)
            .relative()
            .flex_none()
            .flex()
            .h(px(size::BUTTON_HEIGHT))
            .rounded(px(radius::BUTTON))
            .border_2()
            .border_color(gpui_kit::transparent_black())
            .bg(palette.control)
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_key_down(
                move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                    let next = match event.keystroke.key.as_str() {
                        "left" => at.saturating_sub(1),
                        "right" => at + 1,
                        _ => return,
                    };
                    if let Some(&key) = keys.get(next) {
                        choose(key, window, cx);
                    }
                    cx.stop_propagation();
                },
            )
            .children(pill)
            .children(segments)
    })
}

/// A pop-up button: the current value and a chevron. [`super::pop_up`] adds its menu.
pub(super) fn menu_button<F: Fn(&mut Window, &mut App) + 'static>(
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
        .max_w(px(size::POP_UP_MAX_WIDTH))
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

/// A [`field`] that leads with the search glyph: the search of a view and of a long menu.
pub fn search_field(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    state: &Entity<InputState>,
) -> Input {
    field(palette, id, label, state).prefix(glyph(palette, IconName::Search))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AnyWindowHandle, AppContext as _, Context, Render, TestAppContext, WindowHandle, size,
    };

    struct Probe {
        on: bool,
        chosen: &'static str,
        focus: FocusHandle,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let palette = theme::palette(cx);
            div()
                .child(switch(
                    palette,
                    "probe.switch".into(),
                    "Probe",
                    self.on,
                    true,
                    &self.focus,
                    |_, _| {},
                ))
                .child(segmented(
                    palette,
                    "probe.mode".into(),
                    "Mode",
                    &[("a", "A"), ("b", "B")],
                    size::SEGMENT_WIDTH,
                    self.chosen,
                    &self.focus,
                    |_, _, _| {},
                ))
        }
    }

    /// Draws a frame `millis` after the last one, with the frames the last one asked for, and
    /// says how many it asked for.
    fn frame(cx: &mut TestAppContext, handle: AnyWindowHandle, millis: u64) -> usize {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(millis));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let asked = window.simulate_next_frame(cx);
            window.render_frame(cx);
            asked
        })
        .unwrap()
    }

    /// A switch and a segmented control slide to a new state, and say the new state at once.
    #[gpui_kit::test]
    fn a_switch_and_a_segmented_control_slide_and_then_rest(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_global(motion::tests::Play));
        let window: WindowHandle<Probe> =
            cx.open_window(size(px(400.0), px(100.0)), |_, cx| Probe {
                on: false,
                chosen: "a",
                focus: cx.focus_handle(),
            });
        let handle: AnyWindowHandle = window.into();
        frame(cx, handle, 0);
        let before = cx
            .update_window(handle, |_, window, _| {
                (
                    window.find("probe.switch").bounds(),
                    window.find("probe.mode.b").bounds(),
                )
            })
            .unwrap();
        window
            .update(cx, |probe, _, cx| {
                probe.on = true;
                probe.chosen = "b";
                cx.notify();
            })
            .unwrap();
        frame(cx, handle, 0);
        let (label, selected, after) = cx
            .update_window(handle, |_, window, _| {
                (
                    window.find("probe.switch").label().map(str::to_owned),
                    window.find("probe.mode.b").selected(),
                    (
                        window.find("probe.switch").bounds(),
                        window.find("probe.mode.b").bounds(),
                    ),
                )
            })
            .unwrap();
        assert_eq!(
            label.as_deref(),
            Some("Probe, on"),
            "the new state reads at once"
        );
        assert_eq!(selected, Some(true));
        assert_eq!(after, before, "only the knob and the fill move");
        assert!(frame(cx, handle, 50) > 0, "the slide asks for frames");
        frame(cx, handle, motion::SLIDE.as_millis() as u64);
        assert_eq!(frame(cx, handle, 0), 0, "at rest it asks for none");
    }

    struct Swap {
        on: bool,
        focus: FocusHandle,
    }

    impl Render for Swap {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let name = if self.on { "Pause" } else { "Play" };
            div().child(swap_icon_button(
                theme::palette(cx),
                "probe.play".into(),
                [IconName::Play, IconName::Pause],
                self.on,
                name,
                name,
                &self.focus,
                |_, _| {},
            ))
        }
    }

    /// Opens a swap button that shows Play, and turns it to Pause.
    fn swap_to_pause(cx: &mut TestAppContext) -> AnyWindowHandle {
        cx.update(gpui_kit::init);
        let window: WindowHandle<Swap> = cx.open_window(size(px(100.0), px(100.0)), |_, cx| Swap {
            on: false,
            focus: cx.focus_handle(),
        });
        let handle: AnyWindowHandle = window.into();
        frame(cx, handle, 0);
        assert_eq!(layers(cx, handle), [Some(ICON), None], "it rests on Play");
        window
            .update(cx, |swap, _, cx| {
                swap.on = true;
                cx.notify();
            })
            .unwrap();
        frame(cx, handle, 0);
        let label = cx
            .update_window(handle, |_, window, _| {
                window.find("probe.play").label().map(str::to_owned)
            })
            .unwrap();
        assert_eq!(
            label.as_deref(),
            Some("Pause"),
            "the new state reads at once"
        );
        handle
    }

    const ICON: f32 = crate::theme::size::NAV_ICON;

    /// The width of the Play and the Pause icon of the swap button, when it draws them.
    fn layers(cx: &mut TestAppContext, handle: AnyWindowHandle) -> [Option<f32>; 2] {
        cx.update_window(handle, |_, window, _| {
            ["probe.play.icon.0", "probe.play.icon.1"].map(|id| {
                window
                    .try_find(id)
                    .map(|icon| f32::from(icon.bounds().size.width))
            })
        })
        .unwrap()
    }

    #[gpui_kit::test]
    fn a_swap_icon_button_cross_fades_to_the_icon_of_its_new_state(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(motion::tests::Play));
        let handle = swap_to_pause(cx);
        frame(cx, handle, motion::FAST.as_millis() as u64 / 2);
        let [play, pause] = layers(cx, handle);
        let (play, pause) = (
            play.expect("Play fades out"),
            pause.expect("Pause comes in"),
        );
        assert!(
            0.0 < pause && pause < ICON,
            "Pause grows into place: {pause}"
        );
        assert!(0.0 < play && play < ICON, "Play shrinks away: {play}");
        frame(cx, handle, motion::FAST.as_millis() as u64);
        assert_eq!(layers(cx, handle), [None, Some(ICON)], "it rests on Pause");
        assert_eq!(frame(cx, handle, 0), 0, "at rest it asks for no frames");
    }

    #[gpui_kit::test]
    fn under_reduce_motion_a_swap_icon_button_changes_at_once(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.set_global(motion::tests::Play);
            cx.set_reduce_motion(true);
        });
        let handle = swap_to_pause(cx);
        assert_eq!(layers(cx, handle), [None, Some(ICON)]);
        assert_eq!(frame(cx, handle, 0), 0, "no frame is asked for");
    }
}
