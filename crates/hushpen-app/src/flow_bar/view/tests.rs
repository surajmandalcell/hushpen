use super::*;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as _, Bounds, Pixels, TestAppContext, WindowBounds, WindowOptions};

#[test]
fn each_state_has_its_own_size_and_every_pill_but_idle_is_one_height() {
    let mut props = Props::new(BarState::Idle);
    assert_eq!(size_of(&props), IDLE_SIZE);
    for (state, size) in [
        (BarState::Listening, LISTENING_SIZE),
        (BarState::Transcribing, TRANSCRIBING_SIZE),
        (BarState::Result, RESULT_SIZE),
        (BarState::Undo, UNDO_SIZE),
        (BarState::Cancelled, CANCELLED_SIZE),
    ] {
        props.state = state;
        assert_eq!(size_of(&props), size);
        assert_eq!(size.1, HEIGHT, "{state:?}");
    }
    assert_eq!(HEIGHT, 28.0);
    assert_eq!(IDLE_SIZE, (67.0, 22.0));
    assert_eq!(
        (
            LISTENING_SIZE.0,
            RESULT_SIZE.0,
            UNDO_SIZE.0,
            PREPARING_SIZE.0
        ),
        (118.0, 118.0, 126.0, 222.0)
    );
}

#[test]
fn a_notice_is_as_wide_as_its_line_and_never_wider_than_the_widest() {
    let mut props = Props::new(BarState::Blocked);
    props.message = "Select text first".into();
    let short = size_of(&props);
    assert!(short.0 < 200.0, "{short:?}");
    assert_eq!(short.1, HEIGHT);
    props.open_history = true;
    assert!(
        size_of(&props).0 > short.0,
        "room for the mark and the button"
    );
    props.message = "x".repeat(200);
    assert_eq!(size_of(&props), NOTICE_SIZE);
    assert_eq!(NOTICE_SIZE.0, 366.0);
}

#[test]
fn the_listening_pill_fits_the_cross_the_wave_and_the_tick() {
    let wave = WAVE_BARS as f32 * BAR_WIDTH + (WAVE_BARS - 1) as f32 * BAR_WIDTH;
    let inner = 2.0 * size::BUTTON_INSET + 2.0 * ROUND_BUTTON + 2.0 * space::SM + wave;
    assert!(inner <= LISTENING_SIZE.0, "{inner}");
    assert_eq!(
        HEIGHT - ROUND_BUTTON,
        2.0 * size::BUTTON_INSET,
        "4 pt on all sides"
    );
}

#[test]
fn the_three_bars_at_each_end_stand_lower_and_the_middle_ones_do_not() {
    let factors: Vec<f32> = (0..WAVE_BARS).map(|bar| pinch(bar, WAVE_BARS)).collect();
    assert_eq!(
        factors,
        [0.70, 0.85, 0.95, 1.0, 1.0, 1.0, 1.0, 1.0, 0.95, 0.85, 0.70]
    );
    assert_eq!(pinch(0, working::BEADS), 0.70);
    assert_eq!(pinch(working::BEADS - 1, working::BEADS), 0.70);
    assert_eq!(pinch(working::BEADS / 2, working::BEADS), 1.0);
}

#[test]
fn the_newest_level_is_in_the_middle_and_older_ones_spread_out() {
    let mut meter = vec![0.0; 32];
    meter[31] = 1.0;
    meter[30] = 0.5;
    let goal = wave_goal(&meter);
    assert_eq!(goal.len(), WAVE_BARS);
    let center = WAVE_BARS / 2;
    assert_eq!(goal[center], 1.0);
    assert_eq!(goal[center - 1], goal[center + 1]);
    assert!(goal[center - 1] > 0.4 && goal[center - 1] < 0.5);
    assert_eq!(goal[0], 0.0);
    assert_eq!(wave_goal(&[]), vec![0.0; WAVE_BARS]);
}

#[test]
fn the_processing_wave_fits_the_listening_pill_and_only_the_model_wait_has_text() {
    assert!(working::width() + 2.0 * size::INSET <= TRANSCRIBING_SIZE.0);
    let mut props = Props::new(BarState::Transcribing);
    assert_eq!(size_of(&props), LISTENING_SIZE);
    assert_eq!(props.working_text(), "Transcribing");
    props.stage = Stage::Preparing;
    assert_eq!(size_of(&props), PREPARING_SIZE);
    assert_eq!(props.working_text(), "Preparing the AI model");
}

#[test]
fn failures_have_short_lines() {
    assert_eq!(failure_text(Some("ENGINE_NO_SPEECH")), "No speech heard");
    assert_eq!(
        failure_text(Some("INSERT_KEYBOARD_GRABBED")),
        "Another app holds the keyboard"
    );
    assert_eq!(failure_text(Some("ENGINE_CRASHED")), "Transcription failed");
    assert_eq!(failure_text(None), "Transcription failed");
    assert_eq!(result_text(true), "Copied");
    assert_eq!(result_text(false), "Pasted");
}

