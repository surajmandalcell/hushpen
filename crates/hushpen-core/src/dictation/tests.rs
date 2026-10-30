use super::*;
use crate::error::{
    CAPTURE_FAILED, ENGINE_CRASHED, ENGINE_NO_SPEECH, INSERT_SECURE_FIELD, LLM_TIMEOUT,
};

const RAW: &str = "hello world";
const CLEAN: &str = "Hello world.";
/// A moment well past the tap limit, so a hold released now is a recording.
const LATER: u64 = 1_000;

fn machine() -> DictationMachine {
    DictationMachine::new(Config::default())
}

fn handle(machine: &mut DictationMachine, event: AppEvent, now: u64) -> Vec<Effect> {
    machine.handle(event, now)
}

fn hold(machine: &mut DictationMachine) {
    handle(machine, AppEvent::HoldDown, 0);
}

fn transcribed(session: u64) -> AppEvent {
    AppEvent::Transcribed {
        session,
        text: RAW.into(),
    }
}

fn cleaned(session: u64) -> AppEvent {
    AppEvent::Cleaned {
        session,
        text: CLEAN.into(),
    }
}

/// Drives a fresh machine into `state` by real events, so the table never pokes private fields.
fn reach(state: State) -> DictationMachine {
    let mut m = machine();
    if state == State::Idle {
        return m;
    }
    hold(&mut m);
    match state {
        State::Listening => return m,
        State::Cancelled => {
            handle(&mut m, AppEvent::Esc, LATER);
            return m;
        }
        State::Failed => {
            handle(
                &mut m,
                AppEvent::CaptureError {
                    code: CAPTURE_FAILED,
                },
                LATER,
            );
            return m;
        }
        _ => {}
    }
    handle(&mut m, AppEvent::HoldUp, LATER);
    if state == State::Transcribing {
        return m;
    }
    handle(&mut m, transcribed(1), LATER);
    if state == State::Cleaning {
        return m;
    }
    handle(&mut m, cleaned(1), LATER);
    if state == State::Inserting {
        return m;
    }
    handle(&mut m, AppEvent::Inserted { session: 1 }, LATER);
    m
}

fn every_event() -> Vec<AppEvent> {
    vec![
        AppEvent::HoldDown,
        AppEvent::HoldArm,
        AppEvent::HoldUp,
        AppEvent::HandsFreeToggle,
        AppEvent::HomeToggle,
        AppEvent::FlowBarClick,
        AppEvent::Esc,
        AppEvent::PasteLast,
        AppEvent::Tick,
        AppEvent::CaptureError {
            code: CAPTURE_FAILED,
        },
        transcribed(1),
        AppEvent::TranscribeFailed {
            session: 1,
            code: ENGINE_CRASHED,
        },
        cleaned(1),
        AppEvent::Inserted { session: 1 },
        AppEvent::InsertFailed {
            session: 1,
            code: INSERT_SECURE_FIELD,
        },
        AppEvent::CommandDown,
        AppEvent::CommandUp,
        AppEvent::EditFailed {
            session: 1,
            code: LLM_TIMEOUT,
        },
        AppEvent::ReadAloud,
    ]
}

/// Forces `every_event` to name every variant: a new variant breaks this match first.
fn variant_index(event: &AppEvent) -> usize {
    match event {
        AppEvent::HoldDown => 0,
        AppEvent::HoldUp => 1,
        AppEvent::HandsFreeToggle => 2,
        AppEvent::HomeToggle => 3,
        AppEvent::FlowBarClick => 4,
        AppEvent::Esc => 5,
        AppEvent::Tick => 6,
        AppEvent::CaptureError { .. } => 7,
        AppEvent::Transcribed { .. } => 8,
        AppEvent::TranscribeFailed { .. } => 9,
        AppEvent::Cleaned { .. } => 10,
        AppEvent::Inserted { .. } => 11,
        AppEvent::InsertFailed { .. } => 12,
        AppEvent::PasteLast => 13,
        AppEvent::CommandDown => 14,
        AppEvent::CommandUp => 15,
        AppEvent::EditFailed { .. } => 16,
        AppEvent::HoldArm => 17,
        AppEvent::ReadAloud => 18,
    }
}

const VARIANTS: usize = 19;

fn start_effects(session: u64) -> Vec<Effect> {
    vec![Effect::StartCapture { session }, Effect::Cue(Cue::Start)]
}

fn stop_effects() -> Vec<Effect> {
    vec![
        Effect::StopCapture {
            session: 1,
            keep: true,
        },
        Effect::Cue(Cue::Stop),
        Effect::Transcribe { session: 1 },
    ]
}

fn failed_row(text: Option<&str>, code: &'static str) -> Effect {
    Effect::SaveRow {
        session: 1,
        status: RowStatus::Failed,
        text: text.map(str::to_owned),
        code: Some(code),
    }
}

fn notify(code: &'static str) -> Effect {
    Effect::Notify { session: 1, code }
}

const STARTS: [AppEvent; 5] = [
    AppEvent::HoldDown,
    AppEvent::CommandDown,
    AppEvent::HandsFreeToggle,
    AppEvent::HomeToggle,
    AppEvent::FlowBarClick,
];

