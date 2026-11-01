//! The motion of the flow bar, frame by frame: how it comes and goes, and that no change of
//! state jumps.

use super::*;
use crate::flow_bar::working::{self, Stage};
use hushpen_core::flow_bar::BarState;

const FRAME_MS: u64 = 16;

/// The most a frame of 16 ms may change. A fade of 100 ms has six frames, so its steps are
/// limited and smooth, not zero.
const MAX_CONTENT_STEP: f32 = 0.25;
const MAX_PRESENCE_STEP: f32 = 0.31;
const MAX_SCALE_STEP: f32 = 0.025;
/// The share of a size change one frame may cover, and the most the first frame may move.
const MAX_WIDTH_SHARE: f32 = 0.14;
const MAX_FIRST_WIDTH_SHARE: f32 = 0.06;
/// The most a waveform bar may grow, shrink, or move in a frame while one wave turns into the
/// next. The processing wave itself moves up to 2.5 pt in a frame.
const MAX_BAR_STEP: f32 = 4.5;

fn with_text(state: BarState, message: &str, width: f32) -> Props {
    Props {
        message: message.into(),
        text_width: width,
        ..Props::new(state)
    }
}

/// Every look the bar can take, by name, with line widths close to the real font.
fn looks() -> Vec<(&'static str, Props)> {
    let transcribing = Props {
        work_ms: 300,
        ..Props::new(BarState::Transcribing)
    };
    // A new stage blends from what the wave showed, as the live bar starts it.
    let cleaning = Props {
        stage: Stage::Cleaning,
        from: Some(working::mix_at(Stage::Transcribing, None, 300)),
        ..transcribing.clone()
    };
    let preparing = Props {
        stage: Stage::Preparing,
        text_width: 128.0,
        ..transcribing.clone()
    };
    let notice = with_text(BarState::Error, "No speech heard", 93.0);
    let history = Props {
        open_history: true,
        ..notice.clone()
    };
    vec![
        ("idle", Props::new(BarState::Idle)),
        ("listening", Props::new(BarState::Listening)),
        ("transcribing", transcribing),
        ("cleaning", cleaning),
        ("preparing", preparing),
        ("pasted", with_text(BarState::Result, "Pasted", 39.0)),
        ("copied", with_text(BarState::Result, "Copied", 41.0)),
        ("undo", with_text(BarState::Undo, "Pasted", 39.0)),
        (
            "cancelled",
            with_text(BarState::Cancelled, "Cancelled", 57.0),
        ),
        ("notice", notice),
        ("notice with history", history),
    ]
}

fn wave_for(props: &Props) -> Vec<f32> {
    if props.state == BarState::Listening {
        vec![0.7; view::WAVE_BARS]
    } else {
        Vec::new()
    }
}

/// Runs `frames` frames of 16 ms from `from_ms` toward `props` and returns each drawn frame.
fn run(
    motion: &mut Motion,
    from_ms: u64,
    frames: u64,
    props: &Props,
    shown: bool,
    mode: Mode,
) -> Vec<(u64, Props)> {
    (1..=frames)
        .map(|n| {
            let now = from_ms + n * FRAME_MS;
            let drawn = motion.frame(now, props.clone(), shown, &wave_for(props), mode);
            (now, drawn)
        })
        .collect()
}

/// A motion that shows `props`, settled.
fn settled(props: &Props, mode: Mode) -> (Motion, u64) {
    let mut motion = Motion::default();
    let frames = run(&mut motion, 0, 60, props, true, mode);
    assert!(motion.is_settled(), "settled");
    (motion, frames.last().unwrap().0)
}

fn width(props: &Props) -> f32 {
    props.frame.unwrap().0
}

/// The opacity the content of the frame shows with: the old content or the new.
fn content(props: &Props) -> f32 {
    if props.prev_fade > 0.0 {
        props.prev_fade
    } else {
        props.fade
    }
}

/// Which look the content of the frame is.
fn shown_look(props: &Props) -> (BarState, String, bool) {
    let look = match &props.prev {
        Some(prev) if props.prev_fade > 0.0 => prev,
        _ => props,
    };
    (look.state, look.message.clone(), look.preparing())
}