/// Every state the bar can draw, by name.
fn drawn_states() -> Vec<(&'static str, Props)> {
    let with = |state: BarState, message: &str, open_history: bool, hover: bool| Props {
        message: message.into(),
        open_history,
        hover,
        ..Props::new(state)
    };
    let mut listening = Props::new(BarState::Listening);
    listening.levels = vec![0.6; WAVE_BARS];
    let mut preparing = Props::new(BarState::Transcribing);
    preparing.stage = Stage::Preparing;
    preparing.still = true;
    let mut transcribing = Props::new(BarState::Transcribing);
    transcribing.still = true;
    vec![
        ("listening", listening),
        ("transcribing", transcribing),
        ("preparing", preparing),
        ("pasted", with(BarState::Result, "Pasted", false, false)),
        ("copied", with(BarState::Result, "Copied", false, false)),
        ("undo", with(BarState::Undo, "Pasted", false, false)),
        ("undo hover", with(BarState::Undo, "Pasted", false, true)),
        (
            "cancelled",
            with(BarState::Cancelled, CANCELLED_TEXT, false, false),
        ),
        (
            "cancelled hover",
            with(BarState::Cancelled, CANCELLED_TEXT, false, true),
        ),
        (
            "error history",
            with(BarState::Error, "No speech heard", true, false),
        ),
        (
            "error history hover",
            with(BarState::Error, "No speech heard", true, true),
        ),
        (
            "blocked history",
            with(
                BarState::Blocked,
                "Secure field. Text not pasted",
                true,
                false,
            ),
        ),
        (
            "error plain",
            with(BarState::Error, "Speech model not ready", false, false),
        ),
        (
            "blocked long",
            with(BarState::Blocked, &"Long notice ".repeat(40), false, false),
        ),
    ]
}

struct Drawn {
    window: gpui_kit::AnyWindowHandle,
}

impl Drawn {
    fn open(cx: &mut TestAppContext, props: Props) -> Self {
        cx.update(gpui_kit::init);
        let (window, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        gpui_kit::Point::default(),
                        gpui_kit::size(px(400.0), px(60.0)),
                    ))),
                    ..Default::default()
                },
                cx,
                move |_, cx| cx.new(|_| Preview(props)),
            )
            .expect("open test window")
        });
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.render_frame(cx))
            .unwrap();
        Self { window }
    }

    /// Left, right, top, and bottom of element `id`, in points.
    fn edges(&self, cx: &mut TestAppContext, id: &str) -> Option<(f32, f32, f32, f32)> {
        let id = SharedString::from(format!("flowbar.{id}"));
        cx.update_window(self.window, move |_, window, _| {
            window.try_find(id).map(|found| {
                let bounds: Bounds<Pixels> = found.bounds();
                (
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.x + bounds.size.width),
                    f32::from(bounds.origin.y),
                    f32::from(bounds.origin.y + bounds.size.height),
                )
            })
        })
        .unwrap()
    }
}

#[gpui_kit::test]
fn the_line_sits_in_the_middle_between_the_circles_or_of_the_whole_pill(cx: &mut TestAppContext) {
    for (name, props) in drawn_states() {
        let drawn = Drawn::open(cx, props);
        let bar = drawn.edges(cx, "bar").expect("the pill");
        // The ends of the room: the inner edges of the two circles, or the pill itself.
        let ends = ["mark", "undo", "recover", "open-history"]
            .iter()
            .filter_map(|id| drawn.edges(cx, id).map(|edges| (*id, edges)))
            .collect::<Vec<_>>();
        let (left_end, right_end) = match ends.as_slice() {
            [] => (bar.0, bar.1),
            [(first, left), (_, right)] => {
                assert_eq!(*first, "mark", "{name}: the mark is on the left");
                (left.1, right.0)
            }
            other => panic!("{name}: a circle on one side only: {other:?}"),
        };
        // The content: the line, and the wave when the pill has one.
        let line = drawn.edges(cx, "status");
        let wave = drawn.edges(cx, "wave");
        let (start, stop) = match (wave, line) {
            (Some(wave), Some(line)) => (wave.0.min(line.0), wave.1.max(line.1)),
            (Some(only), None) | (None, Some(only)) => (only.0, only.1),
            (None, None) => panic!("{name}: nothing drawn"),
        };
        let (left, right) = (start - left_end, right_end - stop);
        assert!(
            (left - right).abs() <= 0.5,
            "{name}: {left} pt on the left, {right} pt on the right"
        );
        assert!(left >= 0.0 && right >= 0.0, "{name}: the line overflows");
        for (id, (l, r, t, b)) in ends {
            let inset = if id == "mark" { l - bar.0 } else { bar.1 - r };
            assert_eq!(inset, size::BUTTON_INSET, "{name}: {id} side inset");
            assert_eq!(t - bar.2, size::BUTTON_INSET, "{name}: {id} top inset");
            assert_eq!(bar.3 - b, size::BUTTON_INSET, "{name}: {id} bottom inset");
        }
    }
}

#[gpui_kit::test]
fn the_status_marks_are_images_and_not_buttons(cx: &mut TestAppContext) {
    for (name, props, said) in [
        ("undo", Props::new(BarState::Undo), "Pasted"),
        ("cancelled", Props::new(BarState::Cancelled), CANCELLED_TEXT),
    ] {
        let props = Props {
            message: said.into(),
            ..props
        };
        let drawn = Drawn::open(cx, props);
        let found = cx
            .update_window(drawn.window, |_, window, _| window.try_find("flowbar.mark"))
            .unwrap()
            .expect(name);
        assert_eq!(found.role(), Some(Role::Image), "{name}");
        assert_eq!(found.label(), Some(said), "{name}");
    }
    let drawn = Drawn::open(cx, Props::new(BarState::Cancelled));
    let recover = cx
        .update_window(drawn.window, |_, window, _| {
            window.try_find("flowbar.recover")
        })
        .unwrap()
        .expect("the Recover button");
    assert_eq!(recover.role(), Some(Role::Button));
    assert_eq!(recover.label(), Some(RECOVER_LABEL));
}
