use super::*;
use crate::storage;
use gpui_kit::{AppContext as _, Entity, TestAppContext};
use hushpen_core::shortcut::{HandsFree, Modifier};
use hushpen_store::data_dir::DataDir;
use std::cell::RefCell;

const LINUX: Platform = Platform::Linux;

#[derive(Default)]
struct FakeKeys {
    pushed: RefCell<Vec<Bindings>>,
    recording: RefCell<i32>,
    stopped: RefCell<i32>,
    held: RefCell<Vec<Combo>>,
}

impl KeyControl for FakeKeys {
    fn set_bindings(&self, bindings: Bindings) {
        self.pushed.borrow_mut().push(bindings);
    }

    fn start_recording(&self) {
        *self.recording.borrow_mut() += 1;
    }

    fn stop_recording(&self) {
        *self.stopped.borrow_mut() += 1;
    }

    fn conflict(&self, combo: &Combo) -> Option<Conflict> {
        self.held.borrow().contains(combo).then(|| Conflict {
            message: "Another app or the desktop uses this shortcut.".to_owned(),
            steps: "Remove it in System Settings > Keyboard > Shortcuts. Then it works here."
                .to_owned(),
            open: Some(Opener::Command(vec!["true".to_owned()])),
        })
    }
}

impl FakeKeys {
    fn live(&self) -> Bindings {
        self.pushed.borrow().last().cloned().unwrap_or_default()
    }
}

struct Rig {
    shortcuts: Entity<Shortcuts>,
    keys: Rc<FakeKeys>,
    storage: Rc<storage::Storage>,
    _tmp: tempfile::TempDir,
}

fn storage_in(tmp: &tempfile::TempDir) -> Rc<storage::Storage> {
    Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap())
}

fn combo(text: &str) -> Combo {
    Combo::parse(text, LINUX).unwrap()
}

fn rig_with(cx: &mut TestAppContext, held: &[&str], saved: &[(&str, &str)]) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = storage_in(&tmp);
    // The store seeds the defaults of the system the test runs on; the rig is a Linux session.
    for slot in Slot::ALL {
        storage
            .settings
            .set(slot.setting_key(), json!(slot.default_text(LINUX)))
            .unwrap();
    }
    for (key, text) in saved {
        storage.settings.set(key, json!(text)).unwrap();
    }
    let keys = Rc::new(FakeKeys::default());
    *keys.held.borrow_mut() = held.iter().map(|text| combo(text)).collect();
    let shortcuts = cx.new(|cx| Shortcuts::new(Rc::clone(&storage), LINUX, cx));
    let control: Rc<dyn KeyControl> = keys.clone();
    shortcuts.update(cx, |shortcuts, cx| shortcuts.attach_keys(Ok(control), cx));
    Rig {
        shortcuts,
        keys,
        storage,
        _tmp: tmp,
    }
}

fn rig(cx: &mut TestAppContext) -> Rig {
    rig_with(cx, &[], &[])
}

fn record(cx: &mut TestAppContext, rig: &Rig, slot: Slot, keys: &[Key]) {
    rig.shortcuts
        .update(cx, |shortcuts, cx| shortcuts.start_recording(slot, cx))
        .unwrap();
    let sink = rig
        .shortcuts
        .read_with(cx, |shortcuts, _| shortcuts.record_sink());
    sink(Recording::Progress(keys.to_vec()));
    sink(Recording::Captured(keys.to_vec()));
    cx.run_until_parked();
}

fn state(cx: &mut TestAppContext, rig: &Rig) -> Value {
    rig.shortcuts
        .read_with(cx, |shortcuts, _| shortcuts.state_json())
}

fn saved_text(rig: &Rig, slot: Slot) -> String {
    rig.storage
        .settings
        .get(slot.setting_key())
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap()
}

const RIGHT_CTRL: Key = Key::Right(Modifier::Ctrl);

fn chord(letter: char) -> [Key; 3] {
    [
        Key::Any(Modifier::Ctrl),
        Key::Any(Modifier::Alt),
        Key::Char(letter),
    ]
}

