use super::*;
use crate::controller::testkit::*;
use gpui_kit::{AppContext as _, TestAppContext};
use hushpen_core::dictation::State;
use hushpen_core::flow_bar::ERROR_HOLD_MS;
use hushpen_core::insert::{Chord, Outcome as InsertOutcome, Report, Restore};
use hushpen_engine::{Failure, JobOutcome};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

const SCREEN: (f32, f32) = (1280.0, 800.0);

struct Bar {
    rig: Rig,
    bar: Entity<FlowBar>,
    /// The bar's own clock.
    now: Rc<Cell<u64>>,
    pointer: Rc<RefCell<Option<Pointer>>>,
    opened: Rc<RefCell<Vec<String>>>,
}

fn bar(cx: &mut TestAppContext, files: &[&str], chosen: &str) -> Bar {
    let rig = rig(cx, files, chosen);
    let bar = build(cx, &rig);
    let now = Rc::new(Cell::new(10_000));
    let pointer = Rc::new(RefCell::new(None));
    let opened: Rc<RefCell<Vec<String>>> = Rc::default();
    bar.update(cx, |bar, _| {
        let clock = Rc::clone(&now);
        bar.clock = Rc::new(move || clock.get());
        let source = Rc::clone(&pointer);
        bar.pointer = Rc::new(move |_| *source.borrow());
        let log = Rc::clone(&opened);
        bar.on_open_history(Rc::new(move |id, _| log.borrow_mut().push(id.to_owned())));
    });
    Bar {
        rig,
        bar,
        now,
        pointer,
        opened,
    }
}

fn build(cx: &mut TestAppContext, rig: &Rig) -> Entity<FlowBar> {
    cx.new(|_| {
        FlowBar::new(
            Rc::clone(&rig.storage),
            rig.controller.clone(),
            rig.dictation.clone(),
            rig.mic.clone(),
        )
    })
}

impl Bar {
    fn tick(&self, cx: &mut TestAppContext) -> Option<Bounds<Pixels>> {
        self.bar.update(cx, |bar, cx| bar.tick(SCREEN, cx))
    }

    fn props(&self, cx: &mut TestAppContext) -> Props {
        self.bar.read_with(cx, |bar, _| bar.props().clone())
    }

    /// The state after one tick.
    fn state(&self, cx: &mut TestAppContext) -> BarState {
        self.tick(cx);
        self.props(cx).state
    }

    fn click(&self, cx: &mut TestAppContext) {
        self.rig.now.set(self.now.get());
        self.bar.update(cx, |bar, cx| {
            bar.press_at(cx);
            bar.release(cx);
        });
    }

    fn at(&self, ms: u64) {
        self.now.set(ms);
        self.rig.now.set(ms);
    }

    fn origin(&self, cx: &mut TestAppContext) -> (f32, f32) {
        self.tick(cx);
        self.bar.read_with(cx, |bar, _| bar.frame().0)
    }

    fn setting(&self, key: &str) -> Option<serde_json::Value> {
        self.rig.storage.settings.get(key)
    }
}

fn rect(bounds: Bounds<Pixels>) -> (f32, f32, f32, f32) {
    (
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    )
}

fn grabbed() -> Arc<FakeInserter> {
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    {
        let mut script = inserter.script.lock().unwrap();
        *script = Report::new("GtkTarget");
        script.outcome = InsertOutcome::BlockedGrab;
        script.code = Some("INSERT_KEYBOARD_GRABBED");
        script.restore = Restore::NotNeeded;
    }
    inserter
}

/// Two clicks with a spoken answer in between.
fn dictate(cx: &mut TestAppContext, bar: &Bar, outcome: JobOutcome) {
    say(&bar.rig, outcome);
    bar.at(11_000);
    bar.click(cx);
    bar.at(14_000);
    bar.click(cx);
    settle(cx, &bar.rig);
}

#[gpui_kit::test]
fn the_idle_bar_sits_at_the_bottom_center_of_the_screen(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    let (x, y, width, height) = rect(bar.tick(cx).expect("the idle bar shows"));
    assert_eq!((x + width / 2.0, y + height), (640.0, 768.0));
    assert_eq!(bar.props(cx).state, BarState::Idle);
}

#[gpui_kit::test]
fn the_top_preset_puts_the_bar_at_the_top_center(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.rig
        .storage
        .settings
        .set("overlay.position", json!("top"))
        .unwrap();
    let (x, y, width, _) = rect(bar.tick(cx).unwrap());
    assert_eq!((x + width / 2.0, y), (640.0, 40.0));
}

