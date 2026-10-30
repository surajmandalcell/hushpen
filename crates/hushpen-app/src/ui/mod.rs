//! The native building blocks the views share: the grouped section and the lines between its
//! rows ([`group`]), the rows ([`rows`]), the controls ([`controls`]), the pop-up buttons
//! and their menus ([`menu`]), and the tooltips ([`tooltip`]). Colors come from the
//! [`Palette`] the view passes in.

mod controls;
mod group;
mod menu;
#[cfg(test)]
mod menu_tests;
mod rows;
mod tooltip;

pub use controls::{
    Kind, Slider, button, field, icon_button, icon_button_enabled, segmented, swap_icon_button,
    switch, track,
};
pub use group::{group, group_frame, heading, line, section, titled_section};
pub use menu::{Menu, PopUp, pop_up};
pub use rows::{
    Notice, fact_row, field_row, icon_row, labeled_row, menu_row, meter_row, named_lines,
    notice_row, press_row, row, slider_row, toggle_row,
};
pub use tooltip::{covered, tooltip};

use crate::theme::{BODY, Palette, SECONDARY, StyledType, size, space};
use gpui_kit::assets::IconName;
use gpui_kit::{Div, IntoElement, Styled, div, px, svg};

/// Space between the icon of a row and its text.
pub const ROW_GAP: f32 = space::SM;

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
        .size(px(size::GLYPH))
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