/// Every pair that is not a no-op. The test below checks all other pairs change nothing.
fn expected() -> Vec<(State, AppEvent, State, Vec<Effect>)> {
    use AppEvent as E;
    let mut rows = Vec::new();
    for start in STARTS {
        rows.push((State::Idle, start, State::Listening, start_effects(1)));
    }
    // A start never waits for the session before it, whatever that one is doing.
    for from in [
        State::Transcribing,
        State::Cleaning,
        State::Inserting,
        State::Done,
        State::Cancelled,
        State::Failed,
    ] {
        for start in STARTS {
            rows.push((from, start, State::Listening, start_effects(2)));
        }
    }
    rows.push((State::Listening, E::CommandDown, State::Listening, vec![]));
    rows.push((
        State::Cleaning,
        E::EditFailed {
            session: 1,
            code: LLM_TIMEOUT,
        },
        State::Failed,
        vec![failed_row(None, LLM_TIMEOUT), notify(LLM_TIMEOUT)],
    ));
    for resting in [State::Idle, State::Done, State::Cancelled, State::Failed] {
        rows.push((resting, E::PasteLast, resting, vec![Effect::PasteLast]));
    }
    rows.push((
        State::Listening,
        E::HoldUp,
        State::Transcribing,
        stop_effects(),
    ));
    rows.push((
        State::Listening,
        E::HandsFreeToggle,
        State::Listening,
        vec![],
    ));
    for stop in [E::HomeToggle, E::FlowBarClick] {
        rows.push((State::Listening, stop, State::Transcribing, stop_effects()));
    }
    rows.push((
        State::Listening,
        E::Esc,
        State::Cancelled,
        vec![
            Effect::StopCapture {
                session: 1,
                keep: false,
            },
            Effect::Cue(Cue::Cancel),
        ],
    ));
    rows.push((
        State::Listening,
        E::CaptureError {
            code: CAPTURE_FAILED,
        },
        State::Failed,
        vec![
            Effect::StopCapture {
                session: 1,
                keep: true,
            },
            failed_row(None, CAPTURE_FAILED),
            notify(CAPTURE_FAILED),
        ],
    ));
    rows.push((
        State::Transcribing,
        E::Esc,
        State::Cancelled,
        vec![
            Effect::CancelTranscribe { session: 1 },
            Effect::Cue(Cue::Cancel),
            Effect::SaveRow {
                session: 1,
                status: RowStatus::Cancelled,
                text: None,
                code: None,
            },
        ],
    ));
    rows.push((
        State::Transcribing,
        transcribed(1),
        State::Cleaning,
        vec![Effect::Clean {
            session: 1,
            raw: RAW.into(),
        }],
    ));
    rows.push((
        State::Transcribing,
        E::TranscribeFailed {
            session: 1,
            code: ENGINE_CRASHED,
        },
        State::Failed,
        vec![failed_row(None, ENGINE_CRASHED), notify(ENGINE_CRASHED)],
    ));
    rows.push((
        State::Cleaning,
        E::Esc,
        State::Cancelled,
        vec![
            Effect::StopLlm { session: 1 },
            Effect::Cue(Cue::Cancel),
            Effect::SaveRow {
                session: 1,
                status: RowStatus::Cancelled,
                text: Some(RAW.into()),
                code: None,
            },
        ],
    ));
    rows.push((
        State::Cleaning,
        cleaned(1),
        State::Inserting,
        vec![Effect::Insert {
            session: 1,
            text: CLEAN.into(),
            delivery: Delivery::Paste,
        }],
    ));
    rows.push((
        State::Inserting,
        E::Inserted { session: 1 },
        State::Done,
        vec![
            Effect::SaveRow {
                session: 1,
                status: RowStatus::Done,
                text: Some(CLEAN.into()),
                code: None,
            },
            Effect::UpdatePasteLast { text: CLEAN.into() },
        ],
    ));
    rows.push((
        State::Inserting,
        E::InsertFailed {
            session: 1,
            code: INSERT_SECURE_FIELD,
        },
        State::Failed,
        vec![
            failed_row(Some(CLEAN), INSERT_SECURE_FIELD),
            notify(INSERT_SECURE_FIELD),
        ],
    ));
    rows
}

#[test]
fn every_state_and_event_pair_has_an_asserted_outcome() {
    let events = every_event();
    let mut indices: Vec<usize> = events.iter().map(variant_index).collect();
    indices.sort_unstable();
    assert_eq!(indices, (0..VARIANTS).collect::<Vec<_>>());

    let rows = expected();
    let mut pairs = 0;
    for state in State::ALL {
        for event in &events {
            let mut m = reach(state);
            assert_eq!(m.state(), state, "setup for {state:?}");
            let effects = handle(&mut m, event.clone(), LATER);
            let row = rows
                .iter()
                .find(|(from, on, ..)| *from == state && variant_index(on) == variant_index(event));
            match row {
                Some((_, _, next, want)) => {
                    assert_eq!(m.state(), *next, "{state:?} on {event:?}");
                    assert_eq!(&effects, want, "{state:?} on {event:?}");
                }
                None => {
                    assert_eq!(m.state(), state, "{state:?} on {event:?} must not move");
                    assert!(effects.is_empty(), "{state:?} on {event:?}: {effects:?}");
                }
            }
            pairs += 1;
        }
    }
    assert_eq!(pairs, State::ALL.len() * VARIANTS);
    assert_eq!(pairs, 152);
}