#[test]
fn a_new_bar_fades_and_grows_in_at_its_size_within_180_ms() {
    let listening = Props::new(BarState::Listening);
    let mut motion = Motion::default();
    let first = motion.frame(0, listening.clone(), true, &[], Mode::Full);
    assert_eq!(first.presence, 0.0, "the first frame of a new bar is clear");
    assert_eq!(
        width(&first),
        view::LISTENING_SIZE.0,
        "no growing out of a circle"
    );
    assert_eq!(first.scale, SCALE_FROM);
    let mut last = first;
    for (now, frame) in run(&mut motion, 0, 12, &listening, true, Mode::Full) {
        assert!(frame.presence >= last.presence && frame.presence <= 1.0);
        assert!(frame.presence - last.presence <= MAX_PRESENCE_STEP, "{now}");
        assert!(frame.scale >= last.scale && frame.scale <= 1.0);
        assert_eq!(width(&frame), view::LISTENING_SIZE.0);
        last = frame;
        if now >= 176 {
            assert_eq!(last.presence, 1.0, "in after {now} ms");
        }
    }
    assert!(motion.is_settled());
}

#[test]
fn a_bar_that_goes_fades_and_shrinks_a_little_in_place_and_never_shows_again_once_clear() {
    for (name, props) in looks() {
        let (mut motion, now) = settled(&props, Mode::Full);
        let frames = run(&mut motion, now, 20, &props, false, Mode::Full);
        let mut last = 1.0;
        let mut clear_at = None;
        for (at, frame) in &frames {
            assert!(frame.presence <= last, "{name}: the fade goes back at {at}");
            assert!(
                last - frame.presence <= MAX_PRESENCE_STEP,
                "{name} at {at}: {last} to {}",
                frame.presence
            );
            assert_eq!(
                frame.frame,
                Some(view::size_of(&props)),
                "{name}: the window keeps its size"
            );
            assert!((frame.scale - (SCALE_FROM + 0.04 * frame.presence)).abs() < 1e-4);
            if let Some(clear) = clear_at {
                assert_eq!(
                    frame.presence, 0.0,
                    "{name}: drawn again at {at} after {clear}"
                );
            } else if frame.presence == 0.0 {
                clear_at = Some(*at);
            }
            last = frame.presence;
        }
        let clear = clear_at.expect("it fades out to nothing");
        assert!(clear - now <= 176, "{name}: clear after {} ms", clear - now);
        assert!(motion.is_gone());
        assert_eq!(frames.last().unwrap().1.scale, SCALE_FROM);
    }
}

#[test]
fn the_fade_turns_back_without_a_jump_when_the_bar_is_wanted_again() {
    let pasted = with_text(BarState::Result, "Pasted", 39.0);
    let listening = Props::new(BarState::Listening);
    let (mut motion, now) = settled(&pasted, Mode::Full);
    let going = run(&mut motion, now, 4, &pasted, false, Mode::Full);
    let (at, half) = going.last().unwrap().clone();
    assert!(half.presence > 0.0 && half.presence < 1.0);
    let back = run(&mut motion, at, 12, &listening, true, Mode::Full);
    let mut last = half.presence;
    for (_, frame) in &back {
        assert!((frame.presence - last).abs() <= MAX_PRESENCE_STEP);
        last = frame.presence;
    }
    assert_eq!(last, 1.0);
}