#[gpui_kit::test]
fn a_click_starts_and_stops_a_dictation_and_the_bar_follows_each_step(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    say(&bar.rig, outcome_text("hello there", "en"));

    bar.at(11_000);
    bar.click(cx);
    assert_eq!(machine_state(cx, &bar.rig), State::Listening);
    assert_eq!(bar.state(cx), BarState::Listening);

    bar.at(14_000);
    bar.click(cx);
    assert_eq!(bar.state(cx), BarState::Transcribing);

    settle(cx, &bar.rig);
    assert_eq!(bar.state(cx), BarState::Result);
    // No paste path in the rig, so the words only reached the clipboard.
    assert_eq!(bar.props(cx).message, "Copied");
}

#[gpui_kit::test]
fn a_paste_says_inserted(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    attach_inserter(
        cx,
        &bar.rig,
        &FakeInserter::pasted_into("GtkTarget", Chord::CtrlV),
    );
    dictate(cx, &bar, outcome_text("hello there", "en"));
    assert_eq!(bar.state(cx), BarState::Result);
    assert_eq!(bar.props(cx).message, "Inserted");
}

#[gpui_kit::test]
fn the_waveform_follows_the_meter_only_while_listening(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    assert!(bar.state(cx) == BarState::Idle);
    assert!(bar.props(cx).levels.is_empty());
    bar.at(11_000);
    bar.click(cx);
    bar.tick(cx);
    assert_eq!(bar.props(cx).levels.len(), 32);
}

#[gpui_kit::test]
fn no_speech_stays_on_the_bar_with_open_history_after_the_pipeline_is_idle(
    cx: &mut TestAppContext,
) {
    let bar = bar(cx, &["base"], "base");
    dictate(
        cx,
        &bar,
        JobOutcome::Failed(Failure::new("ENGINE_NO_SPEECH", "no audio")),
    );
    assert_eq!(bar.state(cx), BarState::Error);
    let props = bar.props(cx);
    assert_eq!(props.message, "No speech heard");
    assert!(props.open_history);

    // The pipeline clears its failure after two seconds; the bar keeps it.
    bar.at(14_000 + 2_500);
    send_at(cx, &bar.rig, 14_000 + 2_500, AppEvent::Tick).unwrap();
    assert_eq!(machine_state(cx, &bar.rig), State::Idle);
    assert_eq!(bar.state(cx), BarState::Error);
    assert_eq!(bar.props(cx).message, "No speech heard");

    bar.at(14_000 + ERROR_HOLD_MS + 100);
    assert_eq!(bar.state(cx), BarState::Idle);
}

#[gpui_kit::test]
fn a_blocked_paste_shows_the_grab_notice_and_the_open_history_button(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    attach_inserter(cx, &bar.rig, &grabbed());
    dictate(cx, &bar, outcome_text("grabbed words", "en"));
    assert_eq!(bar.state(cx), BarState::Blocked);
    let props = bar.props(cx);
    assert_eq!(props.message, "Another app holds the keyboard");
    assert!(props.open_history);
}

#[gpui_kit::test]
fn open_history_hands_the_failed_row_to_the_main_window_and_clears_the_notice(
    cx: &mut TestAppContext,
) {
    let bar = bar(cx, &["base"], "base");
    dictate(
        cx,
        &bar,
        JobOutcome::Failed(Failure::new("ENGINE_NO_SPEECH", "no audio")),
    );
    assert_eq!(bar.state(cx), BarState::Error);
    let row = bar
        .rig
        .controller
        .read_with(cx, |controller, _| {
            controller.last_row_id().map(str::to_owned)
        })
        .expect("the failed run saved a row");

    bar.bar.update(cx, |bar, cx| bar.open_history(cx));
    cx.run_until_parked();

    assert_eq!(*bar.opened.borrow(), [row]);
    send_at(cx, &bar.rig, 17_000, AppEvent::Tick).unwrap();
    bar.at(17_000);
    assert_eq!(bar.state(cx), BarState::Idle);
}

#[gpui_kit::test]
fn a_click_that_cannot_start_shows_why_and_has_no_history_button(cx: &mut TestAppContext) {
    let bar = bar(cx, &[], "base");
    bar.at(11_000);
    bar.click(cx);
    assert_eq!(bar.state(cx), BarState::Error);
    let props = bar.props(cx);
    assert!(!props.message.is_empty());
    assert!(!props.open_history);

    bar.at(11_000 + ERROR_HOLD_MS + 100);
    assert_eq!(bar.state(cx), BarState::Idle);
}