#[test]
fn a_start_event_while_listening_never_begins_a_second_session() {
    for start in STARTS {
        let mut m = reach(State::Listening);
        let effects = handle(&mut m, start.clone(), LATER);
        assert_eq!(m.session(), 1, "{start:?}");
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::StartCapture { .. })),
            "{start:?}"
        );
    }
}

#[test]
fn a_start_50_ms_after_a_stop_begins_the_next_session_at_once() {
    for (state, still_running) in [
        (State::Transcribing, true),
        (State::Cleaning, true),
        (State::Inserting, true),
        (State::Done, false),
        (State::Cancelled, false),
        (State::Failed, false),
    ] {
        for start in STARTS {
            let mut m = reach(state);
            let effects = handle(&mut m, start.clone(), LATER + 50);
            assert_eq!(effects, start_effects(2), "{state:?} on {start:?}");
            assert_eq!(m.state(), State::Listening, "{state:?} on {start:?}");
            assert_eq!(m.session(), 2, "{state:?} on {start:?}");
            assert_eq!(
                m.behind(),
                usize::from(still_running),
                "{state:?} on {start:?}"
            );
        }
    }
}

fn said(session: u64, text: &str) -> AppEvent {
    AppEvent::Transcribed {
        session,
        text: text.into(),
    }
}

fn polished(session: u64, text: &str) -> AppEvent {
    AppEvent::Cleaned {
        session,
        text: text.into(),
    }
}

fn done_row(session: u64, text: &str) -> Effect {
    Effect::SaveRow {
        session,
        status: RowStatus::Done,
        text: Some(text.into()),
        code: None,
    }
}

fn paste(session: u64, text: &str) -> Effect {
    Effect::Insert {
        session,
        text: text.into(),
        delivery: Delivery::Paste,
    }
}

/// Session 1 stops at `LATER` and is transcribing; session 2 starts 30 ms later and listens.
fn two_quick_holds() -> DictationMachine {
    let mut m = reach(State::Transcribing);
    handle(&mut m, AppEvent::HoldDown, LATER + 30);
    assert_eq!(
        (m.state(), m.session(), m.behind()),
        (State::Listening, 2, 1)
    );
    m
}

#[test]
fn two_quick_dictations_both_go_in_in_the_order_they_were_spoken() {
    let mut m = two_quick_holds();
    handle(&mut m, AppEvent::HoldUp, 2 * LATER);
    // The second one is quicker, but its text waits for the first one.
    assert_eq!(
        handle(&mut m, said(2, "second take"), 2 * LATER),
        vec![Effect::Clean {
            session: 2,
            raw: "second take".into()
        }]
    );
    assert!(handle(&mut m, polished(2, "Second."), 2 * LATER).is_empty());
    assert_eq!(m.state(), State::Inserting);
    handle(&mut m, said(1, "first take"), 2 * LATER + 10);
    assert_eq!(
        handle(&mut m, polished(1, "First."), 2 * LATER + 20),
        vec![paste(1, "First.")]
    );
    assert_eq!(
        handle(&mut m, AppEvent::Inserted { session: 1 }, 2 * LATER + 30),
        vec![
            done_row(1, "First."),
            Effect::UpdatePasteLast {
                text: "First.".into()
            },
            paste(2, "Second."),
        ]
    );
    assert_eq!(
        m.state(),
        State::Inserting,
        "no result flash for the first one"
    );
    assert_eq!(
        handle(&mut m, AppEvent::Inserted { session: 2 }, 2 * LATER + 40),
        vec![
            done_row(2, "Second."),
            Effect::UpdatePasteLast {
                text: "Second.".into()
            },
        ]
    );
    assert_eq!((m.state(), m.behind()), (State::Done, 0));
}

#[test]
fn the_new_session_shows_listening_while_the_older_one_finishes() {
    let mut m = reach(State::Inserting);
    handle(&mut m, AppEvent::HandsFreeToggle, LATER + 20);
    assert_eq!(
        handle(&mut m, AppEvent::Inserted { session: 1 }, LATER + 40),
        vec![
            done_row(1, CLEAN),
            Effect::UpdatePasteLast { text: CLEAN.into() }
        ]
    );
    assert_eq!(
        (m.state(), m.session(), m.behind()),
        (State::Listening, 2, 0)
    );
    assert!(handle(&mut m, AppEvent::Tick, LATER + 5_000).is_empty());
    assert_eq!(m.state(), State::Listening, "the old result never flashes");
}

