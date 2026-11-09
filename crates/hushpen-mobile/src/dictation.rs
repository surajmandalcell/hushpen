use crate::{MobileError, lock};
use hushpen_core::dictation::{self, AppEvent, DictationMachine, Effect};
use hushpen_core::error;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum DictationEvent {
    HoldDown,
    HoldUp,
    Toggle,
    HomeToggle,
    Cancel,
    Recover,
    PasteLast,
    Tick,
    CaptureFailed,
    Transcribed { session: u64, text: String },
    TranscribeFailed { session: u64 },
    Cleaned { session: u64, text: String },
    Inserted { session: u64 },
    InsertFailed { session: u64 },
}

impl From<DictationEvent> for AppEvent {
    fn from(event: DictationEvent) -> Self {
        match event {
            DictationEvent::HoldDown => Self::HoldDown,
            DictationEvent::HoldUp => Self::HoldUp,
            DictationEvent::Toggle => Self::HandsFreeToggle,
            DictationEvent::HomeToggle => Self::HomeToggle,
            DictationEvent::Cancel => Self::Esc,
            DictationEvent::Recover => Self::Recover,
            DictationEvent::PasteLast => Self::PasteLast,
            DictationEvent::Tick => Self::Tick,
            DictationEvent::CaptureFailed => Self::CaptureError {
                code: error::CAPTURE_FAILED,
            },
            DictationEvent::Transcribed { session, text } => Self::Transcribed { session, text },
            DictationEvent::TranscribeFailed { session } => Self::TranscribeFailed {
                session,
                code: error::ENGINE_CRASHED,
            },
            DictationEvent::Cleaned { session, text } => Self::Cleaned { session, text },
            DictationEvent::Inserted { session } => Self::Inserted { session },
            DictationEvent::InsertFailed { session } => Self::InsertFailed {
                session,
                code: error::INSERT_NO_RECEIPT,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum DictationEffect {
    StartCapture {
        session: u64,
    },
    StopCapture {
        session: u64,
        keep: bool,
    },
    HoldCapture {
        session: u64,
    },
    DropAudio {
        session: u64,
    },
    Cue {
        name: String,
    },
    Transcribe {
        session: u64,
    },
    CancelTranscribe {
        session: u64,
    },
    Clean {
        session: u64,
        raw: String,
    },
    Edit {
        session: u64,
        instruction: String,
    },
    StopLlm {
        session: u64,
    },
    Insert {
        session: u64,
        text: String,
        copy: bool,
    },
    SaveRow {
        session: u64,
        status: String,
        text: Option<String>,
        code: Option<String>,
    },
    UpdatePasteLast {
        text: String,
    },
    PasteLast,
    MaxDurationWarning {
        seconds_left: u64,
    },
    Notify {
        session: u64,
        code: String,
    },
}

impl From<Effect> for DictationEffect {
    fn from(effect: Effect) -> Self {
        match effect {
            Effect::StartCapture { session } => Self::StartCapture { session },
            Effect::StopCapture { session, keep } => Self::StopCapture { session, keep },
            Effect::HoldCapture { session } => Self::HoldCapture { session },
            Effect::DropAudio { session } => Self::DropAudio { session },
            Effect::Cue(cue) => Self::Cue {
                name: match cue {
                    dictation::Cue::Start => "start",
                    dictation::Cue::Stop => "stop",
                    dictation::Cue::Cancel => "cancel",
                }
                .into(),
            },
            Effect::Transcribe { session } => Self::Transcribe { session },
            Effect::CancelTranscribe { session } => Self::CancelTranscribe { session },
            Effect::Clean { session, raw } => Self::Clean { session, raw },
            Effect::Edit {
                session,
                instruction,
            } => Self::Edit {
                session,
                instruction,
            },
            Effect::StopLlm { session } => Self::StopLlm { session },
            Effect::Insert {
                session,
                text,
                delivery,
            } => Self::Insert {
                session,
                text,
                copy: delivery == dictation::Delivery::Copy,
            },
            Effect::SaveRow {
                session,
                status,
                text,
                code,
            } => Self::SaveRow {
                session,
                status: match status {
                    dictation::RowStatus::Done => "done",
                    dictation::RowStatus::Cancelled => "cancelled",
                    dictation::RowStatus::Failed => "failed",
                }
                .into(),
                text,
                code: code.map(str::to_owned),
            },
            Effect::UpdatePasteLast { text } => Self::UpdatePasteLast { text },
            Effect::PasteLast => Self::PasteLast,
            Effect::MaxDurationWarning { seconds_left } => {
                Self::MaxDurationWarning { seconds_left }
            }
            Effect::Notify { session, code } => Self::Notify {
                session,
                code: code.into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DictationSnapshot {
    pub session: u64,
    pub state: String,
    pub mode: String,
    pub finishing_sessions: u64,
    pub recoverable_session: Option<u64>,
}

#[derive(uniffi::Object)]
pub struct Dictation {
    machine: Mutex<DictationMachine>,
}

#[uniffi::export]
impl Dictation {
    #[uniffi::constructor]
    pub fn new(max_minutes: u64) -> Result<Arc<Self>, MobileError> {
        if !(1..=60).contains(&max_minutes) {
            return Err(MobileError::invalid(
                "maximum recording time must be 1 to 60 minutes",
            ));
        }
        Ok(Arc::new(Self {
            machine: Mutex::new(DictationMachine::new(dictation::Config::with_max_minutes(
                max_minutes,
            ))),
        }))
    }

    pub fn handle(
        &self,
        event: DictationEvent,
        now_ms: u64,
    ) -> Result<Vec<DictationEffect>, MobileError> {
        if matches!(&event, DictationEvent::Transcribed { text, .. } | DictationEvent::Cleaned { text, .. } if text.len() > 1024 * 1024)
        {
            return Err(MobileError::invalid("dictation text exceeds 1 MiB"));
        }
        Ok(lock(&self.machine)
            .handle(event.into(), now_ms)
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn snapshot(&self, now_ms: u64) -> DictationSnapshot {
        let machine = lock(&self.machine);
        DictationSnapshot {
            session: machine.session(),
            state: machine.state().key().into(),
            mode: machine.mode().key().into(),
            finishing_sessions: machine.behind() as u64,
            recoverable_session: machine.recoverable(now_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_dictation_reuses_the_core_pipeline_and_ignores_stale_results() {
        let dictation = Dictation::new(6).unwrap();
        assert!(
            dictation
                .handle(DictationEvent::Toggle, 0)
                .unwrap()
                .contains(&DictationEffect::StartCapture { session: 1 })
        );
        assert!(
            dictation
                .handle(DictationEvent::Toggle, 1000)
                .unwrap()
                .contains(&DictationEffect::Transcribe { session: 1 })
        );
        assert!(
            dictation
                .handle(
                    DictationEvent::Transcribed {
                        session: 99,
                        text: "stale".into()
                    },
                    1001
                )
                .unwrap()
                .is_empty()
        );
        assert_eq!(dictation.snapshot(1001).state, "transcribing");
        assert!(
            dictation
                .handle(
                    DictationEvent::Transcribed {
                        session: 1,
                        text: "hello".into()
                    },
                    1002
                )
                .unwrap()
                .contains(&DictationEffect::Clean {
                    session: 1,
                    raw: "hello".into()
                })
        );
        assert!(
            dictation
                .handle(
                    DictationEvent::Cleaned {
                        session: 1,
                        text: "Hello".into()
                    },
                    1003
                )
                .unwrap()
                .contains(&DictationEffect::Insert {
                    session: 1,
                    text: "Hello".into(),
                    copy: false
                })
        );
        dictation
            .handle(DictationEvent::Inserted { session: 1 }, 1004)
            .unwrap();
        assert_eq!(dictation.snapshot(1004).state, "done");
    }

    #[test]
    fn cancelling_and_recovering_preserves_core_deadlines() {
        let dictation = Dictation::new(1).unwrap();
        dictation.handle(DictationEvent::HoldDown, 100).unwrap();
        assert!(
            dictation
                .handle(DictationEvent::Cancel, 500)
                .unwrap()
                .contains(&DictationEffect::HoldCapture { session: 1 })
        );
        assert_eq!(dictation.snapshot(4499).recoverable_session, Some(1));
        assert_eq!(dictation.snapshot(4500).recoverable_session, None);
        assert!(Dictation::new(0).is_err());
        assert!(Dictation::new(u64::MAX).is_err());
    }

    #[test]
    fn oversized_results_fail_before_the_reducer_changes() {
        let dictation = Dictation::new(6).unwrap();
        dictation.handle(DictationEvent::Toggle, 0).unwrap();
        dictation.handle(DictationEvent::Toggle, 1000).unwrap();
        let before = dictation.snapshot(1001);
        for event in [
            DictationEvent::Transcribed {
                session: 1,
                text: "x".repeat(1024 * 1024 + 1),
            },
            DictationEvent::Cleaned {
                session: 1,
                text: "x".repeat(1024 * 1024 + 1),
            },
        ] {
            assert!(dictation.handle(event, 1001).is_err());
            assert_eq!(dictation.snapshot(1001), before);
        }
    }
}