#[gpui_kit::test]
fn the_defaults_read_with_linux_names_and_no_mac_words(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let state = state(cx, &rig);
    assert_eq!(state["slots"]["hold"]["display"], "Right Alt");
    assert_eq!(state["slots"]["pasteLast"]["display"], "Ctrl+Alt+V");
    assert_eq!(
        state["slots"]["command"]["display"],
        "Right Alt+Right Shift"
    );
    let text = state.to_string();
    for banned in ["⌘", "⌥", "⌃", "⇧", "Option", "Command"] {
        assert!(!text.contains(banned), "{banned} in {text}");
    }
    assert_eq!(state["recorder_available"], true);
}

#[gpui_kit::test]
fn a_binding_the_field_may_cut_shows_whole_in_a_tooltip(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let state = state(cx, &rig);
    let shown = |slot: &str| state["slots"][slot]["display"].as_str().unwrap().to_owned();
    assert_eq!(shown("handsFree"), "Right Alt ×2 or Right Alt+Space");
    assert!(panel::needs_tooltip(&shown("handsFree")));
    for short in ["hold", "pasteLast", "command"] {
        assert!(!panel::needs_tooltip(&shown(short)), "{short}");
    }
}

#[gpui_kit::test]
fn a_recorded_hold_key_is_saved_shown_and_made_live(cx: &mut TestAppContext) {
    let rig = rig(cx);
    record(cx, &rig, Slot::Hold, &[RIGHT_CTRL]);

    assert_eq!(saved_text(&rig, Slot::Hold), "RightCtrl");
    let state = state(cx, &rig);
    assert_eq!(state["slots"]["hold"]["display"], "Right Ctrl");
    assert_eq!(state["slots"]["hold"]["error"], Value::Null);
    assert_eq!(state["recording"], Value::Null);
    assert_eq!(rig.keys.live().hold, Some(combo("RightCtrl")));
}

#[gpui_kit::test]
fn chords_for_the_other_three_slots_are_saved(cx: &mut TestAppContext) {
    let rig = rig(cx);
    record(cx, &rig, Slot::HandsFree, &chord('h'));
    record(cx, &rig, Slot::PasteLast, &chord('p'));
    record(
        cx,
        &rig,
        Slot::Command,
        &[RIGHT_CTRL, Key::Right(Modifier::Shift)],
    );

    assert_eq!(saved_text(&rig, Slot::HandsFree), "Ctrl+Alt+H");
    assert_eq!(saved_text(&rig, Slot::PasteLast), "Ctrl+Alt+P");
    assert_eq!(saved_text(&rig, Slot::Command), "RightCtrl+RightShift");
    let live = rig.keys.live();
    assert_eq!(
        live.hands_free,
        Some(HandsFree::Custom(combo("Ctrl+Alt+H")))
    );
    assert_eq!(live.paste_last, Some(combo("Ctrl+Alt+P")));
    assert_eq!(live.command, Some(combo("RightCtrl+RightShift")));
}

#[gpui_kit::test]
fn a_chord_another_app_holds_is_saved_with_its_steps_and_works_once_freed(cx: &mut TestAppContext) {
    let rig = rig_with(cx, &["Ctrl+Alt+J"], &[]);
    record(cx, &rig, Slot::PasteLast, &chord('j'));

    let state = state(cx, &rig);
    let error = &state["slots"]["pasteLast"]["error"];
    assert_eq!(error["code"], "SHORTCUT_IN_USE");
    assert!(error["steps"].as_str().unwrap().contains("Remove it"));
    assert_eq!(error["can_open"], true);
    assert_eq!(saved_text(&rig, Slot::PasteLast), "Ctrl+Alt+J");
    assert_eq!(state["slots"]["pasteLast"]["live"], false);
    assert_eq!(rig.keys.live().paste_last, None);

    rig.keys.held.borrow_mut().clear();
    cx.executor()
        .advance_clock(RECHECK + Duration::from_millis(100));
    cx.run_until_parked();
    let state = self::state(cx, &rig);
    assert_eq!(state["slots"]["pasteLast"]["error"], Value::Null);
    assert_eq!(state["slots"]["pasteLast"]["live"], true);
    assert_eq!(rig.keys.live().paste_last, Some(combo("Ctrl+Alt+J")));
}

