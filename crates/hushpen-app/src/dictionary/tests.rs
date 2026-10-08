use super::*;
use crate::shell::Shell;
use crate::storage;
use crate::theme::space;
use crate::views::View;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px,
    size,
};
use hushpen_store::data_dir::DataDir;

struct Rig {
    handle: AnyWindowHandle,
    dictionary: Entity<Dictionary>,
    storage: Rc<Storage>,
    _tmp: tempfile::TempDir,
}

fn open(cx: &mut TestAppContext) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    cx.update(gpui_kit::init);
    let (handle, shell) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        )
        .expect("open test window")
    });
    let dictionary = {
        let storage = Rc::clone(&storage);
        cx.update_window(handle, move |_, window, cx| {
            cx.new(|cx| Dictionary::new(storage, window, cx))
        })
        .unwrap()
    };
    shell.update(cx, |shell, cx| {
        shell.attach_dictionary(dictionary.clone(), cx);
        shell.select(View::Dictionary, cx);
    });
    let rig = Rig {
        handle,
        dictionary,
        storage,
        _tmp: tmp,
    };
    frame(cx, &rig);
    rig
}

fn frame(cx: &mut TestAppContext, rig: &Rig) {
    cx.run_until_parked();
    cx.update_window(rig.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, rig: &Rig, id: &'static str) {
    cx.update_window(rig.handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    frame(cx, rig);
}

fn press(cx: &mut TestAppContext, rig: &Rig, key: &'static str) {
    cx.update_window(rig.handle, |_, window, cx| window.press(key, cx))
        .unwrap();
    frame(cx, rig);
}

fn type_text(cx: &mut TestAppContext, rig: &Rig, text: &str) {
    cx.update_window(rig.handle, |_, window, cx| window.input(text, cx))
        .unwrap();
    frame(cx, rig);
}

fn present(cx: &mut TestAppContext, rig: &Rig, id: &'static str) -> bool {
    cx.update_window(rig.handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn focused(cx: &mut TestAppContext, rig: &Rig, id: &'static str) -> bool {
    cx.update_window(rig.handle, |_, window, _| {
        window
            .try_find(id)
            .is_some_and(|found| found.focused() == Some(true))
    })
    .unwrap()
}

fn rows(rig: &Rig) -> Vec<(String, Option<String>)> {
    store::list(&rig.storage.database)
        .unwrap()
        .into_iter()
        .map(|entry| (entry.phrase, entry.heard_as))
        .collect()
}

fn message(cx: &mut TestAppContext, rig: &Rig) -> Option<String> {
    rig.dictionary
        .read_with(cx, |dictionary, _| dictionary.message().map(str::to_owned))
}

fn form(cx: &mut TestAppContext, rig: &Rig) -> (String, String) {
    let state = cx.update(|cx| rig.dictionary.read(cx).state_json(cx));
    (
        state["form"]["phrase"].as_str().unwrap().to_owned(),
        state["form"]["heard_as"].as_str().unwrap().to_owned(),
    )
}

/// Types into the form like a user: focus a field, type, and press Enter.
fn add_by_keyboard(cx: &mut TestAppContext, rig: &Rig, phrase: &str, heard: &str) {
    click(cx, rig, "dictionary.phrase");
    type_text(cx, rig, phrase);
    if !heard.is_empty() {
        press(cx, rig, "tab");
        type_text(cx, rig, heard);
    }
    press(cx, rig, "enter");
}

#[gpui_kit::test]
fn an_empty_dictionary_shows_an_empty_state_and_the_add_form(cx: &mut TestAppContext) {
    let rig = open(cx);
    for id in [
        "dictionary.list",
        "dictionary.empty",
        "dictionary.phrase",
        "dictionary.heard",
        "dictionary.submit",
    ] {
        assert!(present(cx, &rig, id), "{id}");
    }
    assert!(!present(cx, &rig, "dictionary.row.0"));
    assert!(!present(cx, &rig, "dictionary.cancel"));
    assert!(!present(cx, &rig, "dictionary.message"));
}

#[gpui_kit::test]
fn a_word_and_a_replacement_are_added_with_the_keyboard_and_listed(cx: &mut TestAppContext) {
    let rig = open(cx);

    add_by_keyboard(cx, &rig, "Zyxtrel", "");
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");

    assert_eq!(
        rows(&rig),
        [
            ("Zyxtrel".to_owned(), None),
            ("Foxtrel".to_owned(), Some("fox".to_owned()))
        ]
    );
    assert!(!present(cx, &rig, "dictionary.empty"));
    assert!(present(cx, &rig, "dictionary.row.0"));
    assert!(present(cx, &rig, "dictionary.row.1"));
    assert!(!present(cx, &rig, "dictionary.row.2"));
    assert_eq!(
        form(cx, &rig),
        (String::new(), String::new()),
        "the form is empty again"
    );
    assert!(
        focused(cx, &rig, "dictionary.phrase"),
        "focus is back on the first field for the next entry"
    );
}

#[gpui_kit::test]
fn the_entries_are_listed_again_after_a_restart(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Zyxtrel", "");
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");

    let reopened = {
        let storage = Rc::clone(&rig.storage);
        cx.update_window(rig.handle, move |_, window, cx| {
            cx.new(|cx| Dictionary::new(storage, window, cx))
        })
        .unwrap()
    };
    let entries = reopened.read_with(cx, |dictionary, _| dictionary.entries().to_vec());
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].phrase, "Zyxtrel");
    assert_eq!(entries[1].heard_as.as_deref(), Some("fox"));
}

#[gpui_kit::test]
fn tab_moves_from_the_first_field_to_the_second_and_then_to_add(cx: &mut TestAppContext) {
    let rig = open(cx);
    click(cx, &rig, "dictionary.phrase");
    assert!(focused(cx, &rig, "dictionary.phrase"));
    press(cx, &rig, "tab");
    assert!(focused(cx, &rig, "dictionary.heard"));
    press(cx, &rig, "tab");
    assert!(focused(cx, &rig, "dictionary.submit"));
    press(cx, &rig, "enter");
    assert!(
        message(cx, &rig).is_some(),
        "Enter on Add with an empty form is refused"
    );
}

#[gpui_kit::test]
fn an_empty_phrase_is_refused_with_a_message_and_saves_nothing(cx: &mut TestAppContext) {
    let rig = open(cx);

    click(cx, &rig, "dictionary.phrase");
    press(cx, &rig, "enter");

    assert!(rows(&rig).is_empty());
    assert!(present(cx, &rig, "dictionary.message"));
    assert_eq!(
        message(cx, &rig).as_deref(),
        Some("Enter the word or phrase to write.")
    );

    click(cx, &rig, "dictionary.submit");
    assert!(rows(&rig).is_empty());
    assert!(present(cx, &rig, "dictionary.message"));
}

#[gpui_kit::test]
fn a_duplicate_phrase_is_refused_in_any_case_and_the_message_names_it(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "fox", "");

    add_by_keyboard(cx, &rig, "FOX", "");

    assert_eq!(rows(&rig), [("fox".to_owned(), None)]);
    assert_eq!(
        message(cx, &rig).as_deref(),
        Some("\u{201c}FOX\u{201d} is already in the dictionary.")
    );
    assert_eq!(
        form(cx, &rig).0,
        "FOX",
        "a refused form keeps what the user typed"
    );
}

#[gpui_kit::test]
fn a_saved_entry_clears_an_earlier_message(cx: &mut TestAppContext) {
    let rig = open(cx);
    click(cx, &rig, "dictionary.phrase");
    press(cx, &rig, "enter");
    assert!(present(cx, &rig, "dictionary.message"));

    type_text(cx, &rig, "Zyxtrel");
    press(cx, &rig, "enter");

    assert!(!present(cx, &rig, "dictionary.message"));
    assert_eq!(rows(&rig).len(), 1);
}

#[gpui_kit::test]
fn edit_puts_the_entry_in_the_form_and_save_changes_it(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");

    click(cx, &rig, "dictionary.edit.0");
    assert_eq!(form(cx, &rig), ("Foxtrel".to_owned(), "fox".to_owned()));
    assert!(present(cx, &rig, "dictionary.cancel"));
    assert!(focused(cx, &rig, "dictionary.phrase"));

    // Edit selects the phrase, so typing replaces it.
    type_text(cx, &rig, "Vixen");
    press(cx, &rig, "enter");

    assert_eq!(rows(&rig), [("Vixen".to_owned(), Some("fox".to_owned()))]);
    assert!(!present(cx, &rig, "dictionary.cancel"));
    assert_eq!(form(cx, &rig), (String::new(), String::new()));
    let editing = rig
        .dictionary
        .read_with(cx, |dictionary, _| dictionary.editing());
    assert_eq!(editing, None);
}

#[gpui_kit::test]
fn cancel_leaves_the_entry_and_empties_the_form(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");
    click(cx, &rig, "dictionary.edit.0");

    click(cx, &rig, "dictionary.cancel");

    assert_eq!(rows(&rig), [("Foxtrel".to_owned(), Some("fox".to_owned()))]);
    assert_eq!(form(cx, &rig), (String::new(), String::new()));
    assert!(!present(cx, &rig, "dictionary.cancel"));
}

#[gpui_kit::test]
fn an_edit_may_not_take_the_phrase_of_another_entry(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Alpha", "");
    add_by_keyboard(cx, &rig, "Beta", "");
    click(cx, &rig, "dictionary.edit.1");
    type_text(cx, &rig, "alpha");
    press(cx, &rig, "enter");

    assert_eq!(rows(&rig)[1].0, "Beta");
    assert!(
        message(cx, &rig)
            .unwrap()
            .contains("already in the dictionary")
    );
}

#[gpui_kit::test]
fn delete_removes_the_row_and_the_stored_entry(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Zyxtrel", "");
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");

    click(cx, &rig, "dictionary.delete.0");

    assert_eq!(rows(&rig), [("Foxtrel".to_owned(), Some("fox".to_owned()))]);
    assert!(present(cx, &rig, "dictionary.row.0"));
    assert!(!present(cx, &rig, "dictionary.row.1"));

    click(cx, &rig, "dictionary.delete.0");
    assert!(rows(&rig).is_empty());
    assert!(present(cx, &rig, "dictionary.empty"));
}

#[gpui_kit::test]
fn deleting_the_entry_being_edited_ends_the_edit(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");
    click(cx, &rig, "dictionary.edit.0");

    click(cx, &rig, "dictionary.delete.0");

    assert!(!present(cx, &rig, "dictionary.cancel"));
    let editing = rig
        .dictionary
        .read_with(cx, |dictionary, _| dictionary.editing());
    assert_eq!(editing, None);
}

#[gpui_kit::test]
fn a_row_shows_the_phrase_and_how_it_was_heard(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Zyxtrel", "");
    add_by_keyboard(cx, &rig, "café-ß-🎤", "dog");

    let (name_0, detail_0, name_1, detail_1) = cx
        .update_window(rig.handle, |_, window, _| {
            (
                window.find("dictionary.name.0").label().map(str::to_owned),
                window
                    .find("dictionary.detail.0")
                    .label()
                    .map(str::to_owned),
                window.find("dictionary.name.1").label().map(str::to_owned),
                window
                    .find("dictionary.detail.1")
                    .label()
                    .map(str::to_owned),
            )
        })
        .unwrap();
    assert_eq!(name_0.as_deref(), Some("Zyxtrel"));
    assert_eq!(detail_0.as_deref(), Some("Word"));
    assert_eq!(name_1.as_deref(), Some("café-ß-🎤"));
    assert_eq!(detail_1.as_deref(), Some("Heard as \u{201c}dog\u{201d}"));
}

#[gpui_kit::test]
fn the_state_json_lists_the_entries_the_form_and_the_message(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");
    click(cx, &rig, "dictionary.edit.0");

    let state = cx.update(|cx| rig.dictionary.read(cx).state_json(cx));

    assert_eq!(state["count"], 1);
    assert_eq!(state["entries"][0]["phrase"], "Foxtrel");
    assert_eq!(state["entries"][0]["heard_as"], "fox");
    assert_eq!(state["editing"], state["entries"][0]["id"]);
    assert_eq!(state["form"]["phrase"], "Foxtrel");
    assert!(state["message"].is_null());
    assert_eq!(state["read_only"], false);
}

#[gpui_kit::test]
fn hook_style_calls_without_a_window_add_edit_and_delete(cx: &mut TestAppContext) {
    let rig = open(cx);
    rig.dictionary
        .update(cx, |dictionary, cx| dictionary.add("Foxtrel", "fox", cx))
        .unwrap();
    let id = rig.dictionary.read_with(cx, |d, _| d.entries()[0].id);
    rig.dictionary
        .update(cx, |dictionary, cx| {
            dictionary.update(id, "Vixen", "fox", cx)
        })
        .unwrap();
    assert_eq!(rows(&rig), [("Vixen".to_owned(), Some("fox".to_owned()))]);
    let refused = rig
        .dictionary
        .update(cx, |dictionary, cx| dictionary.add("", "fox", cx));
    assert_eq!(refused.unwrap_err(), "Enter the word or phrase to write.");
    rig.dictionary
        .update(cx, |dictionary, cx| dictionary.remove(id, cx))
        .unwrap();
    assert!(rows(&rig).is_empty());
}

#[gpui_kit::test]
fn the_panel_fits_the_content_width_and_every_control_is_inside_it(cx: &mut TestAppContext) {
    let rig = open(cx);
    add_by_keyboard(cx, &rig, "Foxtrel", "fox");
    let (list, submit, edit, delete) = cx
        .update_window(rig.handle, |_, window, _| {
            (
                window.find("dictionary.list").bounds(),
                window.find("dictionary.submit").bounds(),
                window.find("dictionary.edit.0").bounds(),
                window.find("dictionary.delete.0").bounds(),
            )
        })
        .unwrap();
    assert!(f32::from(list.size.width) <= space::CONTENT_MAX_WIDTH);
    for (name, bounds) in [("submit", submit), ("edit", edit), ("delete", delete)] {
        assert!(bounds.right() <= list.right(), "{name} is inside the panel");
        assert!(bounds.left() >= list.left(), "{name} is inside the panel");
    }
}
