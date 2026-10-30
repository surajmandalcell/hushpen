//! The rows of a group. A row is one line: an optional icon, the title with at most one short
//! line under it, then a value or controls on the right.

use super::controls::{Slider, field, slider, switch};
use super::menu::{PopUp, pop_up};
use super::{ROW_GAP, icon, secondary, text_column, title, value};
use crate::hook;
use crate::theme::{BODY, Palette, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, Entity, FocusHandle, FontFeatures, InteractiveElement,
    IntoElement, ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};
use std::sync::Arc;

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

/// A row that leads with an icon: the icon, the `title`, and at most one short line under it.
/// Add the value or the controls as children.
pub fn icon_row(
    palette: Palette,
    name: IconName,
    title: impl IntoElement,
    detail: Option<AnyElement>,
) -> Div {
    row(palette)
        .child(icon(palette, name))
        .child(text_column().child(title).children(detail))
}

/// A title and at most one short line under it, each under its own id with its text as the
/// label, so a test reads them exactly.
pub fn named_lines(
    palette: Palette,
    title_id: SharedString,
    text: impl Into<SharedString>,
    detail: Option<(SharedString, String)>,
) -> Div {
    let text = text.into();
    text_column()
        .child(
            title(palette)
                .id(title_id)
                .test_support()
                .aria_label(text.clone())
                .child(text),
        )
        .children(detail.map(|(id, detail)| {
            secondary(palette)
                .id(id)
                .test_support()
                .aria_label(detail.clone())
                .w_full()
                .child(detail)
        }))
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
    let text = named_lines(
        palette,
        format!("{id}.title").into(),
        label,
        detail.map(|detail| (format!("{id}.detail").into(), detail)),
    );
    row(palette)
        .id(id)
        .test_support()
        .child(text)
        .child(div().flex_none().flex().items_center().child(trailing))
        .into_any_element()
}

/// A row with a switch on the right. The switch takes `id`, the row `<id>.row`.
pub fn toggle_row(
    palette: Palette,
    id: SharedString,
    title: &'static str,
    detail: &str,
    on: bool,
    focus: &FocusHandle,
    toggle: impl Fn(&mut App) + 'static,
) -> AnyElement {
    labeled_row(
        palette,
        format!("{id}.row").into(),
        title,
        Some(detail.to_owned()),
        switch(palette, id, title, on, true, focus, move |_, cx| toggle(cx)),
    )
}

/// A row with a slider on the right, titled with the label of the slider, and the reading of its
/// value just left of the track. The reading has a fixed width and tabular digits, so neither it
/// nor the track moves while the value changes. The slider takes `<id>`, the reading
/// `<id>.value`, the row `<id>.row`.
pub fn slider_row(
    palette: Palette,
    detail: &str,
    control: Slider,
    set: impl Fn(f32, &mut App) + 'static,
) -> AnyElement {
    let (id, title) = (control.id.clone(), control.label);
    let reading = value(palette)
        .id(SharedString::from(format!("{id}.value")))
        .test_support()
        .aria_label(control.reading.clone())
        .w(px(size::SLIDER_VALUE_WIDTH))
        .text_right()
        .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
        .child(control.reading.clone());
    let trailing = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::XS))
        .child(reading)
        .child(slider(palette, control, set));
    labeled_row(
        palette,
        format!("{id}.row").into(),
        title,
        Some(detail.to_owned()),
        trailing,
    )
}

/// A row with a pop-up button on the right, titled with the label of the button. The row takes
/// `<id>.row`.
pub fn menu_row(
    palette: Palette,
    detail: &str,
    button: PopUp,
    toggle: impl Fn(&mut App) + 'static,
) -> AnyElement {
    let (id, title) = (format!("{}.row", button.id), button.label.to_owned());
    labeled_row(
        palette,
        id.into(),
        title,
        Some(detail.to_owned()),
        pop_up(palette, button, move |_, cx| toggle(cx)),
    )
}