#[gpui_kit::test]
fn only_a_chord_another_slot_holds_is_refused(cx: &mut TestAppContext) {
    let rig = rig(cx);
    record(cx, &rig, Slot::HandsFree, &chord('v'));
    let first = state(cx, &rig);
    assert_eq!(
        first["slots"]["handsFree"]["error"]["code"],
        "SHORTCUT_RESERVED"
    );
    assert_eq!(saved_text(&rig, Slot::HandsFree), "DoubleTap+Hold+Space");

    // A chord every app uses is the user's choice.
    record(
        cx,
        &rig,
        Slot::HandsFree,
        &[Key::Any(Modifier::Ctrl), Key::Char('q')],
    );
    let second = state(cx, &rig);
    assert_eq!(second["slots"]["handsFree"]["error"], Value::Null);
    assert_eq!(saved_text(&rig, Slot::HandsFree), "Ctrl+Q");
    assert_eq!(
        rig.keys.live().hands_free,
        Some(HandsFree::Custom(combo("Ctrl+Q")))
    );
}

#[gpui_kit::test]
fn a_single_key_or_a_media_key_can_be_the_hold_key(cx: &mut TestAppContext) {
    let rig = rig(cx);
    record(cx, &rig, Slot::Hold, &[Key::Char('a')]);
    assert_eq!(saved_text(&rig, Slot::Hold), "A");
    assert_eq!(rig.keys.live().hold, Some(combo("A")));
    record(
        cx,
        &rig,
        Slot::Hold,
        &[Key::Named(hushpen_core::shortcut::Named::PlayPause)],
    );
    assert_eq!(saved_text(&rig, Slot::Hold), "PlayPause");
    assert_eq!(state(cx, &rig)["slots"]["hold"]["display"], "Play/Pause");
}

#[gpui_kit::test]
fn esc_closes_the_recorder_and_changes_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.shortcuts
        .update(cx, |shortcuts, cx| {
            shortcuts.start_recording(Slot::Hold, cx)
        })
        .unwrap();
    assert_eq!(state(cx, &rig)["recording"], "hold");
    assert_eq!(*rig.keys.recording.borrow(), 1);

    let sink = rig
        .shortcuts
        .read_with(cx, |shortcuts, _| shortcuts.record_sink());
    sink(Recording::Cancelled);
    cx.run_until_parked();

    assert_eq!(state(cx, &rig)["recording"], Value::Null);
    assert_eq!(saved_text(&rig, Slot::Hold), "RightAlt");
}

#[gpui_kit::test]
fn the_field_shows_the_keys_pressed_so_far(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.shortcuts
        .update(cx, |shortcuts, cx| {
            shortcuts.start_recording(Slot::PasteLast, cx)
        })
        .unwrap();
    let sink = rig
        .shortcuts
        .read_with(cx, |shortcuts, _| shortcuts.record_sink());
    sink(Recording::Progress(vec![
        Key::Any(Modifier::Ctrl),
        Key::Any(Modifier::Alt),
    ]));
    cx.run_until_parked();
    assert_eq!(state(cx, &rig)["progress"], "Ctrl+Alt");
}

#[gpui_kit::test]
fn a_click_on_the_open_field_closes_the_recorder_and_tells_the_listener(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.shortcuts
        .update(cx, |shortcuts, cx| {
            shortcuts.start_recording(Slot::Hold, cx)
        })
        .unwrap();
    rig.shortcuts
        .update(cx, |shortcuts, cx| shortcuts.cancel_recording(cx));
    assert_eq!(state(cx, &rig)["recording"], Value::Null);
    assert_eq!(*rig.keys.stopped.borrow(), 1);
}

