//! The Dictionary view: one grouped panel with the form on top and a row for each entry.

use super::Dictionary;
use crate::hook;
use crate::theme::{
    self, BODY_MD, BODY_SM, LABEL_MD, ROW_TITLE, StyledType, color, radius, size, space,
};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
};

const EMPTY_TEXT: &str = "No words yet. Add names and terms that Hushpen should get right.";

struct RowView {
    id: i64,
    phrase: String,
    heard_as: Option<String>,
}

#[derive(Clone, Copy)]
enum Action {
    Edit,
    Delete,
}

pub fn render(dictionary: &Entity<Dictionary>, cx: &mut App) -> impl IntoElement + use<> {
    let (rows, focus, message, editing, inputs, submit_focus, cancel_focus) = {
        let view = dictionary.read(cx);
        let rows: Vec<RowView> = view
            .entries()
            .iter()
            .map(|entry| RowView {
                id: entry.id,
                phrase: entry.phrase.clone(),
                heard_as: entry.heard_as.clone(),
            })
            .collect();
        (
            rows,
            view.row_focus.clone(),
            view.message().map(str::to_owned),
            view.editing(),
            (view.phrase_input().clone(), view.heard_input().clone()),
            view.submit_focus.clone(),
            view.cancel_focus.clone(),
        )
    };

    let mut panel = div()
        .id(hook::id("dictionary", "list"))
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(form(
            dictionary,
            editing.is_some(),
            inputs,
            &submit_focus,
            &cancel_focus,
        ));
    if let Some(message) = message {
        panel = panel.child(message_row(message));
    }
    if rows.is_empty() {
        panel = panel.child(empty_row());
    }
    for (index, (row, focus)) in rows.into_iter().zip(focus).enumerate() {
        panel = panel.child(entry_row(dictionary, index, row, focus, editing));
    }
    panel
}

fn form(
    dictionary: &Entity<Dictionary>,
    editing: bool,
    (phrase, heard): (
        Entity<gpui_kit::component::input::InputState>,
        Entity<gpui_kit::component::input::InputState>,
    ),
    submit_focus: &FocusHandle,
    cancel_focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let submit = {
        let dictionary = dictionary.clone();
        move |window: &mut Window, cx: &mut App| {
            // A refusal shows in the message row.
            let _ = dictionary.update(cx, |dictionary, cx| dictionary.submit(window, cx));
        }
    };
    let cancel = {
        let dictionary = dictionary.clone();
        move |window: &mut Window, cx: &mut App| {
            dictionary.update(cx, |dictionary, cx| dictionary.cancel_edit(window, cx));
        }
    };
    let mut buttons = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(button(
            hook::id("dictionary", "submit"),
            if editing { "Save" } else { "Add" },
            submit_focus,
            submit,
        ));
    if editing {
        buttons = buttons.child(button(
            hook::id("dictionary", "cancel"),
            "Cancel",
            cancel_focus,
            cancel,
        ));
    }
    div()
        .id(hook::id("dictionary", "form"))
        .test_support()
        .flex()
        .items_end()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .child(field("phrase", "Write as", &phrase))
        .child(field("heard", "Heard as (optional)", &heard))
        .child(buttons)
}

fn field(
    name: &'static str,
    label: &'static str,
    state: &Entity<gpui_kit::component::input::InputState>,
) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(color::TEXT_MUTED))
                .child(label),
        )
        .child(
            Input::new(state)
                .id(hook::id("dictionary", name))
                .aria_label(label),
        )
}

fn message_row(message: String) -> impl IntoElement {
    div()
        .id(hook::id("dictionary", "message"))
        .test_support()
        .role(Role::Alert)
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::STATUS_ERROR))
        .child(
            svg()
                .path(IconName::CircleAlert.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::STATUS_ERROR)),
        )
        .child(div().flex_1().min_w_0().child(message))
}

fn empty_row() -> impl IntoElement {
    div()
        .id(hook::id("dictionary", "empty"))
        .test_support()
        .px(px(space::LG))
        .py(px(space::LG))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::TEXT_MUTED))
        .child(EMPTY_TEXT)
}

fn entry_row(
    dictionary: &Entity<Dictionary>,
    index: usize,
    row: RowView,
    focus: [FocusHandle; 2],
    editing: Option<i64>,
) -> impl IntoElement + use<> {
    let [edit_focus, delete_focus] = focus;
    let detail = match &row.heard_as {
        Some(heard) => format!("Heard as \u{201c}{heard}\u{201d}"),
        None => "Word".to_owned(),
    };
    let controls = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(row_button(
            dictionary,
            index,
            row.id,
            Action::Edit,
            "Edit",
            &edit_focus,
        ))
        .child(row_button(
            dictionary,
            index,
            row.id,
            Action::Delete,
            "Delete",
            &delete_focus,
        ));
    let mut container = div()
        .id(hook::indexed("dictionary", "row", index))
        .test_support()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)));
    if editing == Some(row.id) {
        container = container.bg(theme::rgb_of(color::SURFACE_SELECTED));
    }
    container
        .child(icon_tile(IconName::BookA))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(hook::indexed("dictionary", "name", index))
                        .test_support()
                        .aria_label(row.phrase.clone())
                        .text_token(ROW_TITLE)
                        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                        .truncate()
                        .child(row.phrase),
                )
                .child(
                    div()
                        .id(hook::indexed("dictionary", "detail", index))
                        .test_support()
                        .aria_label(detail.clone())
                        .w_full()
                        .min_w_0()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .truncate()
                        .child(detail),
                ),
        )
        .child(controls)
}

fn row_button(
    dictionary: &Entity<Dictionary>,
    index: usize,
    id: i64,
    action: Action,
    label: &'static str,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let press = {
        let dictionary = dictionary.clone();
        move |window: &mut Window, cx: &mut App| {
            // A refusal shows in the message row.
            let _ = dictionary.update(cx, |dictionary, cx| match action {
                Action::Edit => dictionary.start_edit(id, window, cx),
                Action::Delete => dictionary.remove(id, cx),
            });
        }
    };
    let element: SharedString = match action {
        Action::Edit => hook::indexed("dictionary", "edit", index),
        Action::Delete => hook::indexed("dictionary", "delete", index),
    };
    button(element, label, focus, press)
}

fn button<F: Fn(&mut Window, &mut App) + Clone + 'static>(
    element: SharedString,
    label: &'static str,
    focus: &FocusHandle,
    press: F,
) -> impl IntoElement + use<F> {
    let on_click = {
        let press = press.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| press(window, cx)
    };
    let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            press(window, cx);
            cx.stop_propagation();
        }
    };
    div()
        .id(element)
        .test_support()
        .track_focus(focus)
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .h(px(32.0))
        .px(px(10.0 - size::FOCUS_RING))
        .rounded(px(radius::MD))
        .border_2()
        .border_color(transparent_black())
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
        .cursor_pointer()
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(label)
}

fn icon_tile(icon: IconName) -> impl IntoElement {
    div()
        .size(px(28.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .bg(theme::rgb_of(color::SURFACE_SOFT))
        .child(
            svg()
                .path(icon.path())
                .size(px(14.0))
                .text_color(theme::rgb_of(color::TEXT_BODY)),
        )
}
