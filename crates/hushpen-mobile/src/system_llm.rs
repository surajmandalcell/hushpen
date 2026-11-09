use crate::MobileError;
use hushpen_core::cleanup::{self, guard, outcome::Outcome};
use hushpen_core::llm_prompt::{CLEANUP_SYSTEM, Controls};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

const CLEANUP_TIMEOUT: Duration = Duration::from_secs(30);

/// The native app provides its system model. Calls run off the UI thread.
#[uniffi::export(with_foreign)]
pub trait SystemLlm: Send + Sync {
    fn available(&self) -> bool;
    fn cleanup(&self, text: String, instructions: String) -> Result<String, MobileError>;
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CleanupResult {
    pub text: String,
    pub outcome: String,
}

#[derive(uniffi::Object)]
pub struct SystemCleanup {
    system: Arc<dyn SystemLlm>,
    busy: Arc<AtomicBool>,
}

struct Busy(Arc<AtomicBool>);

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[uniffi::export]
impl SystemCleanup {
    #[uniffi::constructor]
    pub fn new(system: Arc<dyn SystemLlm>) -> Arc<Self> {
        Arc::new(Self {
            system,
            busy: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn clean(&self, raw: String) -> Result<CleanupResult, MobileError> {
        self.clean_for(raw, CLEANUP_TIMEOUT)
    }

    pub fn available(&self) -> bool {
        self.invoke(Duration::from_secs(2), |system| system.available())
            .unwrap_or(false)
    }
}

impl SystemCleanup {
    fn clean_for(&self, raw: String, timeout: Duration) -> Result<CleanupResult, MobileError> {
        let rule = rule_text(&raw)?;
        self.run(rule, CLEANUP_SYSTEM.to_owned(), timeout)
    }

    pub(crate) fn clean_with_instructions(
        &self,
        text: String,
        instructions: String,
    ) -> Result<CleanupResult, MobileError> {
        self.run(text, instructions, CLEANUP_TIMEOUT)
    }

    fn run(
        &self,
        rule: String,
        mut instructions: String,
        timeout: Duration,
    ) -> Result<CleanupResult, MobileError> {
        if rule.len() > 1024 * 1024 || instructions.len() > 1024 * 1024 {
            return Err(MobileError::invalid(
                "cleanup text or instructions exceed 1 MiB",
            ));
        }
        if Controls::parse(&instructions).is_some() {
            instructions = format!("{CLEANUP_SYSTEM}\n{instructions}");
        }
        let input = rule.clone();
        Ok(
            match self.invoke(timeout, move |system| {
                system
                    .available()
                    .then(|| system.cleanup(input, instructions))
            }) {
                Ok(Some(Ok(output))) => match guard::check(&rule, &output) {
                    Ok(text) => result(text, Outcome::Used),
                    Err(_) => result(rule, Outcome::GuardRejected),
                },
                Ok(None) => result(rule, Outcome::Unavailable),
                Ok(Some(Err(_))) => result(rule, Outcome::Error),
                Err(outcome) => result(rule, outcome),
            },
        )
    }

    fn invoke<T: Send + 'static>(
        &self,
        timeout: Duration,
        call: impl FnOnce(Arc<dyn SystemLlm>) -> T + Send + 'static,
    ) -> Result<T, Outcome> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Outcome::Unavailable);
        }
        let busy = Busy(Arc::clone(&self.busy));
        let system = Arc::clone(&self.system);
        let (sender, receiver) = mpsc::sync_channel(1);
        if std::thread::Builder::new()
            .name("hushpen-system-cleanup".into())
            .spawn(move || {
                let guard = busy;
                let output = call(system);
                drop(guard);
                let _ = sender.send(output);
            })
            .is_err()
        {
            return Err(Outcome::Error);
        }
        receiver.recv_timeout(timeout).map_err(|error| match error {
            RecvTimeoutError::Disconnected => Outcome::Error,
            RecvTimeoutError::Timeout => Outcome::Timeout,
        })
    }
}

/// Uses the same rule cleanup and output checks as desktop dictation.
#[uniffi::export]
pub fn clean_text(
    raw: String,
    system: Option<Arc<SystemCleanup>>,
) -> Result<CleanupResult, MobileError> {
    match system {
        Some(system) => system.clean(raw),
        None => Ok(result(rule_text(&raw)?, Outcome::Off)),
    }
}

fn rule_text(raw: &str) -> Result<String, MobileError> {
    if raw.len() > 1024 * 1024 {
        return Err(MobileError::invalid("cleanup text exceeds 1 MiB"));
    }
    Ok(cleanup::clean(raw, &cleanup::Options::default()))
}