#[gpui_kit::test]
fn a_recorder_nobody_uses_closes_by_itself(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.shortcuts
        .update(cx, |shortcuts, cx| {
            shortcuts.start_recording(Slot::Hold, cx)
        })
        .unwrap();
    cx.executor()
        .advance_clock(RECORDER_IDLE + Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(state(cx, &rig)["recording"], Value::Null);
    assert_eq!(*rig.keys.stopped.borrow(), 1);
}

#[gpui_kit::test]
fn reset_puts_all_four_back_to_the_defaults(cx: &mut TestAppContext) {
    let rig = rig(cx);
    record(cx, &rig, Slot::Hold, &[RIGHT_CTRL]);
    record(cx, &rig, Slot::HandsFree, &chord('h'));
    record(cx, &rig, Slot::PasteLast, &chord('p'));
    record(
        cx,
        &rig,
        Slot::Command,
        &[RIGHT_CTRL, Key::Right(Modifier::Shift)],
    );

    rig.shortcuts
        .update(cx, |shortcuts, cx| shortcuts.reset_all(cx))
        .unwrap();

    for slot in Slot::ALL {
        assert_eq!(saved_text(&rig, slot), slot.default_text(LINUX), "{slot:?}");
    }
    let live = rig.keys.live();
    assert_eq!(live.hold, Some(combo("RightAlt")));
    assert_eq!(live.hands_free, Some(HandsFree::Default));
}

fn switch(cx: &mut TestAppContext, rig: &Rig, slot: Slot, on: bool) -> Result<(), String> {
    rig.shortcuts
        .update(cx, |shortcuts, cx| shortcuts.set_on(slot, on, cx))
}

#[gpui_kit::test]
fn an_off_shortcut_stops_running_keeps_its_keys_and_survives_a_restart(cx: &mut TestAppContext) {
    let rig = rig(cx);
    switch(cx, &rig, Slot::PasteLast, false).unwrap();
    switch(cx, &rig, Slot::Command, false).unwrap();
    let live = rig.keys.live();
    assert_eq!(live.paste_last, None);
    assert_eq!(live.command, None);
    assert_eq!(live.hold, Some(combo("RightAlt")));
    assert_eq!(saved_text(&rig, Slot::PasteLast), "Ctrl+Alt+V");
    assert_eq!(
        rig.storage.settings.get(ENABLED_SETTING),
        Some(json!({"pasteLast": false, "command": false}))
    );
    let state = state(cx, &rig);
    assert_eq!(state["slots"]["pasteLast"]["on"], false);
    assert_eq!(state["slots"]["pasteLast"]["live"], false);
    assert_eq!(state["slots"]["pasteLast"]["display"], "Ctrl+Alt+V");

    let again = cx.new(|cx| Shortcuts::new(Rc::clone(&rig.storage), LINUX, cx));
    assert!(!again.read_with(cx, |shortcuts, _| shortcuts.is_on(Slot::PasteLast)));
    assert_eq!(
        again
            .read_with(cx, |shortcuts, _| shortcuts.bindings())
            .paste_last,
        None
    );

    switch(cx, &rig, Slot::PasteLast, true).unwrap();
    assert_eq!(rig.keys.live().paste_last, Some(combo("Ctrl+Alt+V")));
}

#[gpui_kit::test]
fn with_the_hold_key_off_the_key_stays_live_for_hands_free_only(cx: &mut TestAppContext) {
    let rig = rig(cx);
    switch(cx, &rig, Slot::Hold, false).unwrap();
    assert_eq!(rig.keys.live().hold, Some(combo("RightAlt")));
    assert_eq!(rig.keys.live().hands_free, Some(HandsFree::Default));
    switch(cx, &rig, Slot::HandsFree, false).unwrap();
    assert_eq!(
        rig.keys.live(),
        Bindings {
            paste_last: Some(combo("Ctrl+Alt+V")),
            command: Some(combo("RightAlt+RightShift")),
            read_aloud: Some(combo("Ctrl+Alt+S")),
            ..Bindings::default()
        }
    );
}

#[gpui_kit::test]
fn an_off_shortcut_cannot_be_recorded(cx: &mut TestAppContext) {
    let rig = rig(cx);
    switch(cx, &rig, Slot::PasteLast, false).unwrap();
    let opened = rig.shortcuts.update(cx, |shortcuts, cx| {
        shortcuts.start_recording(Slot::PasteLast, cx)
    });
    assert!(opened.is_err());
    assert_eq!(*rig.keys.recording.borrow(), 0);
}

#[gpui_kit::test]
fn the_keys_of_an_off_shortcut_can_go_to_another_and_then_it_will_not_turn_on(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    switch(cx, &rig, Slot::PasteLast, false).unwrap();
    record(cx, &rig, Slot::HandsFree, &chord('v'));
    assert_eq!(saved_text(&rig, Slot::HandsFree), "Ctrl+Alt+V");
    let refused = switch(cx, &rig, Slot::PasteLast, true).unwrap_err();
    assert!(refused.contains("hands-free"), "{refused}");
    let state = state(cx, &rig);
    assert_eq!(state["slots"]["pasteLast"]["on"], false);
    assert_eq!(
        state["slots"]["pasteLast"]["error"]["code"],
        "SHORTCUT_RESERVED"
    );
}

#[gpui_kit::test]
fn reset_puts_one_shortcut_back_and_leaves_the_others(cx: &mut TestAppContext) {
    let rig = rig(cx);
    record(cx, &rig, Slot::PasteLast, &chord('p'));
    record(cx, &rig, Slot::Hold, &[RIGHT_CTRL]);
    assert_eq!(state(cx, &rig)["slots"]["pasteLast"]["default"], false);
    rig.shortcuts
        .update(cx, |shortcuts, cx| shortcuts.reset(Slot::PasteLast, cx))
        .unwrap();
    assert_eq!(saved_text(&rig, Slot::PasteLast), "Ctrl+Alt+V");
    assert_eq!(saved_text(&rig, Slot::Hold), "RightCtrl");
    assert_eq!(rig.keys.live().paste_last, Some(combo("Ctrl+Alt+V")));
    assert_eq!(state(cx, &rig)["slots"]["pasteLast"]["default"], true);
}

#[gpui_kit::test]
fn reset_all_turns_every_shortcut_on_again(cx: &mut TestAppContext) {
    let rig = rig(cx);
    switch(cx, &rig, Slot::Command, false).unwrap();
    rig.shortcuts
        .update(cx, |shortcuts, cx| shortcuts.reset_all(cx))
        .unwrap();
    assert_eq!(rig.storage.settings.get(ENABLED_SETTING), Some(json!({})));
    assert_eq!(rig.keys.live().command, Some(combo("RightAlt+RightShift")));
}

#[gpui_kit::test]
fn a_chord_held_by_another_app_at_start_shows_the_error_and_the_hold_key_still_runs(
    cx: &mut TestAppContext,
) {
    let rig = rig_with(cx, &["Ctrl+Alt+V"], &[]);
    let state = state(cx, &rig);
    assert_eq!(
        state["slots"]["pasteLast"]["error"]["code"],
        "SHORTCUT_IN_USE"
    );
    assert_eq!(state["slots"]["pasteLast"]["live"], false);
    assert_eq!(state["slots"]["pasteLast"]["setting"], "Ctrl+Alt+V");
    assert_eq!(state["slots"]["hold"]["live"], true);
    let live = rig.keys.live();
    assert_eq!(live.paste_last, None);
    assert_eq!(live.hold, Some(combo("RightAlt")));
}

#[gpui_kit::test]
fn a_saved_value_that_is_not_a_shortcut_falls_back_to_the_default(cx: &mut TestAppContext) {
    let rig = rig_with(
        cx,
        &[],
        &[("shortcut.pasteLast", "Nope"), ("shortcut.hold", "Fn")],
    );
    let state = state(cx, &rig);
    assert_eq!(state["slots"]["pasteLast"]["setting"], "Ctrl+Alt+V");
    assert_eq!(state["slots"]["hold"]["setting"], "RightAlt");
}

#[gpui_kit::test]
fn a_saved_shortcut_is_what_the_listener_starts_with(cx: &mut TestAppContext) {
    let tmp = tempfile::tempdir().unwrap();
    let storage = storage_in(&tmp);
    storage
        .settings
        .set("shortcut.hold", json!("RightCtrl"))
        .unwrap();
    let shortcuts = cx.new(|cx| Shortcuts::new(Rc::clone(&storage), LINUX, cx));
    let bindings = shortcuts.read_with(cx, |shortcuts, _| shortcuts.bindings());
    assert_eq!(bindings.hold, Some(combo("RightCtrl")));
    assert_eq!(bindings.hands_free, Some(HandsFree::Default));
}

#[gpui_kit::test]
fn with_no_key_listener_nothing_can_be_recorded_and_the_reason_shows(cx: &mut TestAppContext) {
    let tmp = tempfile::tempdir().unwrap();
    let storage = storage_in(&tmp);
    let shortcuts = cx.new(|cx| Shortcuts::new(Rc::clone(&storage), LINUX, cx));
    shortcuts.update(cx, |shortcuts, cx| {
        shortcuts.attach_keys(Err("Not available on Wayland.".to_owned()), cx)
    });
    let opened = shortcuts.update(cx, |shortcuts, cx| {
        shortcuts.start_recording(Slot::Hold, cx)
    });
    assert_eq!(opened, Err("Not available on Wayland.".to_owned()));
    let state = shortcuts.read_with(cx, |shortcuts, _| shortcuts.state_json());
    assert_eq!(state["recorder_available"], false);
    assert_eq!(state["slots"]["hold"]["display"], "Right Alt");
}

mod view {
    use super::*;
    use crate::shell::Shell;
    use crate::views::View;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AnyWindowHandle, Bounds, Point, WindowBounds, WindowOptions, px, size};

    fn open(cx: &mut TestAppContext, rig: &Rig) -> (AnyWindowHandle, Entity<Shell>) {
        cx.update(gpui_kit::init);
        let (handle, shell) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(940.0), px(600.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Shell::new(window, cx)),
            )
            .expect("open test window")
        });
        let storage = Rc::clone(&rig.storage);
        let settings = cx.new(|cx| {
            crate::settings::Settings::new(
                crate::settings::Parts {
                    storage,
                    mic: cx.new(|cx| {
                        crate::mic::Mic::new(
                            Rc::clone(&rig.storage),
                            std::sync::Arc::new(crate::controller::testkit::FakeMic {
                                start_error: std::sync::Mutex::new(None),
                                starts: std::sync::atomic::AtomicUsize::new(0),
                                sink: std::sync::Mutex::new(None),
                            }),
                            cx,
                        )
                    }),
                    login: None,
                    default_data_dir: None,
                    restart: None,
                },
                cx,
            )
        });
        settings.update(cx, |settings, cx| settings.set_query("shortcut".into(), cx));
        shell.update(cx, |shell, cx| {
            shell.attach_settings(settings, cx);
            shell.attach_shortcuts(rig.shortcuts.clone(), cx);
            shell.select(View::Settings, cx);
        });
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        (handle, shell)
    }

    fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
        cx.update_window(handle, |_, window, _| {
            window
                .try_find(id)
                .and_then(|element| element.label().map(str::to_owned))
        })
        .unwrap()
    }

    fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
        cx.update_window(handle, |_, window, cx| window.click(id, cx))
            .unwrap();
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn settings_shows_the_four_shortcuts_with_linux_names(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let (handle, _shell) = open(cx, &rig);
        assert_eq!(
            label(cx, handle, "settings.shortcut.hold").as_deref(),
            Some("Right Alt")
        );
        assert_eq!(
            label(cx, handle, "settings.shortcut.pasteLast").as_deref(),
            Some("Ctrl+Alt+V")
        );
        assert_eq!(
            label(cx, handle, "settings.shortcut.command").as_deref(),
            Some("Right Alt+Right Shift")
        );
        assert!(
            label(cx, handle, "settings.shortcut.handsFree")
                .unwrap()
                .contains("Right Alt+Space")
        );
    }

    #[gpui_kit::test]
    fn clicking_a_field_opens_the_recorder_and_shows_the_keys_and_the_refusal(
        cx: &mut TestAppContext,
    ) {
        let rig = rig_with(cx, &["Ctrl+Alt+J"], &[]);
        let (handle, _shell) = open(cx, &rig);
        click(cx, handle, "settings.shortcut.pasteLast");
        assert_eq!(state(cx, &rig)["recording"], "pasteLast");
        assert_eq!(
            label(cx, handle, "settings.shortcut.pasteLast").as_deref(),
            Some("Press the shortcut")
        );

        let sink = rig
            .shortcuts
            .read_with(cx, |shortcuts, _| shortcuts.record_sink());
        sink(Recording::Captured(chord('j').to_vec()));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        assert_eq!(
            label(cx, handle, "settings.shortcut.pasteLast").as_deref(),
            Some("Ctrl+Alt+J")
        );
        assert!(
            label(cx, handle, "settings.shortcut.pasteLast.error")
                .unwrap()
                .contains("Another app")
        );
        assert!(
            label(cx, handle, "settings.shortcut.pasteLast.steps")
                .unwrap()
                .contains("Then it works here")
        );
        click(cx, handle, "settings.shortcut.pasteLast.open");
    }

    #[gpui_kit::test]
    fn the_reset_button_restores_the_defaults(cx: &mut TestAppContext) {
        let rig = rig(cx);
        record(cx, &rig, Slot::Hold, &[RIGHT_CTRL]);
        let (handle, _shell) = open(cx, &rig);
        assert_eq!(
            label(cx, handle, "settings.shortcut.hold").as_deref(),
            Some("Right Ctrl")
        );
        click(cx, handle, "settings.shortcut.reset");
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        assert_eq!(
            label(cx, handle, "settings.shortcut.hold").as_deref(),
            Some("Right Alt")
        );
    }
}
