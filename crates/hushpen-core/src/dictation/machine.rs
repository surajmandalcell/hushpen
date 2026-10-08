use super::{
    AppEvent, Config, Cue, DOUBLE_TAP_MS, Delivery, Effect, MIN_HOLD_MS, Mode, RowStatus, State,
    WARN_BEFORE_MAX_MS,
};
use crate::error::ENGINE_NO_SPEECH;
use crate::transcript::strip_blank_markers;

pub struct DictationMachine {
    config: Config,
    state: State,
    mode: Mode,
    session: u64,
    started_at: u64,
    state_since: u64,
    /// The press time of the last tap, until a second press uses it up or it goes stale.
    tap_at: Option<u64>,
    warned: bool,
    raw: String,
    text: String,
}

impl DictationMachine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: State::Idle,
            mode: Mode::Hold,
            session: 0,
            started_at: 0,
            state_since: 0,
            tap_at: None,
            warned: false,
            raw: String::new(),
            text: String::new(),
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The id of the current or last session; 0 before the first one.
    pub fn session(&self) -> u64 {
        self.session
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn handle(&mut self, event: AppEvent, now: u64) -> Vec<Effect> {
        match event {
            AppEvent::HoldDown => match self.state {
                State::Idle => {
                    let double = self
                        .tap_at
                        .is_some_and(|tap| now.saturating_sub(tap) <= DOUBLE_TAP_MS);
                    self.start(if double { Mode::HandsFree } else { Mode::Hold }, now)
                }
                State::Listening if self.mode != Mode::Hold => self.stop_listening(now),
                _ => Vec::new(),
            },
            AppEvent::HoldUp => {
                if self.state != State::Listening || self.mode != Mode::Hold {
                    return Vec::new();
                }
                if now.saturating_sub(self.started_at) < MIN_HOLD_MS {
                    self.tap_at = Some(self.started_at);
                    self.enter(State::Idle, now);
                    return vec![Effect::StopCapture { keep: false }];
                }
                self.stop_listening(now)
            }
            AppEvent::HandsFreeToggle => match self.state {
                State::Idle => self.start(Mode::HandsFree, now),
                State::Listening if self.mode == Mode::Hold => {
                    self.mode = Mode::HandsFree;
                    Vec::new()
                }
                State::Listening => self.stop_listening(now),
                _ => Vec::new(),
            },
            AppEvent::HomeToggle => match self.state {
                State::Idle => self.start(Mode::Home, now),
                State::Listening => self.stop_listening(now),
                _ => Vec::new(),
            },
            AppEvent::FlowBarClick => match self.state {
                State::Idle => self.start(Mode::HandsFree, now),
                State::Listening => self.stop_listening(now),
                _ => Vec::new(),
            },
            AppEvent::Esc => self.escape(now),
            AppEvent::PasteLast => match self.state {
                State::Idle | State::Done | State::Cancelled | State::Failed => {
                    vec![Effect::PasteLast]
                }
                _ => Vec::new(),
            },
            AppEvent::Tick => self.tick(now),
            AppEvent::CaptureError { code } => {
                if self.state != State::Listening {
                    return Vec::new();
                }
                self.enter(State::Failed, now);
                vec![
                    Effect::StopCapture { keep: true },
                    failed_row(None, code),
                    Effect::Notify { code },
                ]
            }
            AppEvent::Transcribed { session, text } => {
                if !self.expects(State::Transcribing, session) {
                    return Vec::new();
                }
                let raw = strip_blank_markers(&text);
                if raw.is_empty() {
                    return self.no_speech(now);
                }
                self.raw = raw.clone();
                self.enter(State::Cleaning, now);
                vec![Effect::Clean { session, raw }]
            }
            AppEvent::TranscribeFailed { session, code } => {
                if !self.expects(State::Transcribing, session) {
                    return Vec::new();
                }
                self.enter(State::Failed, now);
                vec![failed_row(None, code), Effect::Notify { code }]
            }
            AppEvent::Cleaned { session, text } => {
                if !self.expects(State::Cleaning, session) {
                    return Vec::new();
                }
                let text = text.trim().to_owned();
                if text.is_empty() {
                    return self.no_speech(now);
                }
                self.text = text.clone();
                self.enter(State::Inserting, now);
                let delivery = if self.mode == Mode::Home {
                    Delivery::Copy
                } else {
                    Delivery::Paste
                };
                vec![Effect::Insert {
                    session,
                    text,
                    delivery,
                }]
            }
            AppEvent::Inserted { session } => {
                if !self.expects(State::Inserting, session) {
                    return Vec::new();
                }
                self.enter(State::Done, now);
                vec![
                    Effect::SaveRow {
                        status: RowStatus::Done,
                        text: Some(self.text.clone()),
                        code: None,
                    },
                    Effect::UpdatePasteLast {
                        text: self.text.clone(),
                    },
                ]
            }
            AppEvent::InsertFailed { session, code } => {
                if !self.expects(State::Inserting, session) {
                    return Vec::new();
                }
                self.enter(State::Failed, now);
                vec![
                    failed_row(Some(self.text.clone()), code),
                    Effect::Notify { code },
                ]
            }
        }
    }

    fn expects(&self, state: State, session: u64) -> bool {
        self.state == state && self.session == session
    }

    fn enter(&mut self, state: State, now: u64) {
        self.state = state;
        self.state_since = now;
    }

    fn start(&mut self, mode: Mode, now: u64) -> Vec<Effect> {
        self.session += 1;
        self.mode = mode;
        self.started_at = now;
        self.tap_at = None;
        self.warned = false;
        self.raw.clear();
        self.text.clear();
        self.enter(State::Listening, now);
        vec![
            Effect::StartCapture {
                session: self.session,
            },
            Effect::Cue(Cue::Start),
        ]
    }

    fn stop_listening(&mut self, now: u64) -> Vec<Effect> {
        self.enter(State::Transcribing, now);
        vec![
            Effect::StopCapture { keep: true },
            Effect::Cue(Cue::Stop),
            Effect::Transcribe {
                session: self.session,
            },
        ]
    }

    fn no_speech(&mut self, now: u64) -> Vec<Effect> {
        self.enter(State::Failed, now);
        vec![
            Effect::DiscardAudio,
            Effect::Notify {
                code: ENGINE_NO_SPEECH,
            },
        ]
    }

    fn escape(&mut self, now: u64) -> Vec<Effect> {
        let effects = match self.state {
            State::Listening => vec![
                Effect::StopCapture { keep: false },
                Effect::Cue(Cue::Cancel),
            ],
            State::Transcribing => vec![
                Effect::CancelTranscribe,
                Effect::Cue(Cue::Cancel),
                cancelled_row(None),
            ],
            State::Cleaning => vec![
                Effect::StopLlm,
                Effect::Cue(Cue::Cancel),
                cancelled_row(Some(self.raw.clone())),
            ],
            _ => return Vec::new(),
        };
        self.enter(State::Cancelled, now);
        effects
    }

    fn tick(&mut self, now: u64) -> Vec<Effect> {
        let since = now.saturating_sub(self.state_since);
        match self.state {
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
            State::Done if since >= self.config.done_ms => self.back_to_idle(now),
            State::Cancelled if since >= self.config.cancelled_ms => self.back_to_idle(now),
            State::Failed if since >= self.config.failed_ms => self.back_to_idle(now),
            _ => Vec::new(),
        }
    }

    fn back_to_idle(&mut self, now: u64) -> Vec<Effect> {
        self.enter(State::Idle, now);
        Vec::new()
    }
}

fn failed_row(text: Option<String>, code: &'static str) -> Effect {
    Effect::SaveRow {
        status: RowStatus::Failed,
        text,
        code: Some(code),
    }
}

fn cancelled_row(text: Option<String>) -> Effect {
    Effect::SaveRow {
        status: RowStatus::Cancelled,
        text,
        code: None,
    }
}