#[gpui_kit::test]
fn the_idle_pill_can_be_hidden_but_the_other_states_still_show(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.rig
        .storage
        .settings
        .set("overlay.idleVisible", json!(false))
        .unwrap();
    assert!(bar.tick(cx).is_none());

    send_at(cx, &bar.rig, 11_000, AppEvent::FlowBarClick).unwrap();
    assert!(bar.tick(cx).is_some(), "the listening bar shows");
}

#[gpui_kit::test]
fn turning_the_overlay_off_hides_every_state(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.rig
        .storage
        .settings
        .set("overlay.enabled", json!(false))
        .unwrap();
    assert!(bar.tick(cx).is_none());
    send_at(cx, &bar.rig, 11_000, AppEvent::FlowBarClick).unwrap();
    assert!(bar.tick(cx).is_none());
}

#[gpui_kit::test]
fn the_language_chip_opens_the_list_and_a_choice_sets_the_language(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.tick(cx);
    assert_eq!(bar.props(cx).language.label, "AUTO");
    assert!(bar.props(cx).language.enabled);

    bar.bar.update(cx, |bar, cx| bar.toggle_picker(cx));
    bar.tick(cx);
    let Picker::List {
        codes, selected, ..
    } = bar.props(cx).picker
    else {
        panic!("the list is open");
    };
    assert_eq!(codes.first(), Some(&"auto"));
    assert!(codes.contains(&"es"));
    assert_eq!(selected, "auto");

    bar.bar
        .update(cx, |bar, cx| bar.choose_language("es", cx).unwrap());
    assert_eq!(bar.setting("dictation.language"), Some(json!("es")));
    assert_eq!(
        bar.setting("dictation.recentLanguages").unwrap()[0],
        json!("es")
    );
    bar.tick(cx);
    assert_eq!(bar.props(cx).picker, Picker::Closed);
    assert_eq!(bar.props(cx).language.label, "ES");
}

#[gpui_kit::test]
fn the_next_job_carries_the_language_chosen_on_the_bar(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.bar
        .update(cx, |bar, cx| bar.choose_language("es", cx).unwrap());
    dictate(cx, &bar, outcome_text("hola", "es"));
    assert_eq!(
        bar.rig
            .specs
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .language
            .as_deref(),
        Some("es")
    );
}

#[gpui_kit::test]
fn an_english_only_model_turns_the_picker_off_and_says_why(cx: &mut TestAppContext) {
    let bar = bar(cx, &["tiny.en"], "tiny.en");
    bar.tick(cx);
    let language = bar.props(cx).language;
    assert!(!language.enabled);
    let reason = language.reason.expect("a reason");
    assert!(reason.contains("English only"), "{reason}");

    bar.bar.update(cx, |bar, cx| bar.toggle_picker(cx));
    bar.tick(cx);
    assert!(matches!(
        bar.props(cx).picker,
        Picker::Reason { ref text, .. } if *text == reason
    ));
    bar.bar
        .update(cx, |bar, cx| bar.choose_language("es", cx).unwrap_err());
    assert_eq!(bar.setting("dictation.language"), Some(json!("auto")));
}

#[gpui_kit::test]
fn the_list_opens_toward_the_middle_of_the_screen(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.bar.update(cx, |bar, cx| bar.toggle_picker(cx));
    bar.tick(cx);
    let Picker::List { below, .. } = bar.props(cx).picker else {
        panic!("open");
    };
    assert!(!below, "a bar at the bottom opens upward");
    let (_, y, _, height) = rect(bar.tick(cx).unwrap());
    assert_eq!(y + height, 768.0, "the bottom edge stays where it was");

    bar.rig
        .storage
        .settings
        .set("overlay.position", json!("top"))
        .unwrap();
    bar.tick(cx);
    let Picker::List { below, .. } = bar.props(cx).picker else {
        panic!("open");
    };
    assert!(below);
    assert_eq!(rect(bar.tick(cx).unwrap()).1, 40.0);
}

#[gpui_kit::test]
fn the_open_list_closes_by_itself_and_when_a_dictation_starts(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.bar.update(cx, |bar, cx| bar.toggle_picker(cx));
    bar.tick(cx);
    assert_ne!(bar.props(cx).picker, Picker::Closed);
    bar.at(10_000 + PICKER_HOLD_MS + 1);
    bar.tick(cx);
    assert_eq!(bar.props(cx).picker, Picker::Closed);

    bar.bar.update(cx, |bar, cx| bar.toggle_picker(cx));
    bar.click(cx);
    assert_eq!(bar.state(cx), BarState::Listening);
    assert_eq!(bar.props(cx).picker, Picker::Closed);
}

