//! The microphone step: a short recording, its level, and what the engine made of it.

use super::Onboarding;
use crate::controller::CancelToken;
use crate::hook;
use crate::mic::CaptureState;
use gpui_kit::Context;
use hushpen_core::error::ENGINE_NO_SPEECH;
use hushpen_core::onboarding::{self as rules, Heard, MicVerdict};
use hushpen_engine::{JobOutcome, TranscribeSpec};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicPhase {
    Idle,
    Listening,
    /// The recording is being turned into words.
    Checking,
}

impl MicPhase {
    pub(super) fn key(self) -> &'static str {
        match self {
            MicPhase::Idle => "idle",
            MicPhase::Listening => "listening",
            MicPhase::Checking => "checking",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MicTest {
    pub phase: MicPhase,
    /// The loudest level of the newest test, 0 to 1.
    pub peak: f32,
    pub verdict: Option<MicVerdict>,
    pub passed: bool,
}

pub(super) enum Message {
    Heard(Heard),
}

impl Onboarding {
    pub fn mic_test(&self) -> &MicTest {
        &self.mic_test
    }

    /// The page names the microphone, so it looks for one every second while it is on screen.
    /// The Mic service alone looks every 2 s.
    pub(super) fn look_for_microphone(&mut self, cx: &mut Context<Self>) {
        if self.active() && self.mic.read(cx).state() != CaptureState::Listening {
            self.mic.update(cx, |mic, cx| mic.refresh(cx));
        }
    }

    pub(super) fn mic_changed(&mut self, cx: &mut Context<Self>) {
        let (listed, none, loudest, listening) = {
            let mic = self.mic.read(cx);
            (
                mic.listed(),
                mic.devices().is_empty(),
                mic.meter().into_iter().fold(0.0_f32, f32::max),
                mic.state() == CaptureState::Listening,
            )
        };
        if self.waiting_for_mic && listed {
            self.waiting_for_mic = false;
            if none {
                self.reopen_for_repair(cx);
            }
        }
        if self.mic_test.phase == MicPhase::Listening {
            self.mic_test.peak = self.mic_test.peak.max(loudest);
            if !listening {
                // The microphone went away under the test.
                self.mic_test.phase = MicPhase::Idle;
                self.mic_test.verdict = None;
                self.notice = Some("The microphone stopped. Check it and test again.".into());
            }
        }
        cx.notify();
    }

    pub(super) fn handle(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Heard(heard) => {
                if self.mic_test.phase != MicPhase::Checking {
                    return;
                }
                self.finish_mic_test(heard, cx);
            }
        }
    }

    /// The test button: start a test, or stop the running one.
    pub fn toggle_mic_test(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        match self.mic_test.phase {
            MicPhase::Checking => Err("the last test is still being read".into()),
            MicPhase::Listening => self.stop_mic_test(cx),
            MicPhase::Idle => {
                self.notice = None;
                match self
                    .mic
                    .update(cx, |mic, cx| mic.start(crate::mic::CaptureUse::Test, cx))
                {
                    Ok(_) => {
                        self.mic_test = MicTest {
                            phase: MicPhase::Listening,
                            peak: 0.0,
                            verdict: None,
                            passed: false,
                        };
                        hook::record_event("onboarding", "mic-test start");
                        cx.notify();
                        Ok(())
                    }
                    Err(error) => {
                        self.notice = self
                            .mic
                            .read(cx)
                            .notice()
                            .map(|notice| notice.message.clone());
                        cx.notify();
                        Err(error.to_string())
                    }
                }
            }
        }
    }

    fn stop_mic_test(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let stopped = self.mic.update(cx, |mic, cx| mic.stop(true, cx));
        let loudest = self
            .mic
            .read(cx)
            .meter()
            .into_iter()
            .fold(0.0_f32, f32::max);
        self.mic_test.peak = self.mic_test.peak.max(loudest);
        hook::record_event("onboarding", "mic-test stop");
        let finished = match stopped {
            Ok(Some(finished)) => finished,
            Ok(None) | Err(_) => {
                self.mic_test.phase = MicPhase::Idle;
                self.notice = Some("The recording could not be read. Test again.".into());
                cx.notify();
                return Err("the test recording ended early".into());
            }
        };
        if self.mic_test.peak < rules::HEARD_LEVEL || self.models.read(cx).usable().is_none() {
            let _ = std::fs::remove_file(&finished.path);
            let heard = Heard::NotTried;
            self.mic_test.phase = MicPhase::Checking;
            self.finish_mic_test(heard, cx);
            return Ok(());
        }
        self.mic_test.phase = MicPhase::Checking;
        self.transcribe(finished.path, cx);
        cx.notify();
        Ok(())
    }

    /// Turns the test recording into words on a worker thread, then deletes it.
    fn transcribe(&mut self, wav: PathBuf, cx: &mut Context<Self>) {
        let (run, spawn, language) = {
            let controller = self.controller.read(cx);
            (
                controller.runner(),
                controller.spawner(),
                controller.language(),
            )
        };
        let spec = TranscribeSpec {
            wav_path: wav.clone(),
            language: (language != hushpen_core::language::AUTO).then_some(language),
            prompt: None,
        };
        let messages = self.messages.clone();
        let job = {
            let wav = wav.clone();
            Box::new(move || {
                let heard = match run(spec, Arc::new(CancelToken::default())) {
                    JobOutcome::Done(done) => Heard::Text(done.text),
                    JobOutcome::Failed(failure) if failure.code == ENGINE_NO_SPEECH => {
                        Heard::Text(String::new())
                    }
                    JobOutcome::Failed(_) | JobOutcome::Cancelled => Heard::Failed,
                };
                let _ = std::fs::remove_file(&wav);
                let _ = messages.unbounded_send(Message::Heard(heard));
            })
        };
        if let Err(error) = spawn(job) {
            log::warn!("the microphone test could not start the engine job: {error}");
            let _ = std::fs::remove_file(&wav);
            self.finish_mic_test(Heard::Failed, cx);
        }
    }

    fn finish_mic_test(&mut self, heard: Heard, cx: &mut Context<Self>) {
        let verdict = rules::mic_verdict(self.mic_test.peak, heard);
        self.mic_test.passed = matches!(verdict, MicVerdict::Passed { .. });
        hook::record_event(
            "onboarding",
            &format!(
                "mic-test {}",
                if self.mic_test.passed {
                    "passed"
                } else {
                    "not-passed"
                }
            ),
        );
        self.mic_test.verdict = Some(verdict);
        self.mic_test.phase = MicPhase::Idle;
        cx.notify();
    }
}
