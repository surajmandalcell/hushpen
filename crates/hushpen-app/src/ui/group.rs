//! The grouped section: its frame, the lines between its rows, the title above it, and the help
//! line under it.

use super::secondary;
use crate::theme::{self, HEADING, Palette, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AbsoluteLength, AnyElement, InteractiveElement, IntoElement, ParentElement, Role, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};

/// A section with a stroke and a soft fill. `rows` go in as they are; a line is drawn between
/// each two of them.
pub fn group(id: SharedString, palette: Palette, rows: Vec<AnyElement>) -> impl IntoElement {
    let mut group = group_frame(id, palette);
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            group = group.child(line(palette));
        }
        group = group.child(row);
    }
    group
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

/// The line between two rows of a group. It spans the whole width of the group.
pub fn line(palette: Palette) -> impl IntoElement {
    div()
        .w_full()
        .flex_none()
        .h(theme::hairline())
        .bg(palette.separator)
}

/// The title above a group, like a group title in System Settings: one line in the heading type
/// and the secondary label color, lined up with the text of the rows.
pub fn heading(palette: Palette, id: SharedString, text: &'static str) -> impl IntoElement {
    div()
        .id(id)
        .test_support()
        .role(Role::Heading)
        .aria_label(text)
        .px(px(size::ROW_PADDING))
        .text_token(HEADING)
        .text_color(palette.label_secondary)
        .truncate()
        .child(text)
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
