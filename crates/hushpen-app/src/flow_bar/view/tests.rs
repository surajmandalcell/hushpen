use super::*;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext as _, Bounds, Pixels, TestAppContext, WindowBounds, WindowOptions};

/// A look with a line of `width` points.
fn line(state: BarState, message: &str, width: f32) -> Props {
    Props {
        message: message.into(),
        text_width: width,
        ..Props::new(state)
    }
}

#[test]
fn every_pill_fits_its_content_with_fixed_insets() {
    assert_eq!(HEIGHT, 28.0);
    assert_eq!(IDLE_SIZE, (67.0, 22.0));
    assert_eq!(ROUND_END, 4.0 + 20.0 + 8.0);
    assert_eq!(size::INSET, 12.0);
    assert_eq!(size_of(&Props::new(BarState::Idle)), IDLE_SIZE);
    assert_eq!(size_of(&Props::new(BarState::Listening)), (118.0, HEIGHT));
    assert_eq!(
        size_of(&Props::new(BarState::Transcribing)),
        (118.0, HEIGHT)
    );
    let preparing = Props {
        stage: Stage::Preparing,
        text_width: 128.4,
        ..Props::new(BarState::Transcribing)
    };
    assert_eq!(
        size_of(&preparing).0,
        (12.0 + working::width() + 8.0 + 128.4_f32 + 12.0).ceil()
    );
    for (state, ends) in [
        (BarState::Result, 2.0 * 12.0),
        (BarState::Error, 2.0 * 12.0),
        (BarState::Blocked, 2.0 * 12.0),
        (BarState::Undo, 2.0 * ROUND_END),
        (BarState::Cancelled, 2.0 * ROUND_END),
    ] {
        let props = line(state, "Pasted", 38.6);
        assert_eq!(
            size_of(&props),
            ((ends + 38.6_f32).ceil(), HEIGHT),
            "{state:?}"
        );
    }
    let history = Props {
        open_history: true,
        ..line(BarState::Error, "No speech heard", 92.2)
    };
    assert_eq!(size_of(&history).0, (2.0 * ROUND_END + 92.2_f32).ceil());
}

#[test]
fn a_long_line_stops_at_the_widest_pill() {
    let props = line(BarState::Blocked, &"x".repeat(200), 1_200.0);
    assert_eq!(size_of(&props), (MAX_WIDTH, HEIGHT));
    assert_eq!(MAX_WIDTH, 366.0);
    assert_eq!(
        line_room(&props),
        MAX_WIDTH - 24.0,
        "the line truncates there"
    );
}

#[test]
fn the_room_fits_every_line_the_pill_can_show_so_a_hover_never_resizes_it() {
    let undo = line(BarState::Undo, "Pasted", 0.0);
    assert_eq!(undo.lines(), ["Pasted", UNDO_HOVER]);
    assert!(UNDO_HOVER.len() < "Pasted".len().min("Copied".len()));
    let cancelled = line(BarState::Cancelled, CANCELLED_TEXT, 0.0);
    assert_eq!(cancelled.lines(), [CANCELLED_TEXT, RECOVER_LABEL]);
    let history = Props {
        open_history: true,
        ..line(BarState::Error, "No speech heard", 0.0)
    };
    assert_eq!(history.lines(), ["No speech heard", HISTORY_LABEL]);
    assert!(Props::new(BarState::Listening).lines().is_empty());
}

#[test]
fn the_listening_pill_fits_the_cross_the_wave_and_the_tick() {
    let inner = 2.0 * ROUND_END + working::width();
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
    assert_eq!(props.lines(), ["Preparing the AI model"]);
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
        // The pill is as wide as its content: the line has its fixed gap and no more.
        if wave.is_none() && name != "blocked long" && !name.contains("hover") {
            let gap = if ends.is_empty() {
                size::INSET
            } else {
                space::SM
            };
            assert!(
                left >= gap - 0.01 && left < gap + 1.0,
                "{name}: {left} pt beside the line, want {gap}"
            );
        }
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

#[gpui_kit::test]
fn the_processing_wave_stands_where_the_listening_wave_stood(cx: &mut TestAppContext) {
    let mut listening = Props::new(BarState::Listening);
    listening.levels = vec![0.6; WAVE_BARS];
    let listened = Drawn::open(cx, listening).edges(cx, "wave").unwrap();
    let mut transcribing = Props::new(BarState::Transcribing);
    transcribing.still = true;
    let processed = Drawn::open(cx, transcribing).edges(cx, "wave").unwrap();
    assert_eq!(listened.0, processed.0);
    assert_eq!(listened.1, processed.1);
}

#[test]
fn a_turning_wave_starts_from_the_old_bars_and_ends_on_the_new_ones() {
    let mut listening = Props::new(BarState::Listening);
    listening.levels = vec![0.9; WAVE_BARS];
    let from = drawn_wave(&listening).unwrap();
    let mut turning = Props::new(BarState::Transcribing);
    turning.still = true;
    let to = drawn_wave(&turning).unwrap();
    turning.morph = Some((from.clone(), 0.0));
    assert_eq!(drawn_wave(&turning).unwrap(), from);
    turning.morph = Some((from, 1.0));
    assert_eq!(drawn_wave(&turning).unwrap(), to);
    assert!(morphs(&listening) && morphs(&turning));
    turning.stage = Stage::Preparing;
    assert!(
        !morphs(&turning),
        "the model wait has its line beside the wave"
    );
}
