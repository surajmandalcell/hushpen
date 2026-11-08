//! Capture ownership and event generations, without a microphone stream.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Idle,
    Listening,
    Failed,
}

impl CaptureState {
    pub fn key(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Listening => "listening",
            Self::Failed => "failed",
        }
    }
}

/// Who owns the microphone. Different purposes never share a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureUse {
    Test,
    Dictation,
    Sample,
    Voice,
}

impl CaptureUse {
    pub fn key(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Dictation => "dictation",
            Self::Sample => "sample",
            Self::Voice => "voice",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Test => "the microphone test",
            Self::Dictation => "dictation",
            Self::Sample => "the audio enhance sample",
            Self::Voice => "a voice recording",
        }
    }

    /// Samples and voice takes keep the unmodified microphone input.
    pub fn is_raw(self) -> bool {
        matches!(self, Self::Sample | Self::Voice)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    Shared,
    Open { generation: u64 },
}

#[derive(Debug)]
pub struct Machine {
    state: CaptureState,
    generation: u64,
    active: Option<(u64, CaptureUse)>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            state: CaptureState::Idle,
            generation: 0,
            active: None,
        }
    }
}

impl Machine {
    pub fn state(&self) -> CaptureState {
        self.state
    }

    pub fn used_for(&self) -> Option<CaptureUse> {
        self.active.map(|(_, used_for)| used_for)
    }

    /// The same purpose shares the recording; another purpose gets its current owner.
    pub fn start(&mut self, used_for: CaptureUse) -> Result<Start, CaptureUse> {
        if let Some((_, owner)) = self.active {
            return if owner == used_for {
                Ok(Start::Shared)
            } else {
                Err(owner)
            };
        }
        self.generation += 1;
        Ok(Start::Open {
            generation: self.generation,
        })
    }

    pub fn opened(&mut self, generation: u64, used_for: CaptureUse) {
        self.active = Some((generation, used_for));
        self.state = CaptureState::Listening;
    }

    pub fn accepts(&self, generation: u64) -> bool {
        self.active.is_some_and(|(live, _)| live == generation)
    }

    /// A stopped or detached stream no longer accepts events, even while its WAV finishes.
    pub fn ended(&mut self, failed: bool) {
        self.active = None;
        self.state = if failed {
            CaptureState::Failed
        } else {
            CaptureState::Idle
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_purpose_shares_only_with_itself() {
        for purpose in [
            CaptureUse::Test,
            CaptureUse::Dictation,
            CaptureUse::Sample,
            CaptureUse::Voice,
        ] {
            let mut machine = Machine::default();
            assert_eq!(machine.start(purpose), Ok(Start::Open { generation: 1 }));
            machine.opened(1, purpose);
            for next in [
                CaptureUse::Test,
                CaptureUse::Dictation,
                CaptureUse::Sample,
                CaptureUse::Voice,
            ] {
                assert_eq!(
                    machine.start(next),
                    if next == purpose {
                        Ok(Start::Shared)
                    } else {
                        Err(purpose)
                    }
                );
            }
            assert_eq!(machine.used_for(), Some(purpose));
            assert_eq!(machine.state(), CaptureState::Listening);
        }
    }

    #[test]
    fn old_events_cannot_reach_a_new_or_failed_session() {
        let mut machine = Machine::default();
        machine.start(CaptureUse::Test).unwrap();
        machine.opened(1, CaptureUse::Test);
        assert!(machine.accepts(1));
        machine.ended(false);
        assert!(!machine.accepts(1));
        assert_eq!(machine.state(), CaptureState::Idle);
        assert_eq!(
            machine.start(CaptureUse::Voice),
            Ok(Start::Open { generation: 2 })
        );
        machine.ended(true);
        assert_eq!(machine.state(), CaptureState::Failed);
        assert_eq!(
            machine.start(CaptureUse::Dictation),
            Ok(Start::Open { generation: 3 })
        );
        machine.opened(3, CaptureUse::Dictation);
        assert!(!machine.accepts(1));
        assert!(!machine.accepts(2));
        assert!(machine.accepts(3));
    }

    #[test]
    fn only_samples_and_voice_takes_skip_gain_and_enhance() {
        assert!(!CaptureUse::Test.is_raw());
        assert!(!CaptureUse::Dictation.is_raw());
        assert!(CaptureUse::Sample.is_raw());
        assert!(CaptureUse::Voice.is_raw());
    }
}
