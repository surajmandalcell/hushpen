//! The hold key while the Command Mode chord can still follow. The key arms the hold: the
//! microphone opens so no speech is lost, but the machine hears nothing, so no cue plays, the
//! flow bar stays hidden, and no history row can exist. If the rest of the chord joins inside
//! [`ARM_WINDOW_MS`] the armed audio becomes the command's; otherwise the hold starts as it
//! always did, dated at the key press.

use super::Controller;
use crate::mic::CaptureUse;
use gpui_kit::Context;
use hushpen_core::dictation::{ARM_WINDOW_MS, AppEvent, State};
use hushpen_core::onboarding::KeyGate;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub(super) struct Armed {
    /// When the hold key went down, on the controller's clock.
    pub(super) at: u64,
    seq: u64,
    /// This arm opened the microphone, so it has to close it if no session takes it over.
    pub(super) mic: bool,
}

impl Controller {
    /// Arms a hold, or starts it at once when no chord can follow: a busy pipeline, a closed
    /// onboarding gate, or a start that is refused anyway.
    pub(super) fn arm_hold(&mut self, at: u64, cx: &mut Context<Self>) -> Result<(), String> {
        let arm = self.gate == KeyGate::Open
            && self.machine.state() == State::Idle
            && self.blocker(cx).is_none();
        if !arm {
            return self.dispatch_at(AppEvent::HoldDown, Some(at), cx);
        }
        let mic = self.mic.read(cx).session_use().is_none()
            && self
                .mic
                .update(cx, |mic, cx| mic.start(CaptureUse::Dictation, cx))
                .is_ok();
        self.arm_seq += 1;
        let seq = self.arm_seq;
        self.armed = Some(Armed { at, seq, mic });
        let wait = ARM_WINDOW_MS.saturating_sub((self.clock)().saturating_sub(at));
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(wait))
                .await;
            let _ = this.update(cx, |me, cx| me.settle_arm(seq, cx));
        })
        .detach();
        Ok(())
    }

    fn settle_arm(&mut self, seq: u64, cx: &mut Context<Self>) {
        if self.armed.is_some_and(|armed| armed.seq == seq) {
            let _ = self.promote_armed(cx);
        }
    }

    /// Starts the armed hold as a plain dictation, dated at the key press.
    pub(super) fn promote_armed(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(armed) = self.armed.take() else {
            return Ok(());
        };
        let outcome = self.dispatch_at(AppEvent::HoldDown, Some(armed.at), cx);
        self.release_unused_capture(armed, cx);
        outcome
    }

    /// Closes the microphone that an arm opened when no session started on it.
    pub(super) fn release_unused_capture(&mut self, armed: Armed, cx: &mut Context<Self>) {
        if armed.mic && self.machine.state() != State::Listening {
            let _ = self.mic.update(cx, |mic, cx| mic.stop(false, cx));
            self.wav = None;
        }
    }
}
