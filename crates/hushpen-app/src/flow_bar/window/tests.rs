//! The pop-up keeps its window between dictations where it can, and closes it where it cannot.

use super::*;
use crate::controller::testkit::rig;
use gpui_kit::{AppContext as _, TestAppContext, point, px, size};
use std::cell::RefCell;
use std::time::Instant;

thread_local! {
    static CALLS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn hid(_: WindowHandle) -> Result<(), ChromeError> {
    CALLS.with(|calls| calls.borrow_mut().push("hide"));
    Ok(())
}

fn shown(_: WindowHandle) -> Result<(), ChromeError> {
    CALLS.with(|calls| calls.borrow_mut().push("show"));
    Ok(())
}

fn calls() -> Vec<&'static str> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

/// The pop-up holds its view weakly, so the caller keeps the view alive.
fn pop_up(cx: &mut TestAppContext, keep: bool, hide_call: Call) -> (PopUp, Entity<FlowBar>) {
    cx.update(gpui_kit::init);
    let rig = rig(cx, &["base"], "base");
    let view = cx.new(|_| {
        FlowBar::new(
            Rc::clone(&rig.storage),
            rig.controller.clone(),
            rig.mic.clone(),
        )
    });
    let mut pop_up = PopUp::new(&view);
    pop_up.keep = keep;
    pop_up.hide_call = hide_call;
    pop_up.show_call = shown;
    (pop_up, view)
}

fn bounds() -> Bounds<Pixels> {
    Bounds::new(point(px(100.0), px(700.0)), size(px(112.0), px(32.0)))
}

/// One show with its native work done, as the run loop does it.
fn show(pop_up: &mut PopUp, cx: &mut TestAppContext) {
    if let Some(native) = cx.update(|cx| pop_up.show(bounds(), cx)) {
        native();
    }
}

fn hide_now(pop_up: &mut PopUp, cx: &mut TestAppContext) {
    if let Some(native) = cx.update(|cx| pop_up.hide(cx)) {
        native();
    }
}

fn windows(cx: &mut TestAppContext) -> usize {
    cx.update(|cx| cx.windows().len())
}

#[gpui_kit::test]
fn a_kept_window_is_hidden_and_shown_again_not_rebuilt(cx: &mut TestAppContext) {
    let (mut pop_up, _view) = pop_up(cx, true, hid);
    show(&mut pop_up, cx);
    assert_eq!(windows(cx), 1);
    assert!(pop_up.window().is_some());
    assert_eq!(calls(), Vec::<&str>::new(), "the first show only opens it");

    hide_now(&mut pop_up, cx);
    assert_eq!(windows(cx), 1, "the window stays open");
    assert!(
        pop_up.window().is_none(),
        "a hidden window is not on screen"
    );
    assert_eq!(calls(), ["hide"]);

    // The loop hides every tick while the bar rests.
    hide_now(&mut pop_up, cx);
    hide_now(&mut pop_up, cx);
    assert_eq!(
        calls(),
        Vec::<&str>::new(),
        "one native hide, not one per tick"
    );

    show(&mut pop_up, cx);
    assert_eq!(windows(cx), 1, "no second window");
    assert!(pop_up.window().is_some());
    assert_eq!(calls(), ["show"]);

    // A bar that never hid is only moved.
    show(&mut pop_up, cx);
    assert_eq!(calls(), Vec::<&str>::new());
}

#[gpui_kit::test]
fn a_window_that_is_not_kept_closes_on_hide_and_opens_again(cx: &mut TestAppContext) {
    let (mut pop_up, _view) = pop_up(cx, false, hid);
    show(&mut pop_up, cx);
    hide_now(&mut pop_up, cx);
    assert_eq!(windows(cx), 0);
    assert!(pop_up.window().is_none());
    assert_eq!(calls(), Vec::<&str>::new(), "nothing was hidden natively");
    show(&mut pop_up, cx);
    assert_eq!(windows(cx), 1);
}

#[gpui_kit::test]
fn a_hide_the_platform_refuses_closes_the_window_on_the_next_hide(cx: &mut TestAppContext) {
    // The platform call for a window it cannot hide fails with `WindowHandle::Other`.
    let (mut pop_up, _view) = pop_up(cx, true, hide);
    show(&mut pop_up, cx);
    hide_now(&mut pop_up, cx);
    assert_eq!(windows(cx), 1);
    hide_now(&mut pop_up, cx);
    assert_eq!(windows(cx), 0, "no window is left on screen");
    show(&mut pop_up, cx);
    assert_eq!(windows(cx), 1);
}

#[gpui_kit::test]
fn a_window_the_system_dropped_while_hidden_opens_again(cx: &mut TestAppContext) {
    let (mut pop_up, _view) = pop_up(cx, true, hid);
    show(&mut pop_up, cx);
    hide_now(&mut pop_up, cx);
    let handle = pop_up.handle.unwrap();
    cx.update(|cx| {
        handle
            .update(cx, |_, window, _| window.remove_window())
            .unwrap()
    });
    show(&mut pop_up, cx);
    assert_eq!(windows(cx), 1);
    assert!(pop_up.window().is_some());
}

/// Times the show of a dictation after a hide against the show that builds the window.
/// The test platform has no native window, so this bounds the app's own work only.
#[gpui_kit::test]
fn showing_a_kept_window_costs_less_than_building_one(cx: &mut TestAppContext) {
    const ROUNDS: u32 = 50;
    let time = |keep: bool, cx: &mut TestAppContext| {
        let (mut pop_up, _view) = pop_up(cx, keep, hid);
        let mut total = std::time::Duration::ZERO;
        for _ in 0..ROUNDS {
            let start = Instant::now();
            show(&mut pop_up, cx);
            total += start.elapsed();
            hide_now(&mut pop_up, cx);
        }
        total / ROUNDS
    };
    let built = time(false, cx);
    let kept = time(true, cx);
    eprintln!("SHOW_TIMING build-per-dictation={built:?} keep-alive={kept:?}");
    assert!(kept <= built * 2, "kept {kept:?}, built {built:?}");
}
