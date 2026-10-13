//! The Dictionary view: one group with the form on top and a row for each entry.

use super::Dictionary;
use crate::hook;
use crate::theme::{self, Palette, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
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
    let palette = theme::palette(cx);
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

    let mut group_rows: Vec<AnyElement> = vec![
        form(
            palette,
            dictionary,
            editing.is_some(),
            inputs,
            &submit_focus,
            &cancel_focus,
        )
        .into_any_element(),
    ];
    if let Some(message) = message {
        group_rows.push(ui::note_row(
            palette,
            hook::id("dictionary", "message"),
            message,
            true,
        ));
    }
    if rows.is_empty() {
        group_rows.push(ui::note_row(
            palette,
            hook::id("dictionary", "empty"),
            EMPTY_TEXT.to_owned(),
            false,
        ));
    }
    for (index, (row, focus)) in rows.into_iter().zip(focus).enumerate() {
        group_rows.push(entry_row(palette, dictionary, index, row, focus, editing));
    }
    ui::plain_group(hook::id("dictionary", "list"), palette, group_rows)
}

fn form(
    palette: Palette,
    dictionary: &Entity<Dictionary>,
    editing: bool,
    (phrase, heard): (Entity<InputState>, Entity<InputState>),
    submit_focus: &FocusHandle,
    cancel_focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let submit = {
        let dictionary = dictionary.clone();
        // A refusal shows in the message row.
        move |window: &mut Window, cx: &mut App| {
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
        .child(ui::button(
            palette,
            hook::id("dictionary", "submit"),
            if editing { "Save" } else { "Add" },
            Kind::Default,
            Some(submit_focus),
            true,
            submit,
        ));
    if editing {
        buttons = buttons.child(ui::button(
            palette,
            hook::id("dictionary", "cancel"),
            "Cancel",
            Kind::Plain,
            Some(cancel_focus),
            true,
            cancel,
        ));
    }
    ui::row(palette)
        .id(hook::id("dictionary", "form"))
        .test_support()
        .gap(px(space::SM))
        .child(div().flex_1().min_w_0().child(ui::field(
            palette,
            hook::id("dictionary", "phrase"),
            "Write as",
            &phrase,
        )))
        .child(div().flex_1().min_w_0().child(ui::field(
            palette,
            hook::id("dictionary", "heard"),
            "Heard as (optional)",
            &heard,
        )))
        .child(buttons)
}

fn entry_row(
    palette: Palette,
    dictionary: &Entity<Dictionary>,
    index: usize,
    row: RowView,
    focus: [FocusHandle; 2],
    editing: Option<i64>,
) -> AnyElement {
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
            palette,
            dictionary,
            index,
            row.id,
            Action::Edit,
            "Edit",
            &edit_focus,
        ))
        .child(row_button(
            palette,
            dictionary,
            index,
            row.id,
            Action::Delete,
            "Delete",
            &delete_focus,
        ));
    let mut container = ui::row(palette)
        .id(hook::indexed("dictionary", "row", index))
        .test_support();
    if editing == Some(row.id) {
        container = container.bg(palette.selection);
    }
    container
        .child(
            ui::text_column()
                .child(
                    ui::title(palette)
                        .id(hook::indexed("dictionary", "name", index))
                        .test_support()
                        .aria_label(row.phrase.clone())
                        .child(row.phrase),
                )
                .child(
                    ui::secondary(palette)
                        .id(hook::indexed("dictionary", "detail", index))
                        .test_support()
                        .aria_label(detail.clone())
                        .w_full()
                        .child(detail),
                ),
        )
        .child(controls)
        .into_any_element()
}

fn row_button(
    palette: Palette,
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
            let _ = dictionary.update(cx, |dictionary, cx| match action {
                Action::Edit => dictionary.start_edit(id, window, cx),
                Action::Delete => dictionary.remove(id, cx),
            });
        }
    };
    let element = match action {
        Action::Edit => hook::indexed("dictionary", "edit", index),
        Action::Delete => hook::indexed("dictionary", "delete", index),
    };
    ui::button(
        palette,
        element,
        label,
        Kind::Plain,
        Some(focus),
        true,
        press,
    )
}