#[test]
fn cancelling_the_new_session_leaves_the_older_one_running() {
    let mut m = two_quick_holds();
    assert_eq!(
        handle(&mut m, AppEvent::Esc, LATER + 600),
        vec![
            Effect::StopCapture {
                session: 2,
                keep: false
            },
            Effect::Cue(Cue::Cancel),
        ]
    );
    assert_eq!((m.state(), m.behind()), (State::Cancelled, 1));
    handle(&mut m, transcribed(1), LATER + 700);
    assert_eq!(
        handle(&mut m, cleaned(1), LATER + 700),
        vec![paste(1, CLEAN)]
    );
    assert_eq!(
        handle(&mut m, AppEvent::Inserted { session: 1 }, LATER + 800),
        vec![
            done_row(1, CLEAN),
            Effect::UpdatePasteLast { text: CLEAN.into() }
        ]
    );
    assert_eq!((m.state(), m.behind()), (State::Cancelled, 0));
}

#[test]
fn esc_cancels_the_newest_session_and_never_an_older_one_behind_it() {
    let mut m = reach(State::Cleaning);
    handle(&mut m, AppEvent::HoldDown, LATER + 10);
    handle(&mut m, AppEvent::HoldUp, 2 * LATER);
    assert_eq!(
        handle(&mut m, AppEvent::Esc, 2 * LATER + 10),
        vec![
            Effect::CancelTranscribe { session: 2 },
            Effect::Cue(Cue::Cancel),
            Effect::SaveRow {
                session: 2,
                status: RowStatus::Cancelled,
                text: None,
                code: None
            },
        ]
    );
    assert_eq!(
        handle(&mut m, cleaned(1), 2 * LATER + 20),
        vec![paste(1, CLEAN)]
    );
}

#[test]
fn once_the_newest_session_ends_the_older_one_comes_back_and_esc_cancels_it() {
    let mut m = two_quick_holds();
    handle(&mut m, AppEvent::Esc, LATER + 600);
    handle(
        &mut m,
        AppEvent::Tick,
        LATER + 600 + Config::default().cancelled_ms,
    );
    assert_eq!(
        (m.state(), m.session(), m.behind()),
        (State::Transcribing, 1, 0)
    );
    assert_eq!(
        handle(&mut m, AppEvent::Esc, 3 * LATER),
        vec![
            Effect::CancelTranscribe { session: 1 },
            Effect::Cue(Cue::Cancel),
            Effect::SaveRow {
                session: 1,
                status: RowStatus::Cancelled,
                text: None,
                code: None
            },
        ]
    );
    assert!(handle(&mut m, transcribed(1), 3 * LATER).is_empty());
    assert_eq!(
        handle(&mut m, AppEvent::HomeToggle, 3 * LATER + 10),
        start_effects(3),
        "a new session never reuses an id"
    );
}

#[test]
fn an_older_session_that_fails_lets_the_next_text_go_in() {
    let mut m = two_quick_holds();
    handle(&mut m, AppEvent::HoldUp, 2 * LATER);
    handle(&mut m, transcribed(2), 2 * LATER);
    assert!(handle(&mut m, cleaned(2), 2 * LATER).is_empty());
    let failed = AppEvent::TranscribeFailed {
        session: 1,
        code: ENGINE_CRASHED,
    };
    assert_eq!(
        handle(&mut m, failed, 2 * LATER + 10),
        vec![
            failed_row(None, ENGINE_CRASHED),
            notify(ENGINE_CRASHED),
            paste(2, CLEAN),
        ]
    );
    assert_eq!((m.state(), m.session()), (State::Inserting, 2));
}

#[test]
fn a_tap_while_the_older_session_finishes_brings_it_back_and_a_double_tap_starts_hands_free() {
    let mut m = reach(State::Transcribing);
    handle(&mut m, AppEvent::HoldDown, LATER + 100);
    assert_eq!(
        handle(&mut m, AppEvent::HoldUp, LATER + 150),
        vec![Effect::StopCapture {
            session: 2,
            keep: false
        }]
    );
    assert_eq!(
        (m.state(), m.session(), m.behind()),
        (State::Transcribing, 1, 0)
    );
    assert_eq!(
        handle(&mut m, AppEvent::HoldDown, LATER + 300),
        start_effects(3)
    );
    assert_eq!((m.mode(), m.behind()), (Mode::HandsFree, 1));
}

#[test]
fn paste_last_waits_until_no_older_session_finishes() {
    let mut m = two_quick_holds();
    handle(
        &mut m,
        AppEvent::CaptureError {
            code: CAPTURE_FAILED,
        },
        LATER + 40,
    );
    assert_eq!(m.state(), State::Failed);
    assert!(handle(&mut m, AppEvent::PasteLast, LATER + 50).is_empty());
    handle(&mut m, transcribed(1), LATER + 60);
    handle(&mut m, cleaned(1), LATER + 60);
    handle(&mut m, AppEvent::Inserted { session: 1 }, LATER + 70);
    assert_eq!(
        handle(&mut m, AppEvent::PasteLast, LATER + 80),
        vec![Effect::PasteLast]
    );
}

#[test]
fn a_hold_released_after_249_ms_is_a_discarded_tap() {
    let mut m = machine();
    hold(&mut m);
    let effects = handle(&mut m, AppEvent::HoldUp, 249);
    assert_eq!(m.state(), State::Idle);
    assert_eq!(
        effects,
        vec![Effect::StopCapture {
            session: 1,
            keep: false
        }]
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Transcribe { .. }))
    );
}

