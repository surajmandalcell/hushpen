use super::*;
use crate::ai_cleanup::{Attempt, Phase};
use crate::controller::ai_tests::{RULE, ai_answers, only_row, pasted, setup};
use crate::controller::testkit::*;
use gpui_kit::{AppContext as _, TestAppContext};
use hushpen_core::dictation::State;
use hushpen_core::flow_bar::ERROR_HOLD_MS;
use hushpen_core::insert::{Chord, Outcome as InsertOutcome, Report, Restore};
use hushpen_engine::{Failure, JobOutcome};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

const SCREEN: Screen = Screen {
    origin: (0.0, 0.0),
    size: (1280.0, 800.0),
};
/// A second screen to the right of the first, a little higher.
const RIGHT: Screen = Screen {
    origin: (1280.0, -100.0),
    size: (1920.0, 1080.0),
};
const CORRECTED: &str = "I think we should meet on Wednesday at 3 PM.";

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
    bar_on(cx, rig)
}

/// The idle pill is on, so the geometry tests have a bar to measure, and every change shows at
/// once.
fn bar_on(cx: &mut TestAppContext, rig: Rig) -> Bar {
    rig.storage
        .settings
        .set("overlay.idleVisible", json!(true))
        .unwrap();
    let bar = build(cx, &rig);
    let now = Rc::new(Cell::new(10_000));
    let pointer = Rc::new(RefCell::new(None));
    let opened: Rc<RefCell<Vec<String>>> = Rc::default();
    bar.update(cx, |bar, _| {
        bar.animate = false;
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
            rig.mic.clone(),
        )
    })
}

impl Bar {
    fn tick(&self, cx: &mut TestAppContext) -> Option<Bounds<Pixels>> {
        self.tick_on(SCREEN, cx)
    }

