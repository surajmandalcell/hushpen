//! The native building blocks the views share: the grouped section, its rows, the lines
//! between rows, the push button, the switch, and the pop-up button. Colors come from the
//! [`Palette`] the view passes in.

use crate::hook;
use crate::theme::{self, BODY, BUTTON, Palette, SECONDARY, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::gpui::accesskit::Toggled;
use gpui_kit::{
    AbsoluteLength, AnyElement, App, ClickEvent, Div, Entity, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px, svg,
};

/// Space between the icon of a row and its text.
pub const ROW_GAP: f32 = space::SM;

/// Where the line between two rows starts: at the text of a row that has an icon.
pub const LINE_INSET: f32 = size::ROW_PADDING + size::NAV_ICON + ROW_GAP;

/// Where the line starts in a group whose rows have no icon.
pub const PLAIN_INSET: f32 = size::ROW_PADDING;

/// The track of a switch.
const SWITCH_WIDTH: f32 = 34.0;
const SWITCH_HEIGHT: f32 = 20.0;
const SWITCH_KNOB: f32 = 16.0;

/// The widest a pop-up button grows before its text is cut.
const MENU_MAX_WIDTH: f32 = 240.0;

/// A section with a stroke and a soft fill, whose rows have an icon. `rows` go in as they are;
/// a line is drawn between each two of them.
pub fn group(id: SharedString, palette: Palette, rows: Vec<AnyElement>) -> impl IntoElement {
    group_of(id, palette, LINE_INSET, rows)
}

/// The same for rows that start with their title.
pub fn plain_group(id: SharedString, palette: Palette, rows: Vec<AnyElement>) -> impl IntoElement {
    group_of(id, palette, PLAIN_INSET, rows)
}

/// The frame of a group with no rows yet, for a view that draws its rows itself.
pub fn group_frame(
    id: SharedString,
    palette: Palette,
) -> impl IntoElement + ParentElement + Styled {
    let mut group = div()
        .id(id)
        .test_support()
        .role(Role::Group)
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(palette.group)
        .rounded(px(radius::GROUP))
        .border_1()
        .border_color(palette.separator);
    let stroke = Some(AbsoluteLength::from(px(space::HAIRLINE)));
    let widths = &mut group.style().border_widths;
    (widths.top, widths.right, widths.bottom, widths.left) = (stroke, stroke, stroke, stroke);
    group
}

fn group_of(
    id: SharedString,
    palette: Palette,
    inset: f32,
    rows: Vec<AnyElement>,
) -> impl IntoElement {
    let mut group = group_frame(id, palette);
    let mut first = true;
    for row in rows {
        if !first {
            group = group.child(line(palette, inset));
        }
        first = false;
        group = group.child(row);
    }
    group
}

/// The line between two rows of a group, from `inset` to the end.
pub fn line(palette: Palette, inset: f32) -> impl IntoElement {
    div()
        .w_full()
        .flex_none()
        .pl(px(inset))
        .child(div().w_full().h(theme::hairline()).bg(palette.separator))
}

/// A group with the one line of help under it. The help is secondary text, cut to one line.
pub fn section(
    palette: Palette,
    group: impl IntoElement,
    footer: Option<(SharedString, String)>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .w_full()
        .child(group)
        .children(footer.map(|(id, text)| {
            secondary(palette)
                .id(id)
                .test_support()
                .px(px(size::ROW_PADDING))
                .child(text)
        }))
}

/// One line of a group: icon, text, then a value or controls on the right.
pub fn row(palette: Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .w_full()
        .flex_none()
        .min_h(px(size::ROW_MIN_HEIGHT))
        .px(px(size::ROW_PADDING))
        .py(px(space::XS + space::XXS))
        .text_token(BODY)
        .text_color(palette.label)
}

/// A row with a title on the left, at most one short line of help under it, and a value or
/// control on the right. `id` names the row; the title and the help take `<id>.title` and
/// `<id>.detail`.
pub fn labeled_row(
    palette: Palette,
    id: SharedString,
    label: impl Into<SharedString>,
    detail: Option<String>,
    trailing: impl IntoElement,
) -> AnyElement {
    let label = label.into();
    let mut text = text_column().child(
        title(palette)
            .id(SharedString::from(format!("{id}.title")))
            .test_support()
            .child(label),
    );
    if let Some(detail) = detail {
        text = text.child(
            secondary(palette)
                .id(SharedString::from(format!("{id}.detail")))
                .test_support()
                .child(detail),
        );
    }
    row(palette)
        .id(id)
        .test_support()
        .child(text)
        .child(div().flex_none().flex().items_center().child(trailing))
        .into_any_element()
}

/// The monochrome icon at the start of a row.
pub fn icon(palette: Palette, name: IconName) -> impl IntoElement {
    svg()
        .path(name.path())
        .size(px(size::NAV_ICON))
        .flex_none()
        .text_color(palette.label_secondary)
}

/// A small monochrome glyph at the end of a row or inside a button.
pub fn glyph(palette: Palette, name: IconName) -> impl IntoElement {
    svg()
        .path(name.path())
        .size(px(size::NAV_ICON - space::XS))
        .flex_none()
        .text_color(palette.label_secondary)
}

/// The text of a row, which takes the room the controls leave.
pub fn text_column() -> Div {
    div().flex_1().min_w_0().flex().flex_col()
}

/// A row title: one line, cut with an ellipsis.
pub fn title(palette: Palette) -> Div {
    div().text_token(BODY).text_color(palette.label).truncate()
}

/// The short line under a title, or a group footer: one line, cut with an ellipsis.
pub fn secondary(palette: Palette) -> Div {
    div()
        .text_token(SECONDARY)
        .text_color(palette.label_secondary)
        .truncate()
}

/// A status word or value at the right end of a row. It never wraps.
pub fn value(palette: Palette) -> Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_token(BODY)
        .text_color(palette.label_secondary)
}