#[test]
fn a_hold_released_at_250_ms_or_more_transcribes() {
    for held in [250, 251, 4_000] {
        let mut m = machine();
        hold(&mut m);
        let effects = handle(&mut m, AppEvent::HoldUp, held);
        assert_eq!(m.state(), State::Transcribing, "held {held} ms");
        assert_eq!(effects, stop_effects());
    }
}

#[test]
fn hold_down_repeats_do_not_restart_or_stop_a_hold() {
    let mut m = machine();
    hold(&mut m);
    for at in [30, 60, 90, 400] {
        assert!(handle(&mut m, AppEvent::HoldDown, at).is_empty());
    }
    assert_eq!(m.state(), State::Listening);
    assert_eq!(m.session(), 1);
}

fn tap(m: &mut DictationMachine, down: u64, up: u64) {
    handle(m, AppEvent::HoldDown, down);
    handle(m, AppEvent::HoldUp, up);
}

#[test]
fn two_taps_within_half_a_second_start_hands_free() {
    let mut m = machine();
    tap(&mut m, 0, 100);
    assert_eq!(m.state(), State::Idle);
    let effects = handle(&mut m, AppEvent::HoldDown, 250);
    assert_eq!(m.state(), State::Listening);
    assert_eq!(m.mode(), Mode::HandsFree);
    assert_eq!(m.session(), 2);
    assert_eq!(
        effects,
        vec![Effect::StartCapture { session: 2 }, Effect::Cue(Cue::Start)]
    );
}

#[test]
fn the_release_of_the_second_tap_does_nothing() {
    let mut m = machine();
    tap(&mut m, 0, 100);
    handle(&mut m, AppEvent::HoldDown, 250);
    assert!(handle(&mut m, AppEvent::HoldUp, 350).is_empty());
    assert!(handle(&mut m, AppEvent::HoldUp, 5_000).is_empty());
    assert_eq!(m.state(), State::Listening);
    assert_eq!(m.mode(), Mode::HandsFree);
}

#[test]
fn the_second_press_counts_from_the_first_press_not_the_release() {
    let mut at_limit = machine();
    tap(&mut at_limit, 0, 100);
    handle(&mut at_limit, AppEvent::HoldDown, 500);
    assert_eq!(at_limit.mode(), Mode::HandsFree);

    let mut too_late = machine();
    tap(&mut too_late, 0, 100);
    handle(&mut too_late, AppEvent::HoldDown, 501);
    assert_eq!(too_late.state(), State::Listening);
    assert_eq!(too_late.mode(), Mode::Hold);
}

#[test]
fn a_second_press_after_a_real_hold_is_not_a_double_tap() {
    let mut m = machine();
    tap(&mut m, 0, 300);
    assert_eq!(m.state(), State::Transcribing);
    handle(&mut m, AppEvent::Esc, 310);
    handle(&mut m, AppEvent::Tick, 900);
    assert_eq!(m.state(), State::Idle);
    handle(&mut m, AppEvent::HoldDown, 950);
    assert_eq!(m.mode(), Mode::Hold);
}

fn hold_off() -> DictationMachine {
    DictationMachine::new(Config {
        hold_to_talk: false,
        ..Config::default()
    })
}

#[test]
fn with_hold_to_talk_off_a_hold_records_nothing() {
    let mut m = hold_off();
    assert!(handle(&mut m, AppEvent::HoldDown, 0).is_empty());
    assert!(handle(&mut m, AppEvent::HoldUp, 2_000).is_empty());
    assert_eq!(m.state(), State::Idle);
    assert_eq!(m.session(), 0);
}

#[test]
fn with_hold_to_talk_off_a_double_tap_still_starts_and_a_press_stops_hands_free() {
    let mut m = hold_off();
    tap(&mut m, 0, 100);
    assert_eq!(m.state(), State::Idle);
    let effects = handle(&mut m, AppEvent::HoldDown, 250);
    assert_eq!(m.mode(), Mode::HandsFree);
    assert_eq!(
        effects,
        vec![Effect::StartCapture { session: 1 }, Effect::Cue(Cue::Start)]
    );
    handle(&mut m, AppEvent::HoldUp, 300);
    handle(&mut m, AppEvent::HoldDown, 3_000);
    assert_eq!(m.state(), State::Transcribing);
}

#[test]
fn with_hold_to_talk_off_a_long_press_is_not_half_of_a_double_tap() {
    let mut m = hold_off();
    tap(&mut m, 0, 300);
    handle(&mut m, AppEvent::HoldDown, 400);
    assert_eq!(m.state(), State::Idle);
}

#[test]
fn with_the_built_in_hands_free_off_a_double_tap_is_two_holds() {
    let mut m = DictationMachine::new(Config {
        double_tap: false,
        ..Config::default()
    });
    tap(&mut m, 0, 100);
    handle(&mut m, AppEvent::HoldDown, 250);
    assert_eq!(m.mode(), Mode::Hold);
}