fn pointer_at(bar: &Bar, x: f32, y: f32, left: bool) {
    *bar.pointer.borrow_mut() = Some(Pointer { x, y, left });
}

#[gpui_kit::test]
fn dragging_moves_the_bar_by_the_pointer_offset_and_does_not_start_a_dictation(
    cx: &mut TestAppContext,
) {
    let bar = bar(cx, &["base"], "base");
    let (x0, y0) = bar.origin(cx);
    pointer_at(&bar, 640.0, 754.0, true);
    bar.bar.update(cx, |bar, cx| bar.press_at(cx));

    pointer_at(&bar, 340.0, 554.0, true);
    bar.tick(cx);
    assert_eq!(bar.origin(cx), (x0 - 300.0, y0 - 200.0));
    assert!(bar.bar.read_with(cx, |bar, _| bar.is_pressed()));

    pointer_at(&bar, 340.0, 554.0, false);
    bar.bar.update(cx, |bar, cx| bar.release(cx));

    assert_eq!(machine_state(cx, &bar.rig), State::Idle);
    let spot = bar.setting("overlay.customPos").unwrap();
    assert_eq!(spot["x"], json!(x0 - 300.0 + 64.0));
    assert_eq!(spot["y"], json!(y0 - 200.0 + 28.0));
    assert_eq!(bar.origin(cx), (x0 - 300.0, y0 - 200.0));
}

#[gpui_kit::test]
fn a_dragged_position_survives_a_restart_and_a_preset_clears_it(cx: &mut TestAppContext) {
    let first = bar(cx, &["base"], "base");
    let (x0, y0) = first.origin(cx);
    pointer_at(&first, 100.0, 100.0, true);
    first.bar.update(cx, |bar, cx| bar.press_at(cx));
    pointer_at(&first, 0.0, 50.0, true);
    first.tick(cx);
    pointer_at(&first, 0.0, 50.0, false);
    first.bar.update(cx, |bar, cx| bar.release(cx));
    let moved = first.origin(cx);
    assert_eq!(moved, (x0 - 100.0, y0 - 50.0));

    let restarted = build(cx, &first.rig);
    let again = restarted
        .update(cx, |bar, cx| bar.tick(SCREEN, cx))
        .unwrap();
    assert_eq!((rect(again).0, rect(again).1), moved);

    first
        .rig
        .storage
        .settings
        .set("overlay.position", json!("top"))
        .unwrap();
    assert_eq!(first.setting("overlay.customPos"), Some(json!(null)));
    assert_eq!(first.origin(cx).1, 40.0);
}

#[gpui_kit::test]
fn a_press_that_barely_moves_is_a_click(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.origin(cx);
    pointer_at(&bar, 640.0, 754.0, true);
    bar.at(11_000);
    bar.bar.update(cx, |bar, cx| bar.press_at(cx));
    pointer_at(&bar, 642.0, 755.0, true);
    bar.tick(cx);
    pointer_at(&bar, 642.0, 755.0, false);
    bar.bar.update(cx, |bar, cx| bar.release(cx));

    assert_eq!(machine_state(cx, &bar.rig), State::Listening);
    assert_eq!(bar.setting("overlay.customPos"), Some(json!(null)));
}

#[gpui_kit::test]
fn a_release_the_window_never_heard_ends_the_press_on_the_next_tick(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.origin(cx);
    pointer_at(&bar, 640.0, 754.0, true);
    bar.at(11_000);
    bar.bar.update(cx, |bar, cx| bar.press_at(cx));
    pointer_at(&bar, 900.0, 300.0, true);
    bar.tick(cx);
    pointer_at(&bar, 900.0, 300.0, false);
    bar.tick(cx);

    assert!(!bar.bar.read_with(cx, |bar, _| bar.is_pressed()));
    assert!(bar.setting("overlay.customPos").unwrap().is_object());
    assert_eq!(machine_state(cx, &bar.rig), State::Idle);
}

#[gpui_kit::test]
fn a_dragged_bar_never_leaves_the_screen(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.origin(cx);
    pointer_at(&bar, 640.0, 754.0, true);
    bar.bar.update(cx, |bar, cx| bar.press_at(cx));
    pointer_at(&bar, -5000.0, -5000.0, true);
    bar.tick(cx);
    assert_eq!(bar.origin(cx), (0.0, 0.0));
}