fn result(text: String, outcome: Outcome) -> CleanupResult {
    CleanupResult {
        text,
        outcome: outcome.key().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct System(bool, Result<String, MobileError>);

    impl SystemLlm for System {
        fn available(&self) -> bool {
            self.0
        }

        fn cleanup(&self, _: String, _: String) -> Result<String, MobileError> {
            self.1.clone()
        }
    }

    #[test]
    fn cleanup_uses_the_system_model_or_keeps_rule_text() {
        let raw = "um hello there".to_owned();
        let rule = clean_text(raw.clone(), None).unwrap();
        assert_eq!(
            (rule.text.as_str(), rule.outcome.as_str()),
            ("Hello there.", "off")
        );
        for (system, text, outcome) in [
            (
                System(true, Ok("Hello there!".into())),
                "Hello there!",
                "used",
            ),
            (
                System(false, Ok("unused".into())),
                "Hello there.",
                "unavailable",
            ),
            (
                System(true, Err(MobileError::failed("unavailable"))),
                "Hello there.",
                "error",
            ),
            (
                System(true, Ok("I cannot help.".into())),
                "Hello there.",
                "guard_rejected",
            ),
        ] {
            let cleaned =
                clean_text(raw.clone(), Some(SystemCleanup::new(Arc::new(system)))).unwrap();
            assert_eq!(
                (cleaned.text.as_str(), cleaned.outcome.as_str()),
                (text, outcome)
            );
        }
    }

    #[test]
    fn a_timeout_keeps_rule_text_and_blocks_more_callbacks_until_the_first_returns() {
        struct Slow {
            finish: Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>,
        }
        impl SystemLlm for Slow {
            fn available(&self) -> bool {
                true
            }
            fn cleanup(&self, text: String, _: String) -> Result<String, MobileError> {
                let (mutex, condition) = &*self.finish;
                let guard = crate::lock(mutex);
                let _guard = condition
                    .wait_timeout_while(guard, Duration::from_secs(1), |done| !*done)
                    .unwrap();
                Ok(text)
            }
        }
        let finish = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let cleanup = SystemCleanup::new(Arc::new(Slow {
            finish: Arc::clone(&finish),
        }));
        let first = cleanup
            .clean_for("hello there".into(), Duration::from_millis(10))
            .unwrap();
        assert_eq!(
            (first.text.as_str(), first.outcome.as_str()),
            ("Hello there.", "timeout")
        );
        let next = cleanup
            .clean_for("hello there".into(), Duration::from_millis(10))
            .unwrap();
        assert_eq!(next.outcome, "unavailable");
        *crate::lock(&finish.0) = true;
        finish.1.notify_all();
        let until = std::time::Instant::now() + Duration::from_secs(1);
        while cleanup.busy.load(Ordering::Acquire) && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!cleanup.busy.load(Ordering::Acquire));
        assert_eq!(cleanup.clean("hello there".into()).unwrap().outcome, "used");
    }

    #[test]
    fn rust_supplies_local_instructions_and_preserves_desktop_instructions_and_text() {
        struct Recording(Arc<std::sync::Mutex<Vec<(String, String)>>>);
        impl SystemLlm for Recording {
            fn available(&self) -> bool {
                true
            }
            fn cleanup(&self, text: String, instructions: String) -> Result<String, MobileError> {
                crate::lock(&self.0).push((text.clone(), instructions));
                Ok(text)
            }
        }
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let system = SystemCleanup::new(Arc::new(Recording(Arc::clone(&seen))));
        system.clean("um hello there".into()).unwrap();
        let desktop = "hello full stop";
        let instructions = hushpen_core::llm_prompt::cleanup_system_keeping(&["Zorvath".into()]);
        let result = system
            .clean_with_instructions(desktop.into(), instructions.clone())
            .unwrap();
        assert_eq!(result.text, desktop);
        let controls = Controls::default().line();
        system
            .clean_with_instructions(desktop.into(), controls.clone())
            .unwrap();
        assert_eq!(
            *crate::lock(&seen),
            [
                ("Hello there.".into(), CLEANUP_SYSTEM.into()),
                (desktop.into(), instructions),
                (desktop.into(), format!("{CLEANUP_SYSTEM}\n{controls}")),
            ]
        );
        assert!(
            system
                .clean_with_instructions(desktop.into(), "x".repeat(1024 * 1024 + 1))
                .is_err()
        );
        assert_eq!(crate::lock(&seen).len(), 3);
    }

    #[test]
    fn hosted_cleanup_keeps_the_desktop_text_when_output_fails_its_guard() {
        let system = SystemCleanup::new(Arc::new(System(true, Ok("I cannot help.".into()))));
        let result = system
            .clean_with_instructions(
                "hello full stop".into(),
                "Preserve spoken punctuation".into(),
            )
            .unwrap();
        assert_eq!(
            (result.text.as_str(), result.outcome.as_str()),
            ("hello full stop", "guard_rejected")
        );
    }
}