#[test]
fn a_double_tap_is_used_up_and_a_third_tap_starts_fresh() {
    let mut m = machine();
    tap(&mut m, 0, 100);
    handle(&mut m, AppEvent::HoldDown, 250);
    handle(&mut m, AppEvent::HoldUp, 300);
    handle(&mut m, AppEvent::HoldDown, 400);
    assert_eq!(m.state(), State::Transcribing);
    handle(&mut m, AppEvent::HoldUp, 450);
    handle(&mut m, AppEvent::Esc, 460);
    handle(&mut m, AppEvent::Tick, 1_000);
    handle(&mut m, AppEvent::HoldDown, 1_100);
    assert_eq!(m.mode(), Mode::Hold);
}

#[test]
fn in_hands_free_the_next_press_goes_to_transcribing() {
    let mut m = machine();
    tap(&mut m, 0, 100);
    handle(&mut m, AppEvent::HoldDown, 250);
    handle(&mut m, AppEvent::HoldUp, 300);
    let effects = handle(&mut m, AppEvent::HoldDown, 3_000);
    assert_eq!(m.state(), State::Transcribing);
    assert_eq!(
        effects,
        vec![
            Effect::StopCapture {
                session: 2,
                keep: true
            },
            Effect::Cue(Cue::Stop),
            Effect::Transcribe { session: 2 }
        ]
    );
    assert!(handle(&mut m, AppEvent::HoldUp, 3_050).is_empty());
}

#[test]
fn hold_plus_space_turns_a_hold_into_hands_free() {
    let mut m = machine();
    hold(&mut m);
    handle(&mut m, AppEvent::HandsFreeToggle, 400);
    assert_eq!(m.mode(), Mode::HandsFree);
    assert!(handle(&mut m, AppEvent::HoldUp, 600).is_empty());
    assert_eq!(m.state(), State::Listening);
    handle(&mut m, AppEvent::HandsFreeToggle, 2_000);
    assert_eq!(m.state(), State::Transcribing);
}

#[test]
fn home_sessions_copy_and_never_paste() {
    let mut m = machine();
    handle(&mut m, AppEvent::HomeToggle, 0);
    assert_eq!(m.mode(), Mode::Home);
    assert!(handle(&mut m, AppEvent::HoldUp, 400).is_empty());
    handle(&mut m, AppEvent::HomeToggle, 2_000);
    handle(&mut m, transcribed(1), 2_100);
    let effects = handle(&mut m, cleaned(1), 2_200);
    assert_eq!(
        effects,
        vec![Effect::Insert {
            session: 1,
            text: CLEAN.into(),
            delivery: Delivery::Copy
        }]
    );
}

#[test]
fn the_flow_bar_starts_hands_free() {
    let mut m = machine();
    handle(&mut m, AppEvent::FlowBarClick, 0);
    assert_eq!(m.mode(), Mode::HandsFree);
}

#[test]
fn the_default_maximum_is_six_minutes() {
    assert_eq!(Config::default().max_minutes, 6);
    assert_eq!(Config::default().max_ms(), 360_000);
}

#[test]
fn hands_free_warns_at_five_minutes_then_stops_at_six_keeping_the_audio() {
    let mut m = machine();
    handle(&mut m, AppEvent::HandsFreeToggle, 10_000);
    assert!(handle(&mut m, AppEvent::Tick, 10_000 + 299_999).is_empty());
    assert_eq!(
        handle(&mut m, AppEvent::Tick, 10_000 + 300_000),
        vec![Effect::MaxDurationWarning { seconds_left: 60 }]
    );
    assert!(handle(&mut m, AppEvent::Tick, 10_000 + 330_000).is_empty());
    assert_eq!(m.state(), State::Listening);
    let effects = handle(&mut m, AppEvent::Tick, 10_000 + 360_000);
    assert_eq!(m.state(), State::Transcribing);
    assert_eq!(effects, stop_effects());
    assert!(!effects.contains(&Effect::StopCapture {
        session: 1,
        keep: false
    }));
}

#[test]
fn a_late_tick_stops_without_a_warning() {
    let mut m = machine();
    handle(&mut m, AppEvent::HandsFreeToggle, 0);
    let effects = handle(&mut m, AppEvent::Tick, 400_000);
    assert_eq!(effects, stop_effects());
}

#[test]
fn a_hold_also_stops_at_the_maximum() {
    let mut m = machine();
    hold(&mut m);
    handle(&mut m, AppEvent::Tick, 300_000);
    handle(&mut m, AppEvent::Tick, 360_000);
    assert_eq!(m.state(), State::Transcribing);
    assert!(handle(&mut m, AppEvent::HoldUp, 361_000).is_empty());
}

#[test]
fn the_warning_follows_the_configured_maximum() {
    let mut m = DictationMachine::new(Config {
        max_minutes: 2,
        ..Config::default()
    });
    handle(&mut m, AppEvent::HandsFreeToggle, 0);
    assert!(handle(&mut m, AppEvent::Tick, 59_999).is_empty());
    assert_eq!(
        handle(&mut m, AppEvent::Tick, 60_000),
        vec![Effect::MaxDurationWarning { seconds_left: 60 }]
    );
    assert_eq!(handle(&mut m, AppEvent::Tick, 120_000), stop_effects());
}

