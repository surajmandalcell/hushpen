//! Motion: the durations, the easing, and the rules every animation in the app keeps. An
//! animation plays once when its state changes, never on a plain re-render. It eases out with no
//! bounce, and it moves only opacity, a few pixels of offset, or a fill, never the layout of
//! text. Under Reduce Motion each one shows its final state at once and asks for no frames.
//!
//! Time comes from the executor's clock, so a test with a fake clock steps an animation frame by
//! frame. A unit test sees only final states unless it plays motion on purpose, and the pixel
//! tests turn on Reduce Motion, so every baseline is a settled frame.

use gpui_kit::base::motion::{MotionStatus, Transition, transition_with_status};
use gpui_kit::{AnyElement, App, ElementId, IntoElement, RenderOnce, Styled, Window, px};
use std::time::{Duration, Instant};

/// Something small that appears: a tooltip, an open menu.
pub const FAST: Duration = Duration::from_millis(90);
/// A control that moves from one state to the other: the knob of a switch, the fill of a
/// segmented control.
pub const SLIDE: Duration = Duration::from_millis(110);
/// A surface that appears or changes: the view in the content pane, a sheet, an onboarding step.
pub const BASE: Duration = Duration::from_millis(130);
/// How far a surface travels into its place while it fades in, in pixels.
pub const TRAVEL: f32 = 4.0;

/// Cubic ease-out: quick at first, then slowing into place. It never passes 1, so nothing
/// bounces.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Whether motion runs now. It never does under Reduce Motion.
pub fn animates(cx: &App) -> bool {
    !cx.reduce_motion() && plays(cx)
}

#[cfg(not(test))]
fn plays(_: &App) -> bool {
    true
}

/// A unit test reads final states, as a person with Reduce Motion sees them, unless it plays
/// motion on purpose.
#[cfg(test)]
fn plays(cx: &App) -> bool {
    cx.has_global::<tests::Play>()
}

/// The progress, from 0 to 1, of an entrance that starts on the first frame `id` renders, such
/// as an open menu. It plays again only after `id` has been gone for a frame.
pub fn enter(
    id: impl Into<ElementId>,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let now = cx.background_executor().now();
    let started = *window.use_keyed_state(id, cx, |_, _| now).read(cx);
    progress(Some(started), now, duration, window, cx)
}

/// The progress, from 0 to 1, since `key` last changed in the place `id`, such as the view in
/// the content pane. The first frame shows the final state: only a change plays.
pub fn change<K: PartialEq + Clone + 'static>(
    id: impl Into<ElementId>,
    key: K,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    struct Changed<K> {
        key: K,
        at: Option<Instant>,
    }
    let now = cx.background_executor().now();
    let state = window.use_keyed_state(id, cx, |_, _| Changed {
        key: key.clone(),
        at: None,
    });
    let at = state.update(cx, |state, _| {
        if state.key != key {
            state.key = key;
            state.at = Some(now);
        }
        state.at
    });
    progress(at, now, duration, window, cx)
}

/// The value in the place `id` on its way to `target`, such as the knob of a switch. The first
/// frame shows `target`; a new target starts from wherever the value is.
pub fn toward(
    id: impl Into<ElementId>,
    target: f32,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let duration = if animates(cx) {
        duration
    } else {
        Duration::ZERO
    };
    let moved = transition_with_status(
        id.into(),
        target,
        Transition::new(duration).ease(ease_out),
        window,
        cx,
    );
    match moved.status {
        MotionStatus::Delayed | MotionStatus::Running => moved.value,
        // Exactly the target, so a settled control is drawn as it is at rest.
        MotionStatus::Idle | MotionStatus::Finished => target,
    }
}

/// An element built at the value in the place `id` on its way to `target`, for a builder that has
/// no window at hand, such as a switch. See [`toward`].
pub fn moving<E: IntoElement + 'static>(
    id: impl Into<ElementId>,
    target: f32,
    duration: Duration,
    build: impl FnOnce(f32) -> E + 'static,
) -> impl IntoElement {
    Moving {
        id: id.into(),
        target,
        duration,
        build: Box::new(move |value| build(value).into_any_element()),
    }
}

#[derive(IntoElement)]
struct Moving {
    id: ElementId,
    target: f32,
    duration: Duration,
    build: Box<dyn FnOnce(f32) -> AnyElement>,
}

impl RenderOnce for Moving {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let value = toward(self.id, self.target, self.duration, window, cx);
        (self.build)(value)
    }
}

/// `element` faded to `progress` and still `travel` pixels short of its place, below it when
/// `travel` is positive and above it when negative. At 1 it comes back untouched, so a settled
/// frame is the frame the app drew before it had motion.
pub fn arrive<E: Styled>(element: E, progress: f32, travel: f32) -> E {
    if progress >= 1.0 {
        return element;
    }
    element.opacity(progress).top(px(travel * (1.0 - progress)))
}