/// A row that says something, with no control: a notice, an empty list, a failure. It stays one
/// line and is cut at the end.
pub fn note_row(palette: Palette, id: SharedString, text: String, alert: bool) -> AnyElement {
    let mut note = row(palette)
        .id(id.clone())
        .test_support()
        .aria_label(text.clone())
        .child(glyph(
            palette,
            if alert {
                IconName::CircleAlert
            } else {
                IconName::Info
            },
        ))
        .child(
            title(palette)
                .flex_1()
                .min_w_0()
                .text_color(if alert {
                    palette.label
                } else {
                    palette.label_secondary
                })
                .child(text),
        );
    if alert {
        note = note.role(Role::Alert);
    }
    note.into_any_element()
}

/// A row that says the state of something, with an optional button at its end. A failure shows
/// in the error color. It stays one line and is cut at the end.
pub fn status_row(
    palette: Palette,
    id: SharedString,
    text: String,
    failed: bool,
    trailing: Option<AnyElement>,
) -> AnyElement {
    row(palette)
        .id(id)
        .test_support()
        .role(Role::Status)
        .aria_label(text.clone())
        .child(glyph(
            palette,
            if failed {
                IconName::CircleAlert
            } else {
                IconName::Info
            },
        ))
        .child(
            title(palette)
                .flex_1()
                .min_w_0()
                .text_color(if failed {
                    palette.error
                } else {
                    palette.label_secondary
                })
                .child(text),
        )
        .children(trailing)
        .into_any_element()
}

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
        .size(px(SWITCH_KNOB))
        .flex_none()
        .rounded(px(radius::FULL))
        .bg(palette.on_accent);
    let track = div()
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .px(px((SWITCH_HEIGHT - SWITCH_KNOB) / 2.0))
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
        .max_w(px(MENU_MAX_WIDTH))
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

/// One option of an open pop-up list, drawn as a row of the group. The chosen one has a check.
pub fn option_row<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: String,
    selected: bool,
    focus: &FocusHandle,
    choose: F,
) -> AnyElement {
    let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| choose(window, cx);
    row(palette)
        .id(id)
        .test_support()
        .track_focus(focus)
        .role(Role::ListBoxOption)
        .aria_label(label.clone())
        .aria_selected(selected)
        .px(px(size::ROW_PADDING - size::FOCUS_RING))
        .border_2()
        .border_color(gpui_kit::transparent_black())
        .cursor_pointer()
        .hover(move |style| style.bg(palette.hover()))
        .focus_visible(move |style| style.border_color(palette.accent))
        .on_click(on_click)
        .child(
            svg()
                .path(IconName::Check.path())
                .size(px(size::NAV_ICON))
                .flex_none()
                .text_color(if selected {
                    palette.label
                } else {
                    gpui_kit::transparent_black().into()
                }),
        )
        .child(title(palette).flex_1().min_w_0().child(label))
        .into_any_element()
}

/// A row that works like a button: the hover fill, the ring under keyboard focus, and the click.
/// Add the text and the trailing glyph as children.
pub fn press_row<F: Fn(&mut Window, &mut App) + 'static>(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    press: F,
) -> impl IntoElement + ParentElement + Styled {
    hook::mark_disabled(&id, !enabled);
    let element = row(palette)
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .px(px(size::ROW_PADDING - size::FOCUS_RING))
        .border_2()
        .border_color(gpui_kit::transparent_black());
    if enabled {
        let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| press(window, cx);
        element
            .track_focus(focus)
            .cursor_pointer()
            .hover(move |style| style.bg(palette.hover()))
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_click(on_click)
    } else {
        element.text_color(palette.label_tertiary)
    }
}

/// A text field on the control fill. Its stroke shows only under keyboard focus, as the ring.
pub fn field(
    palette: Palette,
    id: SharedString,
    label: &'static str,
    state: &Entity<InputState>,
) -> Input {
    Input::new(state)
        .id(id)
        .aria_label(label)
        .bg(palette.control)
        .rounded(px(radius::BUTTON))
        .border_color(gpui_kit::transparent_black())
}