#[gpui_kit::test]
fn the_state_section_reports_the_bar(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.tick(cx);
    let state = bar.bar.read_with(cx, |bar, _| bar.state_json());
    assert_eq!(state["state"], "idle");
    assert_eq!(state["visible"], true);
    assert_eq!(state["bounds"]["width"], 128.0);
    assert_eq!(state["picker"], "closed");
    assert_eq!(state["position"], "bottom");
}

mod in_a_window {
    use super::*;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AnyWindowHandle, SharedString, WindowBounds, WindowOptions, size};

    fn open(cx: &mut TestAppContext, bar: &Bar) -> AnyWindowHandle {
        cx.update(gpui_kit::init);
        let view = bar.bar.clone();
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        gpui_kit::Point::default(),
                        size(px(380.0), px(240.0)),
                    ))),
                    ..Default::default()
                },
                cx,
                move |_, _| view,
            )
            .expect("open test window")
        });
        frame(cx, bar, handle);
        handle
    }

    fn frame(cx: &mut TestAppContext, bar: &Bar, handle: AnyWindowHandle) {
        bar.tick(cx);
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }

    fn click(cx: &mut TestAppContext, bar: &Bar, handle: AnyWindowHandle, id: &str) {
        let id = SharedString::from(id.to_owned());
        cx.update_window(handle, move |_, window, cx| window.click(id, cx))
            .unwrap();
        frame(cx, bar, handle);
    }

    fn present(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> bool {
        let id = SharedString::from(id.to_owned());
        cx.update_window(handle, move |_, window, _| window.try_find(id).is_some())
            .unwrap()
    }

    #[gpui_kit::test]
    fn clicking_the_pill_starts_a_dictation_and_clicking_it_again_stops_it(
        cx: &mut TestAppContext,
    ) {
        let bar = bar(cx, &["base"], "base");
        let handle = open(cx, &bar);
        say(&bar.rig, outcome_text("hello", "en"));

        bar.at(11_000);
        click(cx, &bar, handle, "flowbar.bar");
        assert_eq!(machine_state(cx, &bar.rig), State::Listening);
        assert_eq!(bar.props(cx).state, BarState::Listening);

        bar.at(14_000);
        click(cx, &bar, handle, "flowbar.bar");
        assert_eq!(machine_state(cx, &bar.rig), State::Transcribing);
    }

    #[gpui_kit::test]
    fn the_language_chip_and_an_option_work_without_starting_a_dictation(cx: &mut TestAppContext) {
        let bar = bar(cx, &["base"], "base");
        let handle = open(cx, &bar);
        assert!(present(cx, handle, "flowbar.language"));
        assert!(!present(cx, handle, "flowbar.language.list"));

        click(cx, &bar, handle, "flowbar.language");
        assert!(present(cx, handle, "flowbar.language.list"));
        assert!(present(cx, handle, "flowbar.language.option.en"));
        assert_eq!(machine_state(cx, &bar.rig), State::Idle);

        click(cx, &bar, handle, "flowbar.language.option.en");
        assert_eq!(bar.setting("dictation.language"), Some(json!("en")));
        assert!(!present(cx, handle, "flowbar.language.list"));
        assert_eq!(machine_state(cx, &bar.rig), State::Idle);
    }

    #[gpui_kit::test]
    fn an_english_only_model_shows_a_reason_on_a_disabled_chip(cx: &mut TestAppContext) {
        let bar = bar(cx, &["tiny.en"], "tiny.en");
        let handle = open(cx, &bar);
        assert!(present(cx, handle, "flowbar.language"));
        assert!(!bar.props(cx).language.enabled);

        click(cx, &bar, handle, "flowbar.language");
        assert!(present(cx, handle, "flowbar.language.reason"));
        assert!(!present(cx, handle, "flowbar.language.list"));
        assert_eq!(machine_state(cx, &bar.rig), State::Idle);
    }

    #[gpui_kit::test]
    fn the_failure_notice_has_an_open_history_button(cx: &mut TestAppContext) {
        let bar = bar(cx, &["base"], "base");
        let handle = open(cx, &bar);
        assert!(!present(cx, handle, "flowbar.open-history"));
        dictate(
            cx,
            &bar,
            JobOutcome::Failed(Failure::new("ENGINE_NO_SPEECH", "no audio")),
        );
        frame(cx, &bar, handle);
        assert!(present(cx, handle, "flowbar.open-history"));

        click(cx, &bar, handle, "flowbar.open-history");
        assert_eq!(bar.opened.borrow().len(), 1);
        assert_eq!(
            machine_state(cx, &bar.rig),
            State::Failed,
            "no new dictation"
        );
    }
}
