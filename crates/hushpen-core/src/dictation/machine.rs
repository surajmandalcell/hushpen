use super::{
    AppEvent, Config, Cue, DOUBLE_TAP_MS, Delivery, Effect, MIN_HOLD_MS, Mode, RowStatus, State,
    WARN_BEFORE_MAX_MS,
};
use crate::error::ENGINE_NO_SPEECH;
use crate::transcript::strip_blank_markers;

/// One session, from its start to its history row.
struct Run {
    session: u64,
    state: State,
    mode: Mode,
    raw: String,
    text: String,
    /// The `Insert` effect went out. A text waits until every older session has finished.
    sent: bool,
}

impl Run {
    fn new(session: u64, state: State, mode: Mode) -> Self {
        Self {
            session,
            state,
            mode,
            raw: String::new(),
            text: String::new(),
            sent: false,
        }
    }

    /// Past the microphone, but not yet at its row.
    fn finishing(&self) -> bool {
        matches!(
            self.state,
            State::Transcribing | State::Cleaning | State::Inserting
        )
    }
}

#[derive(Clone, Copy)]
enum Slot {
    Front,
    Behind(usize),
}

pub struct DictationMachine {
    config: Config,
    /// The newest session: the one the flow bar shows and Esc cancels.
    run: Run,
    /// Older sessions that still transcribe, clean, or insert, oldest first. A session moves
    /// here when a new one starts on top of it, and leaves when it saves its row.
    behind: Vec<Run>,
    last_session: u64,
    started_at: u64,
    state_since: u64,
    /// The press time of the last tap, until a second press uses it up or it goes stale.
    tap_at: Option<u64>,
    /// When the hold key went down while it does not record, to tell a tap from a hold.
    pressed_at: Option<u64>,
    warned: bool,
}