/// A row with a text field on the right, and an optional control after it. The field takes
/// `id`, the row `<id>.row`.
pub fn field_row(
    palette: Palette,
    id: SharedString,
    title: &'static str,
    detail: &str,
    state: &Entity<InputState>,
    extra: Option<AnyElement>,
) -> AnyElement {
    let trailing = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(
            div()
                .w(px(size::FIELD_WIDTH))
                .child(field(palette, id.clone(), title, state)),
        )
        .children(extra);
    labeled_row(
        palette,
        format!("{id}.row").into(),
        title,
        Some(detail.to_owned()),
        trailing,
    )
}

/// A row with the name of a fact on the left and its value on the right, cut to one line, with
/// an optional control after the value. The whole value is in the tooltip and in the label of
/// `value_id`. A missing value says so in the tertiary color.
pub fn fact_row(
    palette: Palette,
    id: SharedString,
    value_id: SharedString,
    title: &str,
    value: Option<String>,
    trailing: Option<AnyElement>,
) -> AnyElement {
    let full = value.clone().unwrap_or_default();
    let mut text = div()
        .id(value_id)
        .test_support()
        .aria_label(full.clone())
        .flex_1()
        .min_w_0()
        .truncate()
        .text_right()
        .text_color(if value.is_some() {
            palette.label_secondary
        } else {
            palette.label_tertiary
        })
        .child(value.unwrap_or_else(|| "Not available".to_owned()));
    if !full.is_empty() {
        text = super::tooltip(text, full);
    }
    row(palette)
        .id(id)
        .test_support()
        .gap(px(space::LG))
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .child(title.to_owned()),
        )
        .child(text)
        .children(trailing)
        .into_any_element()
}

/// What a notice row says, which sets its icon, its text color, and its role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// Plain information, such as an empty list or a running job.
    Info,
    /// Something the user has to act on.
    Alert,
    /// A failed check or job, in the error color.
    Failure,
}

impl Notice {
    /// The state of a check or a job: a failure, or plain information.
    pub fn status(failed: bool) -> Self {
        if failed { Self::Failure } else { Self::Info }
    }
}

/// A row that says something, with no control but an optional button at its end: a notice, an
/// empty list, a failure. It stays one line and is cut at the end.
pub fn notice_row(
    palette: Palette,
    id: SharedString,
    text: String,
    notice: Notice,
    trailing: Option<AnyElement>,
) -> AnyElement {
    let (name, tone, role) = match notice {
        Notice::Info => (IconName::Info, palette.label_secondary, Role::Status),
        Notice::Alert => (IconName::CircleAlert, palette.label, Role::Alert),
        Notice::Failure => (IconName::CircleAlert, palette.error, Role::Status),
    };
    row(palette)
        .id(id)
        .test_support()
        .role(role)
        .aria_label(text.clone())
        .child(icon(palette, name))
        .child(
            title(palette)
                .flex_1()
                .min_w_0()
                .text_color(tone)
                .child(text),
        )
        .children(trailing)
        .into_any_element()
}

/// The microphone level meter: one bar for each level, from 0 to 1, centered in its row and
/// growing up and down from its middle. The bars are dim while nothing listens. Ids are
/// `<view>.mic.meter` and `<view>.mic.bar.<n>`.
pub fn meter_row(palette: Palette, view: &str, levels: &[f32], listening: bool) -> AnyElement {
    let tone = if listening {
        palette.label_secondary
    } else {
        palette.label_tertiary
    };
    let span = size::METER_BAR_MAX - size::METER_BAR_REST;
    let bars = levels.iter().enumerate().map(|(index, level)| {
        div()
            .id(hook::indexed(view, "mic.bar", index))
            .test_support()
            .flex_none()
            .w(px(size::METER_BAR_WIDTH))
            .h(px(size::METER_BAR_REST + level.clamp(0.0, 1.0) * span))
            .rounded(px(radius::BUTTON))
            .bg(tone)
    });
    div()
        .id(hook::id(view, "mic.meter"))
        .test_support()
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .w_full()
        .gap(px(size::METER_BAR_GAP))
        .h(px(size::METER_BAR_MAX + 2.0 * space::MD))
        .px(px(size::ROW_PADDING))
        .py(px(space::MD))
        .children(bars)
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