#[test]
fn escape_in_listening_discards_and_a_later_release_starts_nothing() {
    let mut m = machine();
    hold(&mut m);
    let effects = handle(&mut m, AppEvent::Esc, 600);
    assert_eq!(m.state(), State::Cancelled);
    assert!(effects.contains(&Effect::StopCapture {
        session: 1,
        keep: false
    }));
    assert!(handle(&mut m, AppEvent::HoldUp, 900).is_empty());
    assert_eq!(m.state(), State::Cancelled);
}

#[test]
fn escape_in_cleaning_stops_the_llm_saves_raw_text_and_never_inserts() {
    let mut m = reach(State::Cleaning);
    let effects = handle(&mut m, AppEvent::Esc, LATER);
    assert_eq!(m.state(), State::Cancelled);
    assert!(effects.contains(&Effect::StopLlm { session: 1 }));
    assert!(effects.contains(&Effect::SaveRow {
        session: 1,
        status: RowStatus::Cancelled,
        text: Some(RAW.into()),
        code: None
    }));
    assert!(!effects.iter().any(|e| matches!(e, Effect::Insert { .. })));
    // The AI answer that arrives afterwards is dropped.
    assert!(handle(&mut m, cleaned(1), LATER + 5).is_empty());
    assert_eq!(m.state(), State::Cancelled);
}

#[test]
fn escape_in_inserting_and_the_end_states_changes_nothing() {
    for state in [
        State::Inserting,
        State::Done,
        State::Cancelled,
        State::Failed,
    ] {
        let mut m = reach(state);
        assert!(handle(&mut m, AppEvent::Esc, LATER).is_empty(), "{state:?}");
        assert_eq!(m.state(), state);
    }
}

#[test]
fn only_running_sessions_take_escape() {
    let taken: Vec<State> = State::ALL
        .into_iter()
        .filter(|state| state.takes_escape())
        .collect();
    assert_eq!(
        taken,
        [
            State::Listening,
            State::Transcribing,
            State::Cleaning,
            State::Inserting
        ]
    );
}

#[test]
fn end_states_return_to_idle_on_their_timers() {
    let config = Config::default();
    for (state, wait) in [
        (State::Done, config.done_ms),
        (State::Cancelled, config.cancelled_ms),
        (State::Failed, config.failed_ms),
    ] {
        let mut m = reach(state);
        assert!(handle(&mut m, AppEvent::Tick, LATER + wait - 1).is_empty());
        assert_eq!(m.state(), state, "{state:?} one ms early");
        handle(&mut m, AppEvent::Tick, LATER + wait);
        assert_eq!(m.state(), State::Idle, "{state:?}");
    }
}

#[test]
fn a_new_session_can_start_after_the_end_state_clears() {
    let mut m = reach(State::Done);
    handle(&mut m, AppEvent::Tick, LATER + 2_000);
    let effects = handle(&mut m, AppEvent::HandsFreeToggle, LATER + 3_000);
    assert_eq!(
        effects,
        vec![Effect::StartCapture { session: 2 }, Effect::Cue(Cue::Start)]
    );
}

#[test]
fn an_answer_from_an_older_session_is_ignored() {
    let mut m = reach(State::Cancelled);
    handle(&mut m, AppEvent::Tick, LATER + 600);
    handle(&mut m, AppEvent::HoldDown, 5_000);
    handle(&mut m, AppEvent::HoldUp, 6_000);
    assert_eq!(m.session(), 2);
    assert!(handle(&mut m, transcribed(1), 6_100).is_empty());
    assert_eq!(m.state(), State::Transcribing);
    assert_eq!(
        handle(&mut m, transcribed(2), 6_200).len(),
        1,
        "the current session is accepted"
    );
    assert_eq!(m.state(), State::Cleaning);
}

#[test]
fn a_transcript_with_no_words_fails_with_no_speech_and_saves_a_failed_row() {
    for text in ["", "  ", "[BLANK_AUDIO]", "(silence) [MUSIC]"] {
        let mut m = reach(State::Transcribing);
        let effects = handle(
            &mut m,
            AppEvent::Transcribed {
                session: 1,
                text: text.into(),
            },
            LATER,
        );
        assert_eq!(m.state(), State::Failed, "{text:?}");
        assert_eq!(
            effects,
            vec![failed_row(None, ENGINE_NO_SPEECH), notify(ENGINE_NO_SPEECH)]
        );
    }
}

#[test]
fn cleanup_that_leaves_nothing_fails_with_no_speech() {
    let mut m = reach(State::Cleaning);
    let effects = handle(
        &mut m,
        AppEvent::Cleaned {
            session: 1,
            text: " ".into(),
        },
        LATER,
    );
    assert_eq!(m.state(), State::Failed);
    assert!(!effects.iter().any(|e| matches!(e, Effect::Insert { .. })));
}

#[test]
fn the_raw_text_passed_to_cleanup_has_its_blank_markers_removed() {
    let mut m = reach(State::Transcribing);
    let effects = handle(
        &mut m,
        AppEvent::Transcribed {
            session: 1,
            text: "[BLANK_AUDIO] hello world".into(),
        },
        LATER,
    );
    assert_eq!(
        effects,
        vec![Effect::Clean {
            session: 1,
            raw: "hello world".into()
        }]
    );
}
// Command Mode.

fn command_session() -> DictationMachine {
    let mut m = machine();
    handle(&mut m, AppEvent::CommandDown, 0);
    m
}

