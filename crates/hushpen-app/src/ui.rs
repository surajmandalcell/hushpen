//! The native building blocks the restyled views share: the grouped section, its rows, the lines
//! between rows, and the push button. Colors come from the [`Palette`] the view passes in.

use crate::hook;
use crate::theme::{self, BODY, BUTTON, Palette, SECONDARY, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AbsoluteLength, AnyElement, App, ClickEvent, Div, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

/// Space between the icon of a row and its text.
pub const ROW_GAP: f32 = space::SM;

/// Where the line between two rows starts: at the text of a row that has an icon.
pub const LINE_INSET: f32 = size::ROW_PADDING + size::NAV_ICON + ROW_GAP;

/// A section with a stroke and a soft fill. `rows` go in as they are; a line is drawn between
/// each two of them.
pub fn group(id: SharedString, palette: Palette, rows: Vec<AnyElement>) -> impl IntoElement {
    let mut group = div()
        .id(id)
        .test_support()
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
    let mut first = true;
    for row in rows {
        if !first {
            group = group.child(line(palette));
        }
        first = false;
        group = group.child(row);
    }
    group
}

fn line(palette: Palette) -> impl IntoElement {
    div()
        .w_full()
        .flex_none()
        .pl(px(LINE_INSET))
        .child(div().w_full().h(theme::hairline()).bg(palette.separator))
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

/// The monochrome icon at the start of a row.
pub fn icon(palette: Palette, name: gpui_kit::assets::IconName) -> impl IntoElement {
    gpui_kit::svg()
        .path(name.path())
        .size(px(size::NAV_ICON))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The accent fill: the one action of the page or row.
    Default,
    /// A neutral fill.
    Plain,
}

/// A push button, 22 px high. A disabled one keeps its focus handle so focus never falls out of
/// the window when the button turns off under the user's finger.
pub fn button<F: Fn(&mut App) + 'static>(
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
        let on_click = move |_: &ClickEvent, _: &mut Window, cx: &mut App| press(cx);
        element = element
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_click(on_click);
    }
    element.child(label)
}