fn progress(
    started: Option<Instant>,
    now: Instant,
    duration: Duration,
    window: &mut Window,
    cx: &App,
) -> f32 {
    let Some(started) = started else {
        return 1.0;
    };
    if !animates(cx) || duration.is_zero() {
        return 1.0;
    }
    let t = now.saturating_duration_since(started).as_secs_f32() / duration.as_secs_f32();
    if t >= 1.0 {
        return 1.0;
    }
    window.request_animation_frame();
    ease_out(t)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use gpui_kit::{Context, Global, IntoElement, Render, TestAppContext, WindowHandle, div, size};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    /// Turns motion on for one test, which then steps it with the fake clock.
    pub(crate) struct Play;
    impl Global for Play {}

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Sample {
        entered: f32,
        changed: f32,
        moved: f32,
    }

    struct Probe {
        key: Rc<Cell<u8>>,
        target: Rc<Cell<f32>>,
        samples: Rc<RefCell<Vec<Sample>>>,
    }

    impl Render for Probe {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let sample = Sample {
                entered: enter("probe.enter", BASE, window, cx),
                changed: change("probe.change", self.key.get(), BASE, window, cx),
                moved: toward("probe.toward", self.target.get(), SLIDE, window, cx),
            };
            self.samples.borrow_mut().push(sample);
            div()
        }
    }

    struct Fixture {
        window: WindowHandle<Probe>,
        key: Rc<Cell<u8>>,
        target: Rc<Cell<f32>>,
        samples: Rc<RefCell<Vec<Sample>>>,
    }

    impl Fixture {
        fn open(cx: &mut TestAppContext) -> Self {
            let key = Rc::new(Cell::new(0));
            let target = Rc::new(Cell::new(0.0));
            let samples = Rc::new(RefCell::new(Vec::new()));
            let window = cx.open_window(size(px(100.0), px(100.0)), {
                let (key, target, samples) = (key.clone(), target.clone(), samples.clone());
                move |_, _| Probe {
                    key,
                    target,
                    samples,
                }
            });
            cx.run_until_parked();
            Self {
                window,
                key,
                target,
                samples,
            }
        }

        /// Draws a frame `millis` later than the last one.
        fn after(&self, cx: &mut TestAppContext, millis: u64) -> Sample {
            cx.executor().advance_clock(Duration::from_millis(millis));
            self.window
                .update(cx, |_, window, _| window.refresh())
                .unwrap();
            cx.run_until_parked();
            self.last()
        }

        fn last(&self) -> Sample {
            *self.samples.borrow().last().unwrap()
        }

        /// Runs the frames the last one asked for, and says how many there were.
        fn pending(&self, cx: &mut TestAppContext) -> usize {
            let count = self
                .window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap();
            cx.run_until_parked();
            count
        }
    }

    fn near(actual: f32, expected: f32) -> bool {
        (actual - expected).abs() < 1e-4
    }

    #[gpui_kit::test]
    fn an_animation_eases_into_place_once_and_then_asks_for_no_frames(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_global(Play));
        let fixture = Fixture::open(cx);
        let first = fixture.last();
        assert_eq!(first.entered, 0.0, "an entrance starts hidden");
        assert_eq!(first.changed, 1.0, "the first frame of a place is settled");
        assert_eq!(first.moved, 0.0, "a value starts at its target");

        fixture.key.set(1);
        fixture.target.set(1.0);
        let changed = fixture.after(cx, 0);
        assert_eq!((changed.changed, changed.moved), (0.0, 0.0));

        let half_ms = BASE.as_millis() as u64 / 2;
        let half = fixture.after(cx, half_ms);
        let slid = half_ms as f32 / SLIDE.as_millis() as f32;
        assert!(near(half.entered, ease_out(0.5)), "{half:?}");
        assert!(near(half.changed, ease_out(0.5)), "{half:?}");
        assert!(near(half.moved, ease_out(slid)), "{half:?}");

        let done = fixture.after(cx, half_ms);
        assert_eq!(
            done,
            Sample {
                entered: 1.0,
                changed: 1.0,
                moved: 1.0
            }
        );
        fixture.pending(cx);
        assert_eq!(
            fixture.pending(cx),
            0,
            "a settled animation asks for no frames"
        );
        assert_eq!(
            fixture.after(cx, 0),
            done,
            "a re-render does not play it again"
        );
    }

    #[gpui_kit::test]
    fn reduce_motion_shows_the_final_state_at_once(cx: &mut TestAppContext) {
        cx.update(|cx| {
            cx.set_global(Play);
            cx.set_reduce_motion(true);
        });
        let fixture = Fixture::open(cx);
        assert_eq!(
            fixture.last().entered,
            1.0,
            "an entrance is already in place"
        );
        fixture.key.set(1);
        fixture.target.set(1.0);
        let changed = fixture.after(cx, 0);
        assert_eq!(
            changed,
            Sample {
                entered: 1.0,
                changed: 1.0,
                moved: 1.0
            }
        );
        assert_eq!(fixture.pending(cx), 0, "no frame is asked for");
    }

    #[gpui_kit::test]
    fn a_unit_test_reads_final_states_unless_it_plays_motion(cx: &mut TestAppContext) {
        let fixture = Fixture::open(cx);
        fixture.key.set(1);
        fixture.target.set(1.0);
        assert_eq!(
            fixture.after(cx, 0),
            Sample {
                entered: 1.0,
                changed: 1.0,
                moved: 1.0
            }
        );
    }

    #[test]
    fn the_easing_starts_and_ends_in_place_and_never_overshoots() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        let mut last = 0.0;
        for step in 0..=100 {
            let at = ease_out(step as f32 / 100.0);
            assert!((last..=1.0).contains(&at), "{at} at step {step}");
            last = at;
        }
        assert!(
            ease_out(0.5) > 0.5,
            "it eases out: more than half way at half time"
        );
    }
}