impl DictationMachine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            run: Run::new(0, State::Idle, Mode::Hold),
            behind: Vec::new(),
            last_session: 0,
            started_at: 0,
            state_since: 0,
            tap_at: None,
            pressed_at: None,
            warned: false,
        }
    }

    /// The state of the newest session.
    pub fn state(&self) -> State {
        self.run.state
    }

    pub fn mode(&self) -> Mode {
        self.run.mode
    }

    /// The id of the newest session, or of the last one; 0 before the first one.
    pub fn session(&self) -> u64 {
        self.run.session
    }

    /// How many older sessions still finish behind the newest one.
    pub fn behind(&self) -> usize {
        self.behind.len()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn handle(&mut self, event: AppEvent, now: u64) -> Vec<Effect> {
        match event {
            AppEvent::HoldDown => match self.run.state {
                State::Listening if matches!(self.run.mode, Mode::Hold | Mode::Command) => {
                    Vec::new()
                }
                State::Listening => self.stop_listening(now),
                _ => {
                    let double = self.config.double_tap
                        && self
                            .tap_at
                            .is_some_and(|tap| now.saturating_sub(tap) <= DOUBLE_TAP_MS);
                    if double {
                        self.start(Mode::HandsFree, now)
                    } else if self.config.hold_to_talk {
                        self.start(Mode::Hold, now)
                    } else {
                        self.tap_at = None;
                        self.pressed_at = Some(now);
                        Vec::new()
                    }
                }
            },
            AppEvent::HoldArm => Vec::new(),
            AppEvent::HoldUp => {
                if let Some(pressed) = self.pressed_at.take()
                    && self.run.state != State::Listening
                    && now.saturating_sub(pressed) < MIN_HOLD_MS
                {
                    self.tap_at = Some(pressed);
                }
                if self.run.state != State::Listening || self.run.mode != Mode::Hold {
                    return Vec::new();
                }
                if now.saturating_sub(self.started_at) < MIN_HOLD_MS {
                    self.tap_at = Some(self.started_at);
                    return self.discard(now);
                }
                self.stop_listening(now)
            }
            AppEvent::CommandDown => match (self.run.state, self.run.mode) {
                // The command keys contain the hold key, so the hold has already started the
                // recording; it keeps running as the instruction.
                (State::Listening, Mode::Hold) => {
                    self.run.mode = Mode::Command;
                    Vec::new()
                }
                (State::Listening, _) => Vec::new(),
                _ => self.start(Mode::Command, now),
            },
            AppEvent::CommandUp => {
                if self.run.state != State::Listening || self.run.mode != Mode::Command {
                    return Vec::new();
                }
                if now.saturating_sub(self.started_at) < MIN_HOLD_MS {
                    return self.discard(now);
                }
                self.stop_listening(now)
            }
            AppEvent::HandsFreeToggle => match (self.run.state, self.run.mode) {
                (State::Listening, Mode::Hold) => {
                    self.run.mode = Mode::HandsFree;
                    Vec::new()
                }
                (State::Listening, Mode::Command) => Vec::new(),
                (State::Listening, _) => self.stop_listening(now),
                _ => self.start(Mode::HandsFree, now),
            },
            AppEvent::HomeToggle => match self.run.state {
                State::Listening => self.stop_listening(now),
                _ => self.start(Mode::Home, now),
            },
            AppEvent::FlowBarClick => match self.run.state {
                State::Listening => self.stop_listening(now),
                _ => self.start(Mode::HandsFree, now),
            },
            AppEvent::Esc => self.escape(now),
            AppEvent::PasteLast => match self.run.state {
                State::Idle | State::Done | State::Cancelled | State::Failed
                    if self.behind.is_empty() =>
                {
                    vec![Effect::PasteLast]
                }
                _ => Vec::new(),
            },
            AppEvent::ReadAloud => Vec::new(),
            AppEvent::Tick => self.tick(now),
            AppEvent::CaptureError { code } => {
                if self.run.state != State::Listening {
                    return Vec::new();
                }
                let session = self.run.session;
                let effects = vec![
                    Effect::StopCapture {
                        session,
                        keep: true,
                    },
                    failed_row(session, None, code),
                    Effect::Notify { session, code },
                ];
                self.finish(Slot::Front, State::Failed, effects, now)
            }
            AppEvent::Transcribed { session, text } => {
                let Some(slot) = self.find(session, State::Transcribing) else {
                    return Vec::new();
                };
                let raw = strip_blank_markers(&text);
                if raw.is_empty() {
                    return self.no_speech(slot, now);
                }
                self.set_state(slot, State::Cleaning, now);
                let run = self.slot_mut(slot);
                run.raw = raw.clone();
                if run.mode == Mode::Command {
                    return vec![Effect::Edit {
                        session,
                        instruction: raw,
                    }];
                }
                vec![Effect::Clean { session, raw }]
            }
            AppEvent::EditFailed { session, code } => {
                let Some(slot) = self.find(session, State::Cleaning) else {
                    return Vec::new();
                };
                let effects = vec![
                    failed_row(session, None, code),
                    Effect::Notify { session, code },
                ];
                self.finish(slot, State::Failed, effects, now)
            }
            AppEvent::TranscribeFailed { session, code } => {
                let Some(slot) = self.find(session, State::Transcribing) else {
                    return Vec::new();
                };
                let effects = vec![
                    failed_row(session, None, code),
                    Effect::Notify { session, code },
                ];
                self.finish(slot, State::Failed, effects, now)
            }
            AppEvent::Cleaned { session, text } => {
                let Some(slot) = self.find(session, State::Cleaning) else {
                    return Vec::new();
                };
                let text = text.trim().to_owned();
                if text.is_empty() {
                    return self.no_speech(slot, now);
                }
                self.set_state(slot, State::Inserting, now);
                self.slot_mut(slot).text = text;
                self.release_insert().into_iter().collect()
            }
            AppEvent::Inserted { session } => {
                let Some(slot) = self.find_sent(session) else {
                    return Vec::new();
                };
                let text = self.slot(slot).text.clone();
                let effects = vec![
                    Effect::SaveRow {
                        session,
                        status: RowStatus::Done,
                        text: Some(text.clone()),
                        code: None,
                    },
                    Effect::UpdatePasteLast { text },
                ];
                self.finish(slot, State::Done, effects, now)
            }
            AppEvent::InsertFailed { session, code } => {
                let Some(slot) = self.find_sent(session) else {
                    return Vec::new();
                };
                let text = self.slot(slot).text.clone();
                let effects = vec![
                    failed_row(session, Some(text), code),
                    Effect::Notify { session, code },
                ];
                self.finish(slot, State::Failed, effects, now)
            }
        }
    }

    /// The session `session` when it is in `state`, so a late answer from an ended run finds
    /// nothing.
    fn find(&self, session: u64, state: State) -> Option<Slot> {
        if self.run.session == session && self.run.state == state {
            return Some(Slot::Front);
        }
        self.behind
            .iter()
            .position(|run| run.session == session && run.state == state)
            .map(Slot::Behind)
    }

    fn find_sent(&self, session: u64) -> Option<Slot> {
        self.find(session, State::Inserting)
            .filter(|slot| self.slot(*slot).sent)
    }

    fn slot(&self, slot: Slot) -> &Run {
        match slot {
            Slot::Front => &self.run,
            Slot::Behind(index) => &self.behind[index],
        }
    }

    fn slot_mut(&mut self, slot: Slot) -> &mut Run {
        match slot {
            Slot::Front => &mut self.run,
            Slot::Behind(index) => &mut self.behind[index],
        }
    }

    fn set_state(&mut self, slot: Slot, state: State, now: u64) {
        match slot {
            Slot::Front => self.enter(state, now),
            Slot::Behind(index) => self.behind[index].state = state,
        }
    }

    fn enter(&mut self, state: State, now: u64) {
        self.run.state = state;
        self.state_since = now;
    }

    /// Ends a session with its row: the newest one shows `end` on the flow bar, an older one
    /// just goes. Either way the next text in line may now go in.
    fn finish(
        &mut self,
        slot: Slot,
        end: State,
        mut effects: Vec<Effect>,
        now: u64,
    ) -> Vec<Effect> {
        match slot {
            Slot::Front => self.enter(end, now),
            Slot::Behind(index) => {
                self.behind.remove(index);
            }
        }
        effects.extend(self.release_insert());
        effects
    }

    /// The `Insert` of the oldest running session, once its text is ready. Only the oldest one
    /// may insert, so texts go in the order they were spoken and two pastes never overlap.
    fn release_insert(&mut self) -> Option<Effect> {
        let oldest = self.behind.first_mut().unwrap_or(&mut self.run);
        if oldest.state != State::Inserting || oldest.sent {
            return None;
        }
        oldest.sent = true;
        let delivery = if oldest.mode == Mode::Home {
            Delivery::Copy
        } else {
            Delivery::Paste
        };
        Some(Effect::Insert {
            session: oldest.session,
            text: oldest.text.clone(),
            delivery,
        })
    }

    /// Starts a new session at once. The session it replaces on the flow bar keeps going
    /// behind it when it is still on its way to the text.
    fn start(&mut self, mode: Mode, now: u64) -> Vec<Effect> {
        self.last_session += 1;
        let session = self.last_session;
        let previous = std::mem::replace(&mut self.run, Run::new(session, State::Listening, mode));
        if previous.finishing() {
            self.behind.push(previous);
        }
        self.started_at = now;
        self.state_since = now;
        self.tap_at = None;
        self.warned = false;
        vec![Effect::StartCapture { session }, Effect::Cue(Cue::Start)]
    }

    fn stop_listening(&mut self, now: u64) -> Vec<Effect> {
        let session = self.run.session;
        self.enter(State::Transcribing, now);
        vec![
            Effect::StopCapture {
                session,
                keep: true,
            },
            Effect::Cue(Cue::Stop),
            Effect::Transcribe { session },
        ]
    }

    /// A tap: the recording is thrown away and leaves no row.
    fn discard(&mut self, now: u64) -> Vec<Effect> {
        let session = self.run.session;
        self.close_front(now);
        vec![Effect::StopCapture {
            session,
            keep: false,
        }]
    }

    fn no_speech(&mut self, slot: Slot, now: u64) -> Vec<Effect> {
        let session = self.slot(slot).session;
        let effects = vec![
            failed_row(session, None, ENGINE_NO_SPEECH),
            Effect::Notify {
                session,
                code: ENGINE_NO_SPEECH,
            },
        ];
        self.finish(slot, State::Failed, effects, now)
    }

    /// Cancels the newest session only: an older one that finishes behind it is the text of
    /// an earlier dictation, and it still goes in.
    fn escape(&mut self, now: u64) -> Vec<Effect> {
        let session = self.run.session;
        let effects = match self.run.state {
            State::Listening => vec![
                Effect::StopCapture {
                    session,
                    keep: false,
                },
                Effect::Cue(Cue::Cancel),
            ],
            State::Transcribing => vec![
                Effect::CancelTranscribe { session },
                Effect::Cue(Cue::Cancel),
                cancelled_row(session, None),
            ],
            State::Cleaning => vec![
                Effect::StopLlm { session },
                Effect::Cue(Cue::Cancel),
                cancelled_row(session, Some(self.run.raw.clone())),
            ],
            _ => return Vec::new(),
        };
        self.finish(Slot::Front, State::Cancelled, effects, now)
    }

    fn tick(&mut self, now: u64) -> Vec<Effect> {
        let since = now.saturating_sub(self.state_since);
        match self.run.state {
            State::Listening => {
                let elapsed = now.saturating_sub(self.started_at);
                let max = self.config.max_ms();
                if elapsed >= max {
                    return self.stop_listening(now);
                }
                if !self.warned && elapsed >= max.saturating_sub(WARN_BEFORE_MAX_MS) {
                    self.warned = true;
                    return vec![Effect::MaxDurationWarning {
                        seconds_left: WARN_BEFORE_MAX_MS / 1_000,
                    }];
                }
                Vec::new()
            }
            State::Done if since >= self.config.done_ms => self.close_front(now),
            State::Cancelled if since >= self.config.cancelled_ms => self.close_front(now),
            State::Failed if since >= self.config.failed_ms => self.close_front(now),
            _ => Vec::new(),
        }
    }

    /// The newest session is over. The newest older session that still runs comes back to the
    /// flow bar, so it shows its progress and Esc can cancel it.
    fn close_front(&mut self, now: u64) -> Vec<Effect> {
        match self.behind.pop() {
            Some(run) => {
                self.run = run;
                self.state_since = now;
            }
            None => self.enter(State::Idle, now),
        }
        Vec::new()
    }
}

fn failed_row(session: u64, text: Option<String>, code: &'static str) -> Effect {
    Effect::SaveRow {
        session,
        status: RowStatus::Failed,
        text,
        code: Some(code),
    }
}

fn cancelled_row(session: u64, text: Option<String>) -> Effect {
    Effect::SaveRow {
        session,
        status: RowStatus::Cancelled,
        text,
        code: None,
    }
}