    /// A tick while the pointer is on `screen`.
    fn tick_on(&self, screen: Screen, cx: &mut TestAppContext) -> Option<Bounds<Pixels>> {
        self.bar.update(cx, |bar, cx| bar.tick(screen, cx))
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
fn the_bar_appears_on_the_screen_with_the_pointer(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    let (x, y, width, height) = rect(bar.tick_on(RIGHT, cx).expect("the idle bar shows"));
    assert_eq!((x + width / 2.0, y + height), (2240.0, 948.0));
}

#[gpui_kit::test]
fn a_dictation_moves_the_bar_to_the_pointer_and_keeps_it_there(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    let (x, ..) = rect(bar.tick(cx).unwrap());
    assert!(x < 1280.0, "the idle pill starts on the first screen");
    say(&bar.rig, JobOutcome::Cancelled);
    bar.at(11_000);
    bar.click(cx);
    assert_eq!(machine_state(cx, &bar.rig), State::Listening);
    let (x, _, width, _) = rect(bar.tick_on(RIGHT, cx).unwrap());
    assert_eq!(x + width / 2.0, 2240.0);
    let (x, _, width, _) = rect(bar.tick_on(SCREEN, cx).unwrap());
    assert_eq!(
        x + width / 2.0,
        2240.0,
        "the bar does not jump during a dictation"
    );
}

#[gpui_kit::test]
fn a_bar_dragged_on_one_screen_keeps_its_spot_on_another(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    let (x0, y0) = {
        bar.tick_on(RIGHT, cx);
        bar.bar.read_with(cx, |bar, _| bar.frame().0)
    };
    pointer_at(&bar, 2000.0, 900.0, true);
    bar.bar.update(cx, |bar, cx| bar.press_at(cx));
    pointer_at(&bar, 1900.0, 900.0, true);
    bar.tick_on(RIGHT, cx);
    pointer_at(&bar, 1900.0, 900.0, false);
    bar.bar.update(cx, |bar, cx| bar.release(cx));
    let spot = bar.setting("overlay.customPos").unwrap();
    assert_eq!(
        spot["x"],
        json!(x0 - 100.0 - 1280.0 + view::IDLE_SIZE.0 / 2.0)
    );
    assert_eq!(spot["y"], json!(y0 + 100.0 + view::IDLE_SIZE.1));

    let again = build(cx, &bar.rig);
    again.update(cx, |bar, _| bar.animate = false);
    let on_first = again.update(cx, |bar, cx| bar.tick(SCREEN, cx)).unwrap();
    // The first screen is shorter, so the bar sits on its bottom edge.
    assert_eq!(
        (rect(on_first).0, rect(on_first).1),
        (x0 - 100.0 - 1280.0, SCREEN.size.1 - view::IDLE_SIZE.1)
    );
}

#[gpui_kit::test]
fn a_bar_dragged_on_the_second_screen_stays_on_it(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.tick_on(RIGHT, cx);
    pointer_at(&bar, 2240.0, 930.0, true);
    bar.bar.update(cx, |bar, cx| bar.press_at(cx));
    pointer_at(&bar, -5000.0, -5000.0, true);
    bar.tick_on(RIGHT, cx);
    assert_eq!(
        bar.bar.read_with(cx, |bar, _| bar.frame().0),
        (1280.0, -100.0)
    );
}

/// A host whose native window work updates the App, as GPUI does when AppKit reports a move.
struct Reentrant {
    app: gpui_kit::AsyncApp,
    native_calls: Rc<Cell<u32>>,
}

impl Host for Reentrant {
    fn screen(&self, _: &App) -> Screen {
        SCREEN
    }

    fn show(&mut self, _: Bounds<Pixels>, _: &mut App) -> Option<Native> {
        let app = self.app.clone();
        let calls = Rc::clone(&self.native_calls);
        Some(Box::new(move || {
            // Panics with `already borrowed` when the loop still holds the App.
            app.update(|_| ());
            calls.set(calls.get() + 1);
        }))
    }

    fn hide(&mut self, _: &mut App) {}

    fn window(&self) -> Option<AnyWindowHandle> {
        None
    }
}

#[gpui_kit::test]
fn the_native_window_work_runs_after_the_app_is_free(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    let native_calls = Rc::new(Cell::new(0));
    let host = Reentrant {
        app: cx.to_async(),
        native_calls: Rc::clone(&native_calls),
    };
    let entity = bar.bar.clone();
    cx.update(|cx| run(cx, entity, Box::new(host)));
    cx.run_until_parked();
    assert_eq!(native_calls.get(), 1);
    cx.executor().advance_clock(TICK);
    cx.run_until_parked();
    assert_eq!(native_calls.get(), 2);
}

/// Counts the times the bar asked for its window.
struct Counting {
    shows: Rc<Cell<u32>>,
}

impl Host for Counting {
    fn screen(&self, _: &App) -> Screen {
        SCREEN
    }

    fn show(&mut self, _: Bounds<Pixels>, _: &mut App) -> Option<Native> {
        self.shows.set(self.shows.get() + 1);
        None
    }

    fn hide(&mut self, _: &mut App) {}

    fn window(&self) -> Option<AnyWindowHandle> {
        None
    }
}

#[gpui_kit::test]
fn a_start_shows_the_bar_without_waiting_for_the_next_tick(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let bar = build(cx, &rig);
    bar.update(cx, |bar, _| bar.animate = false);
    let shows = Rc::new(Cell::new(0));
    let host = Counting {
        shows: Rc::clone(&shows),
    };
    let entity = bar.clone();
    cx.update(|cx| run(cx, entity, Box::new(host)));
    cx.run_until_parked();
    assert_eq!(shows.get(), 0, "no bar at rest");

    send_at(cx, &rig, 11_000, AppEvent::HomeToggle).unwrap();
    cx.run_until_parked();
    assert_eq!(shows.get(), 1, "the bar shows before the clock moves");
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
fn a_paste_says_pasted(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    attach_inserter(
        cx,
        &bar.rig,
        &FakeInserter::pasted_into("GtkTarget", Chord::CtrlV),
    );
    dictate(cx, &bar, outcome_text("hello there", "en"));
    assert_eq!(bar.state(cx), BarState::Result);
    assert_eq!(bar.props(cx).message, "Pasted");
}

#[gpui_kit::test]
fn the_waveform_follows_the_meter_only_while_listening(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    assert!(bar.state(cx) == BarState::Idle);
    assert!(bar.props(cx).levels.is_empty());
    bar.at(11_000);
    bar.click(cx);
    bar.tick(cx);
    assert_eq!(bar.props(cx).levels.len(), view::WAVE_BARS);
}

#[gpui_kit::test]
fn a_fresh_install_shows_no_bar_until_a_dictation_starts(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let bar = build(cx, &rig);
    bar.update(cx, |bar, _| bar.animate = false);
    assert_eq!(
        rig.storage.settings.get("overlay.idleVisible"),
        Some(json!(false))
    );
    let tick = |cx: &mut TestAppContext| bar.update(cx, |bar, cx| bar.tick(SCREEN, cx));
    assert!(tick(cx).is_none());
    send_at(cx, &rig, 11_000, AppEvent::FlowBarClick).unwrap();
    assert!(tick(cx).is_some(), "the listening bar shows");
    send_at(cx, &rig, 11_500, AppEvent::Esc).unwrap();
    assert!(tick(cx).is_none(), "gone again after a cancel");
}

#[gpui_kit::test]
fn a_bar_that_goes_away_fades_out_before_its_window_closes(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    bar.bar.update(cx, |bar, _| bar.animate = true);
    bar.at(11_000);
    bar.click(cx);
    for step in 1..=40 {
        bar.at(11_000 + step * 16);
        bar.tick(cx);
    }
    let (_, (width, _)) = bar.bar.read_with(cx, |bar, _| bar.frame());
    assert!(
        (width - view::LISTENING_SIZE.0).abs() < 1.0,
        "settled at {width}"
    );

    bar.bar.update(cx, |bar, cx| bar.cancel(cx));
    bar.rig
        .storage
        .settings
        .set("overlay.idleVisible", json!(false))
        .unwrap();
    bar.at(11_700);
    assert!(bar.tick(cx).is_some(), "still fading on the first frame");
    assert_eq!(
        bar.props(cx).state,
        BarState::Listening,
        "it keeps its last look while it goes"
    );
    let mut now = 11_700;
    while bar.tick(cx).is_some() {
        now += 16;
        bar.at(now);
        assert!(now < 12_300, "the window is still open after 600 ms");
    }
}

#[gpui_kit::test]
fn the_cross_cancels_and_the_tick_finishes(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    say(&bar.rig, outcome_text("hello there", "en"));
    bar.at(11_000);
    bar.click(cx);
    assert_eq!(machine_state(cx, &bar.rig), State::Listening);
    bar.bar.update(cx, |bar, cx| bar.cancel(cx));
    assert_ne!(machine_state(cx, &bar.rig), State::Listening);
    assert!(
        bar.rig.specs.lock().unwrap().is_empty(),
        "nothing transcribed"
    );

    send_at(cx, &bar.rig, 13_000, AppEvent::Tick).unwrap();
    bar.at(13_000);
    bar.click(cx);
    assert_eq!(machine_state(cx, &bar.rig), State::Listening);
    bar.at(15_000);
    bar.bar.update(cx, |bar, cx| bar.finish(cx));
    assert_eq!(machine_state(cx, &bar.rig), State::Transcribing);
    // A tick with nothing listening must not start a new dictation.
    bar.bar.update(cx, |bar, cx| bar.finish(cx));
    assert_eq!(machine_state(cx, &bar.rig), State::Transcribing);
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
    assert_eq!(spot["x"], json!(x0 - 300.0 + view::IDLE_SIZE.0 / 2.0));
    assert_eq!(spot["y"], json!(y0 - 200.0 + view::IDLE_SIZE.1));
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
    restarted.update(cx, |bar, _| bar.animate = false);
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
    assert_eq!(state["bounds"]["width"], view::IDLE_SIZE.0);
    assert_eq!(state["settled"], true);
    assert_eq!(state["position"], "bottom");
}

/// A push-to-talk run that the AI step answers, pasted through `inserter`.
fn cleaned_bar(cx: &mut TestAppContext, inserter: Option<&Arc<FakeInserter>>) -> Bar {
    let (rig, _) = setup(cx);
    if let Some(inserter) = inserter {
        attach_inserter(cx, &rig, inserter);
    }
    ai_answers(cx, &rig, Attempt::Done(CORRECTED.into()), Ok(()));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);
    let bar = bar_on(cx, rig);
    bar.at(3_100);
    bar
}

fn cleaned_target() -> Arc<FakeInserter> {
    FakeInserter::pasted_into("Target-gtk.py", Chord::CtrlV)
}

#[gpui_kit::test]
fn an_ai_cleaned_paste_offers_undo_cleanup_in_place_of_the_flash(cx: &mut TestAppContext) {
    let bar = cleaned_bar(cx, None);

    assert_eq!(bar.state(cx), BarState::Undo);
    let props = bar.props(cx);
    assert_eq!(props.message, "Pasted");
    assert_eq!(view::size_of(&props), view::UNDO_SIZE);
    let state = bar.bar.read_with(cx, |bar, _| bar.state_json());
    assert_eq!(state["state"], "undo");
    assert_eq!(state["visible"], true);

    bar.at(3_000 + 5_200);
    assert_ne!(bar.state(cx), BarState::Undo, "gone after five seconds");
}

#[gpui_kit::test]
fn an_expired_undo_offer_leaves_the_hidden_bar_idle(cx: &mut TestAppContext) {
    let bar = cleaned_bar(cx, None);
    bar.rig
        .storage
        .settings
        .set("overlay.idleVisible", json!(false))
        .unwrap();
    assert_eq!(bar.state(cx), BarState::Undo);
    bar.at(3_000 + 2_500);
    send_at(cx, &bar.rig, 3_000 + 2_500, AppEvent::Tick).unwrap();
    assert_eq!(machine_state(cx, &bar.rig), State::Idle);
    assert_eq!(
        bar.state(cx),
        BarState::Undo,
        "the offer outlives the done state"
    );

    bar.at(3_000 + 5_200);
    assert!(
        bar.tick(cx).is_none(),
        "the bar is hidden after the offer ran out"
    );
    let state = bar.bar.read_with(cx, |bar, _| bar.state_json());
    assert_eq!(state["state"], "idle");
    assert_eq!(state["visible"], false);
}

#[gpui_kit::test]
fn a_terminal_never_gets_the_undo_state(cx: &mut TestAppContext) {
    let terminal = FakeInserter::pasted_into("XTerm", Chord::ShiftInsert);
    let bar = cleaned_bar(cx, Some(&terminal));

    assert_eq!(bar.state(cx), BarState::Result);
    assert_eq!(bar.props(cx).message, "Pasted");
    assert!(terminal.undos.lock().unwrap().is_empty());
}

#[gpui_kit::test]
fn a_change_of_focus_takes_undo_off_the_bar(cx: &mut TestAppContext) {
    let target = cleaned_target();
    let bar = cleaned_bar(cx, Some(&target));
    assert_eq!(bar.state(cx), BarState::Undo);

    target.target.lock().unwrap().window = Some(99);

    assert_ne!(bar.state(cx), BarState::Undo);
}

#[gpui_kit::test]
fn the_transcribing_pill_says_when_the_ai_model_is_loading(cx: &mut TestAppContext) {
    let bar = bar(cx, &["base"], "base");
    say(&bar.rig, outcome_text("hello there", "en"));
    bar.at(11_000);
    bar.click(cx);
    bar.at(14_000);
    bar.click(cx);
    assert_eq!(bar.state(cx), BarState::Transcribing);
    assert!(!bar.props(cx).preparing);
    assert_eq!(bar.props(cx).working_text(), "Transcribing");

    bar.rig.controller.read_with(cx, |controller, _| {
        controller.show_ai_phase(Some(Phase::Preparing))
    });
    bar.tick(cx);
    let props = bar.props(cx);
    assert!(props.preparing);
    assert_eq!(props.working_text(), "Preparing the AI model");
    assert_eq!(view::size_of(&props), view::PREPARING_SIZE);

    bar.rig.controller.read_with(cx, |controller, _| {
        controller.show_ai_phase(Some(Phase::Cleaning))
    });
    bar.tick(cx);
    assert_eq!(bar.props(cx).working_text(), "Transcribing");
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
    fn the_listening_pill_has_a_cross_and_a_tick_that_work_on_a_click(cx: &mut TestAppContext) {
        let bar = bar(cx, &["base"], "base");
        let handle = open(cx, &bar);
        say(&bar.rig, outcome_text("hello", "en"));
        assert!(!present(cx, handle, "flowbar.cancel"));

        bar.at(11_000);
        click(cx, &bar, handle, "flowbar.bar");
        assert!(present(cx, handle, "flowbar.cancel"));
        assert!(present(cx, handle, "flowbar.finish"));
        click(cx, &bar, handle, "flowbar.cancel");
        assert_ne!(machine_state(cx, &bar.rig), State::Listening);
        assert!(
            bar.rig.specs.lock().unwrap().is_empty(),
            "nothing transcribed"
        );

        send_at(cx, &bar.rig, 13_000, AppEvent::Tick).unwrap();
        bar.at(13_000);
        click(cx, &bar, handle, "flowbar.bar");
        bar.at(15_000);
        click(cx, &bar, handle, "flowbar.finish");
        assert_eq!(machine_state(cx, &bar.rig), State::Transcribing);
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

    #[gpui_kit::test]
    fn the_undo_button_takes_the_cleanup_back_without_starting_a_dictation(
        cx: &mut TestAppContext,
    ) {
        let target = cleaned_target();
        let bar = cleaned_bar(cx, Some(&target));
        let handle = open(cx, &bar);
        assert!(present(cx, handle, "flowbar.undo"));

        click(cx, &bar, handle, "flowbar.undo");
        settle(cx, &bar.rig);

        assert_eq!(target.undos.lock().unwrap().len(), 1);
        assert_eq!(
            pasted(&target),
            [CORRECTED.to_owned(), RULE.to_owned()],
            "the rule text replaced the AI text"
        );
        assert_eq!(only_row(&bar.rig).llm_outcome.as_deref(), Some("undone"));
        assert_ne!(machine_state(cx, &bar.rig), State::Listening);
        frame(cx, &bar, handle);
        assert!(!present(cx, handle, "flowbar.undo"));
    }

    #[gpui_kit::test]
    fn a_terminal_has_no_undo_button(cx: &mut TestAppContext) {
        let terminal = FakeInserter::pasted_into("XTerm", Chord::ShiftInsert);
        let bar = cleaned_bar(cx, Some(&terminal));
        let handle = open(cx, &bar);
        assert!(!present(cx, handle, "flowbar.undo"));
        assert!(terminal.undos.lock().unwrap().is_empty());
    }
}

fn command_bar(cx: &mut TestAppContext, selection: Option<&str>) -> (Bar, Arc<FakeInserter>) {
    let bar = bar(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    *inserter.selected.lock().unwrap() = selection.map(str::to_owned);
    attach_inserter(cx, &bar.rig, &inserter);
    say(&bar.rig, outcome_text("make this formal", "en"));
    (bar, inserter)
}

/// The command keys as the shortcut engine sends them: the hold key, then the rest.
fn command_chord(cx: &mut TestAppContext, bar: &Bar, at: u64) {
    bar.at(at);
    let _ = send_at(cx, &bar.rig, at, AppEvent::HoldDown);
    let _ = send_at(cx, &bar.rig, at + 10, AppEvent::CommandDown);
}

#[gpui_kit::test]
fn a_command_with_nothing_selected_says_so_and_has_no_history_button(cx: &mut TestAppContext) {
    let (bar, inserter) = command_bar(cx, None);
    ai_answers(cx, &bar.rig, Attempt::Done(CORRECTED.into()), Ok(()));
    command_chord(cx, &bar, 11_000);
    let _ = send_at(cx, &bar.rig, 11_500, AppEvent::CommandUp);
    let _ = send_at(cx, &bar.rig, 11_500, AppEvent::HoldUp);
    settle(cx, &bar.rig);

    assert_eq!(bar.state(cx), BarState::Blocked);
    let props = bar.props(cx);
    assert_eq!(props.message, "Select text first");
    assert!(!props.open_history);
    assert!(pasted(&inserter).is_empty());

    bar.at(11_000 + ERROR_HOLD_MS + 100);
    assert_eq!(bar.state(cx), BarState::Idle);
}

#[gpui_kit::test]
fn a_command_with_ai_off_says_what_it_needs(cx: &mut TestAppContext) {
    let (bar, _inserter) = command_bar(cx, Some("some selected words"));
    command_chord(cx, &bar, 11_000);

    assert_eq!(bar.state(cx), BarState::Blocked);
    assert_eq!(
        bar.props(cx).message,
        "Command mode needs AI cleanup or the endpoint"
    );
}

#[gpui_kit::test]
fn a_command_that_the_model_cannot_finish_shows_an_error_and_pastes_nothing(
    cx: &mut TestAppContext,
) {
    let (bar, inserter) = command_bar(cx, Some("some selected words"));
    ai_answers(cx, &bar.rig, Attempt::TimedOut, Ok(()));
    command_chord(cx, &bar, 11_000);
    let _ = send_at(cx, &bar.rig, 13_000, AppEvent::CommandUp);
    let _ = send_at(cx, &bar.rig, 13_000, AppEvent::HoldUp);
    settle(cx, &bar.rig);

    bar.at(13_100);
    assert_eq!(bar.state(cx), BarState::Error);
    assert_eq!(bar.props(cx).message, "AI edit timed out. Text unchanged");
    assert!(pasted(&inserter).is_empty());
}