#[test]
fn the_command_keys_start_a_command_session_that_stops_when_they_come_up() {
    let mut m = command_session();
    assert_eq!(m.mode(), Mode::Command);
    assert_eq!(m.mode().key(), "command");
    assert_eq!(handle(&mut m, AppEvent::CommandUp, LATER), stop_effects());
    assert_eq!(m.state(), State::Transcribing);
}

#[test]
fn a_hold_that_already_runs_becomes_the_command_session() {
    let mut m = machine();
    hold(&mut m);
    assert_eq!(handle(&mut m, AppEvent::CommandDown, 60), vec![]);
    assert_eq!(m.mode(), Mode::Command);
    assert_eq!(m.session(), 1, "the recording goes on, it does not restart");
    // The hold key coming up before the command keys changes nothing.
    assert_eq!(handle(&mut m, AppEvent::HoldUp, 700), vec![]);
    assert_eq!(handle(&mut m, AppEvent::CommandUp, 710), stop_effects());
}

#[test]
fn a_hands_free_or_home_session_ignores_the_command_keys() {
    for start in [AppEvent::HandsFreeToggle, AppEvent::HomeToggle] {
        let mut m = machine();
        handle(&mut m, start, 0);
        let mode = m.mode();
        assert!(handle(&mut m, AppEvent::CommandDown, 100).is_empty());
        assert!(handle(&mut m, AppEvent::CommandUp, 200).is_empty());
        assert_eq!(m.mode(), mode);
        assert_eq!(m.state(), State::Listening);
    }
}

#[test]
fn a_command_tap_is_discarded_and_is_not_half_of_a_double_tap() {
    let mut m = command_session();
    assert_eq!(
        handle(&mut m, AppEvent::CommandUp, 249),
        vec![Effect::StopCapture {
            session: 1,
            keep: false
        }]
    );
    assert_eq!(m.state(), State::Idle);
    let effects = handle(&mut m, AppEvent::HoldDown, 300);
    assert_eq!(
        m.mode(),
        Mode::Hold,
        "a tap of Command Mode arms no hands-free"
    );
    assert_eq!(
        effects,
        vec![Effect::StartCapture { session: 2 }, Effect::Cue(Cue::Start),]
    );
}

#[test]
fn the_instruction_goes_to_the_model_as_an_edit_and_never_to_cleanup() {
    let mut m = command_session();
    handle(&mut m, AppEvent::CommandUp, LATER);
    let effects = handle(&mut m, transcribed(1), LATER);
    assert_eq!(
        effects,
        vec![Effect::Edit {
            session: 1,
            instruction: RAW.into()
        }]
    );
    assert_eq!(m.state(), State::Cleaning);
}

#[test]
fn the_edited_text_is_pasted_and_saved_like_a_dictation() {
    let mut m = command_session();
    handle(&mut m, AppEvent::CommandUp, LATER);
    handle(&mut m, transcribed(1), LATER);
    let effects = handle(&mut m, cleaned(1), LATER);
    assert_eq!(
        effects,
        vec![Effect::Insert {
            session: 1,
            text: CLEAN.into(),
            delivery: Delivery::Paste
        }]
    );
    let done = handle(&mut m, AppEvent::Inserted { session: 1 }, LATER);
    assert_eq!(m.state(), State::Done);
    assert!(done.contains(&Effect::SaveRow {
        session: 1,
        status: RowStatus::Done,
        text: Some(CLEAN.into()),
        code: None
    }));
}

#[test]
fn a_failed_edit_pastes_nothing_and_a_late_answer_changes_nothing() {
    let mut m = command_session();
    handle(&mut m, AppEvent::CommandUp, LATER);
    handle(&mut m, transcribed(1), LATER);
    let failed = AppEvent::EditFailed {
        session: 1,
        code: LLM_TIMEOUT,
    };
    let effects = handle(&mut m, failed.clone(), LATER);
    assert_eq!(m.state(), State::Failed);
    assert!(!effects.iter().any(|e| matches!(e, Effect::Insert { .. })));
    assert!(handle(&mut m, cleaned(1), LATER).is_empty());
    assert!(handle(&mut m, failed, LATER).is_empty());
}

#[test]
fn an_edit_failure_from_an_older_session_is_ignored() {
    let mut m = command_session();
    handle(&mut m, AppEvent::CommandUp, LATER);
    handle(&mut m, transcribed(1), LATER);
    let stale = AppEvent::EditFailed {
        session: 7,
        code: LLM_TIMEOUT,
    };
    assert!(handle(&mut m, stale, LATER).is_empty());
    assert_eq!(m.state(), State::Cleaning);
}

#[test]
fn esc_while_the_model_edits_stops_it_and_pastes_nothing() {
    let mut m = command_session();
    handle(&mut m, AppEvent::CommandUp, LATER);
    handle(&mut m, transcribed(1), LATER);
    let effects = handle(&mut m, AppEvent::Esc, LATER);
    assert_eq!(m.state(), State::Cancelled);
    assert!(effects.contains(&Effect::StopLlm { session: 1 }));
    assert!(handle(&mut m, cleaned(1), LATER).is_empty());
}
