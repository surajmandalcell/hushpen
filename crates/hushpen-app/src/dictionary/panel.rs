//! The Dictionary view: one group with the form on top and a row for each entry, then the word
//! packs, and the sheet that previews the words of a pack.

use super::{Dictionary, packs};
use crate::theme::{self, Palette, SECONDARY, StyledType, TOOLBAR_TITLE, radius, space};
use crate::ui::{self, Kind, Notice};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px,
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
        group_rows.push(ui::notice_row(
            palette,
            hook::id("dictionary", "message"),
            message,
            Notice::Alert,
            None,
        ));
    }
    if rows.is_empty() {
        group_rows.push(ui::notice_row(
            palette,
            hook::id("dictionary", "empty"),
            EMPTY_TEXT.to_owned(),
            Notice::Info,
            None,
        ));
    }
    for (index, (row, focus)) in rows.into_iter().zip(focus).enumerate() {
        group_rows.push(entry_row(palette, dictionary, index, row, focus, editing));
    }
    div()
        .flex()
        .flex_col()
        .gap(px(space::XXL))
        .w_full()
        .child(ui::group(
            hook::id("dictionary", "list"),
            palette,
            group_rows,
        ))
        .child(packs_section(palette, dictionary, cx))
}

/// The packs section: a heading, one row for each pack with its Preview button and its switch,
/// and one line of help.
fn packs_section(palette: Palette, dictionary: &Entity<Dictionary>, cx: &App) -> AnyElement {
    let view = dictionary.read(cx);
    let rows = packs::all()
        .iter()
        .enumerate()
        .zip(view.pack_focus.clone())
        .map(|((index, pack), [switch_focus, preview_focus])| {
            let on = view.pack_on(pack.id);
            let preview = {
                let dictionary = dictionary.clone();
                move |window: &mut Window, cx: &mut App| {
                    dictionary.update(cx, |me, cx| me.open_preview(index, window, cx));
                }
            };
            let toggle = {
                let dictionary = dictionary.clone();
                let id = pack.id;
                move |_: &mut Window, cx: &mut App| {
                    dictionary.update(cx, |me, cx| me.toggle_pack(id, cx));
                }
            };
            let detail = format!("{} words. {}", pack.words().len(), pack.about);
            ui::row(palette)
                .id(hook::id("dictionary", &format!("pack.{}", pack.id)))
                .test_support()
                .child(ui::named_lines(
                    palette,
                    hook::id("dictionary", &format!("pack.{}.title", pack.id)),
                    pack.name.clone(),
                    Some((
                        hook::id("dictionary", &format!("pack.{}.detail", pack.id)),
                        detail,
                    )),
                ))
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(space::SM))
                        .child(ui::button(
                            palette,
                            hook::id("dictionary", &format!("pack.{}.preview", pack.id)),
                            "Preview\u{2026}",
                            Kind::Plain,
                            Some(&preview_focus),
                            true,
                            preview,
                        ))
                        .child(ui::switch(
                            palette,
                            hook::id("dictionary", &format!("pack.{}.switch", pack.id)),
                            "Use this pack",
                            on,
                            true,
                            &switch_focus,
                            toggle,
                        )),
                )
                .into_any_element()
        })
        .collect();
    ui::titled_section(
        palette,
        hook::id("dictionary", "packs.heading"),
        "Packs",
        ui::group(hook::id("dictionary", "packs"), palette, rows),
        Some((hook::id("dictionary", "packs.help"), PACKS_HELP.to_owned())),
    )
}

const PACKS_HELP: &str = "Words of a pack that is on count like your own. Your words come first.";

/// The sheet with the words of one pack, or nothing while no preview is open. It comes in like
/// every sheet: the scrim fades in, and the sheet fades in and rises a few pixels into place.
pub fn preview_sheet(
    dictionary: &Entity<Dictionary>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let pack = dictionary.read(cx).preview()?;
    let arrival = motion::enter(
        hook::id("dictionary", "preview.arrival"),
        motion::BASE,
        window,
        cx,
    );
    let palette = theme::palette(cx);
    let close = {
        let dictionary = dictionary.clone();
        move |window: &mut Window, cx: &mut App| {
            dictionary.update(cx, |me, cx| me.close_preview(window, cx));
        }
    };
    let on_key = {
        let close = close.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                close(window, cx);
                cx.stop_propagation();
            }
        }
    };
    let view = dictionary.read(cx);
    let words = pack.words();
    let rows: Vec<AnyElement> = words
        .iter()
        .enumerate()
        .map(|(index, word)| {
            let heard = pack.heard_as(word);
            let detail = (!heard.is_empty()).then(|| {
                let heard: Vec<String> = heard
                    .iter()
                    .map(|heard| format!("\u{201c}{heard}\u{201d}"))
                    .collect();
                (
                    hook::indexed("dictionary", "preview.detail", index),
                    format!("Replaces {}", heard.join(", ")),
                )
            });
            ui::row(palette)
                .id(hook::indexed("dictionary", "preview.word", index))
                .test_support()
                .child(ui::named_lines(
                    palette,
                    hook::indexed("dictionary", "preview.name", index),
                    (*word).to_owned(),
                    detail,
                ))
                .into_any_element()
        })
        .collect();
    let line = format!("{} words. {}", words.len(), pack.about);
    let header = div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(hook::id("dictionary", "preview.title"))
                .test_support()
                .role(Role::Heading)
                .aria_label(pack.name.clone())
                .text_token(TOOLBAR_TITLE)
                .text_color(palette.label)
                .truncate()
                .child(pack.name.clone()),
        )
        .child(
            div()
                .id(hook::id("dictionary", "preview.line"))
                .test_support()
                .aria_label(line.clone())
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .truncate()
                .child(line),
        );
    let list = div()
        .id(hook::id("dictionary", "preview.scroll"))
        .test_support()
        .max_h(px(PREVIEW_HEIGHT))
        .overflow_y_scroll()
        .child(ui::group(
            hook::id("dictionary", "preview.list"),
            palette,
            rows,
        ));
    let sheet = div()
        .id(hook::id("dictionary", "preview"))
        .test_support()
        .role(Role::Dialog)
        .aria_label(pack.name.clone())
        .on_key_down(on_key)
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w(px(space::SHEET_WIDTH))
        .p(px(space::XL))
        .bg(palette.content)
        .rounded(px(radius::WINDOW))
        .border(theme::hairline())
        .border_color(palette.separator)
        .child(header)
        .child(list)
        .child(div().flex().justify_end().child(ui::button(
            palette,
            hook::id("dictionary", "preview.done"),
            "Done",
            Kind::Default,
            Some(&view.sheet_done),
            true,
            close,
        )));
    Some(
        div()
            .focus_trap(hook::id("dictionary", "preview.scrim"), &view.sheet_trap)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::faded(palette.scrim, arrival))
            .child(motion::arrive(sheet, arrival, motion::TRAVEL))
            .into_any_element(),
    )
}

/// The most the word list of the preview takes, so the sheet fits in the window.
const PREVIEW_HEIGHT: f32 = 324.0;

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
    let mut buttons = div().flex_none().flex().items_center().gap(px(space::SM));
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
    let buttons = buttons.child(ui::button(
        palette,
        hook::id("dictionary", "submit"),
        if editing { "Save" } else { "Add" },
        Kind::Default,
        Some(submit_focus),
        true,
        submit,
    ));
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
        .child(ui::named_lines(
            palette,
            hook::indexed("dictionary", "name", index),
            row.phrase,
            Some((hook::indexed("dictionary", "detail", index), detail)),
        ))
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