/// Every change from one look to another, frame by frame at 60 frames a second.
#[test]
fn no_change_between_two_looks_jumps_in_size_opacity_scale_or_wave() {
    for (from_name, from) in looks() {
        for (to_name, to) in looks() {
            if from_name == to_name {
                continue;
            }
            let name = format!("{from_name} -> {to_name}");
            let (mut motion, now) = settled(&from, Mode::Full);
            let before = motion.frame(now + 1, from.clone(), true, &wave_for(&from), Mode::Full);
            let frames = run(&mut motion, now + 1, 50, &to, true, Mode::Full);
            let (w0, w1) = (width(&before), view::size_of(&to).0);
            let change = (w1 - w0).abs();
            let mut last = before;
            for (index, (at, frame)) in frames.iter().enumerate() {
                let step = (width(frame) - width(&last)).abs();
                let share = if index == 0 {
                    MAX_FIRST_WIDTH_SHARE
                } else {
                    MAX_WIDTH_SHARE
                };
                assert!(
                    // The last step may snap the half point the spring calls settled.
                    step <= change * share + 0.6,
                    "{name}: the width moves {step} at {at} (frame {index}, {} to {})",
                    width(&last),
                    width(frame)
                );
                let lo = w0.min(w1) - 0.01;
                let hi = w0.max(w1) + 0.01;
                assert!(
                    (lo..=hi).contains(&width(frame)),
                    "{name}: overshoot to {} at {at}",
                    width(frame)
                );
                assert!(
                    !(frame.prev_fade > 0.0 && frame.fade > 0.0),
                    "{name}: two contents at once at {at}"
                );
                if shown_look(frame) == shown_look(&last) {
                    let step = (content(frame) - content(&last)).abs();
                    assert!(step <= MAX_CONTENT_STEP, "{name}: content {step} at {at}");
                } else {
                    assert!(
                        content(frame) <= MAX_CONTENT_STEP && content(&last) <= MAX_CONTENT_STEP,
                        "{name}: the content switches while it shows at {at}"
                    );
                }
                // The marks of a look draw only while its own content shows.
                if frame.prev_fade == 0.0 && last.prev_fade == 0.0 && same_look(frame, &last) {
                    assert!(
                        (frame.marks - last.marks).abs() <= MAX_CONTENT_STEP,
                        "{name}"
                    );
                    assert!(
                        (frame.mark_scale - last.mark_scale).abs() <= MAX_SCALE_STEP,
                        "{name}: marks scale at {at}"
                    );
                }
                if let (Some(a), Some(b)) = (view::drawn_wave(&last), view::drawn_wave(frame))
                    && view::morphs(&last)
                    && view::morphs(frame)
                {
                    for (a, b) in a.iter().zip(&b) {
                        assert!(
                            (a.height - b.height).abs() <= MAX_BAR_STEP
                                && (a.lift - b.lift).abs() <= MAX_BAR_STEP,
                            "{name}: a wave bar jumps at {at}: {a:?} {b:?}"
                        );
                        assert!((a.alpha - b.alpha).abs() <= MAX_CONTENT_STEP, "{name}");
                    }
                }
                last = frame.clone();
            }
            assert!(motion.is_settled(), "{name}: still moving after 800 ms");
            assert_eq!(width(&last), w1, "{name}: lands on the new size");
            assert_eq!((last.fade, last.marks, last.prev_fade), (1.0, 1.0, 0.0));
        }
    }
}

#[test]
fn a_listening_wave_turns_into_the_processing_wave_where_it_stands() {
    let listening = Props::new(BarState::Listening);
    let transcribing = Props::new(BarState::Transcribing);
    let (mut motion, now) = settled(&listening, Mode::Full);
    let last = motion.frame(
        now + 1,
        listening.clone(),
        true,
        &wave_for(&listening),
        Mode::Full,
    );
    let listened = view::drawn_wave(&last).unwrap();
    let first = motion.frame(now + 2, transcribing.clone(), true, &[], Mode::Full);
    let (from, done) = first.morph.clone().expect("the wave turns");
    assert_eq!(from, listened, "it starts from the bars as they stood");
    assert!(done < 0.01);
    assert_eq!(
        view::drawn_wave(&first).unwrap(),
        listened,
        "no pop on the first frame"
    );
    assert_eq!(
        first.prev.as_ref().map(|prev| prev.state),
        Some(BarState::Listening)
    );
    assert!(
        first.prev_fade > 0.9,
        "the circles fade out around the wave"
    );
    let frames = run(&mut motion, now + 2, 30, &transcribing, true, Mode::Full);
    assert!(frames.iter().any(|(_, frame)| frame.morph.is_some()));
    assert!(frames.last().unwrap().1.morph.is_none(), "the turn ends");
}

#[test]
fn the_marks_come_in_after_the_size_has_settled() {
    let transcribing = Props::new(BarState::Transcribing);
    let undo = with_text(BarState::Undo, "Pasted", 39.0);
    let (mut motion, now) = settled(&transcribing, Mode::Full);
    let frames = run(&mut motion, now, 40, &undo, true, Mode::Full);
    let goal = view::size_of(&undo).0;
    for (at, frame) in &frames {
        if frame.marks > 0.0 {
            assert!(
                (width(frame) - goal).abs() <= 0.05 * (118.0 - goal),
                "marks at {at} while the width is {}",
                width(frame)
            );
            assert!(frame.mark_scale >= MARK_FROM);
        }
    }
    let first_in = frames
        .iter()
        .find(|(_, frame)| frame.marks > 0.0)
        .unwrap()
        .0;
    assert!(
        first_in - now >= MARKS_AFTER_MS as u64,
        "marks in after {} ms",
        first_in - now
    );
    assert_eq!(frames.last().unwrap().1.mark_scale, 1.0);
}

#[test]
fn reduce_motion_only_crossfades_and_changes_the_size_while_the_pill_is_empty() {
    for (from_name, from) in looks() {
        for (to_name, to) in looks() {
            if from_name == to_name {
                continue;
            }
            let (mut motion, now) = settled(&from, Mode::Crossfade);
            let mut last = motion.frame(now + 1, from.clone(), true, &[], Mode::Crossfade);
            for (at, frame) in run(&mut motion, now + 1, 30, &to, true, Mode::Crossfade) {
                assert_eq!((frame.scale, frame.mark_scale), (1.0, 1.0));
                assert!(frame.morph.is_none());
                if width(&frame) != width(&last) {
                    assert!(
                        content(&frame) < 0.05 && content(&last) < 0.05,
                        "{from_name} -> {to_name}: resized with content at {at}"
                    );
                }
                last = frame;
            }
            assert_eq!(width(&last), view::size_of(&to).0);
        }
    }
    let pasted = with_text(BarState::Result, "Pasted", 39.0);
    let (mut motion, now) = settled(&pasted, Mode::Crossfade);
    let gone = motion.frame(now + 16, pasted, false, &[], Mode::Crossfade);
    assert_eq!(gone.presence, 0.0, "Reduce Motion hides at once");
    assert!(motion.is_gone());
}

#[test]
fn the_waveform_rises_fast_and_falls_slow() {
    let listening = Props::new(BarState::Listening);
    let mut motion = Motion::default();
    for n in 0..4 {
        motion.frame(n * FRAME_MS, listening.clone(), true, &[1.0], Mode::Full);
    }
    let up = motion.wave[0];
    assert!(up > 0.7, "rose to {up}");
    for n in 4..8 {
        motion.frame(n * FRAME_MS, listening.clone(), true, &[0.0], Mode::Full);
    }
    let down = motion.wave[0];
    assert!(down > 0.2 && down < up, "fell to {down}");
}

#[test]
fn a_stall_does_not_replay_and_instant_mode_jumps_to_the_goal() {
    let undo = with_text(BarState::Undo, "Pasted", 39.0);
    let (mut motion, now) = settled(&Props::new(BarState::Listening), Mode::Full);
    motion.frame(now + 1, undo.clone(), true, &[], Mode::Full);
    let after = motion.frame(now + 10_000, undo.clone(), true, &[], Mode::Full);
    assert!(
        width(&after) > view::size_of(&undo).0,
        "one stalled step is at most 50 ms"
    );

    let mut instant = Motion::default();
    let drawn = instant.frame(0, undo.clone(), true, &[], Mode::Instant);
    assert_eq!(drawn.frame, Some(view::size_of(&undo)));
    assert_eq!((drawn.presence, drawn.fade, drawn.marks), (1.0, 1.0, 1.0));
    assert!(instant.is_settled());
    let gone = instant.frame(16, undo, false, &[], Mode::Instant);
    assert_eq!(gone.presence, 0.0);
    assert!(instant.is_gone());
}
