//! The AI step of a dictation: gets the cleanup model ready, runs one job with a timeout that
//! counts only the generation, and checks the answer. Every way it can go wrong ends in the rule
//! text, never in an error for the user.

mod endpoint;

pub use endpoint::EndpointCleaner;

use crate::controller::CancelToken;
use crate::llm_host::LlmHost;
use crate::net::endpoint::Transport;
use crate::storage::Storage;
use hushpen_core::cleanup::guard;
use hushpen_core::cleanup::outcome::Outcome;
use hushpen_core::error::{LLM_GUARD_REJECTED, LLM_UNAVAILABLE};
use hushpen_core::llm_prompt::CLEANUP_SYSTEM;
use hushpen_engine::{LlmClient, LlmLoaded, LlmOutcome, LlmSpec, LlmState, LlmWork};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// The longest the first dictation waits for the model to load. A first Metal load compiles
/// shaders for 12 to 16 s, so this is far above the cleanup timeout, which starts only after.
pub const LOAD_WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);
const MIN_TOKENS: usize = 32;
const MAX_TOKENS: usize = 1024;

/// Where a running AI step stands, for the screens and the hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The model is loading.
    Preparing,
    /// The model is writing the cleaned text.
    Cleaning,
}

impl Phase {
    pub fn key(self) -> &'static str {
        match self {
            Phase::Preparing => "preparing",
            Phase::Cleaning => "cleaning",
        }
    }
}

/// The phase of the running step, shared with the thread that runs it.
#[derive(Default)]
pub struct Progress(AtomicU8);

impl Progress {
    pub fn set(&self, phase: Option<Phase>) {
        let value = match phase {
            None => 0,
            Some(Phase::Preparing) => 1,
            Some(Phase::Cleaning) => 2,
        };
        self.0.store(value, Ordering::SeqCst);
    }

    pub fn get(&self) -> Option<Phase> {
        match self.0.load(Ordering::SeqCst) {
            1 => Some(Phase::Preparing),
            2 => Some(Phase::Cleaning),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    Done(String),
    TimedOut,
    Cancelled,
    /// The error code of the failure.
    Failed(String),
}

/// What runs the model. The real one is [`HostCleaner`]; tests pass a fake.
pub trait Cleaner: Send + Sync {
    /// Gets the model into memory and returns the code of the failure when it cannot.
    fn prepare(&self, token: &CancelToken, progress: &Progress) -> Result<(), String>;

    /// Runs one job. `timeout` counts from the moment the job is sent.
    fn clean(&self, text: &str, timeout: Duration, token: &CancelToken) -> Attempt;
}

/// How one AI step ended: the outcome, the text to use, and what the history row keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub outcome: Outcome,
    /// The error code behind the outcome, or `OK`.
    pub code: String,
    /// The model's text, only when it was used.
    pub text: Option<String>,
    pub ms: u64,
}

impl Record {
    pub fn off() -> Self {
        Self {
            outcome: Outcome::Off,
            code: Outcome::Off.code(None),
            text: None,
            ms: 0,
        }
    }

    fn failed(outcome: Outcome, failure: Option<&str>, began: Instant) -> Self {
        Self {
            outcome,
            code: outcome.code(failure),
            text: None,
            ms: elapsed_ms(began),
        }
    }
}

/// Runs the AI step for `rule`, the text the rules produced. A `None` result means the user
/// cancelled and nothing should be inserted.
pub fn run(
    cleaner: Option<&dyn Cleaner>,
    rule: &str,
    timeout: Duration,
    token: &CancelToken,
    progress: &Progress,
) -> Option<Record> {
    let began = Instant::now();
    let Some(cleaner) = cleaner else {
        return Some(Record::failed(
            Outcome::Unavailable,
            Some(LLM_UNAVAILABLE),
            began,
        ));
    };
    progress.set(Some(Phase::Preparing));
    let prepared = cleaner.prepare(token, progress);
    if token.is_cancelled() {
        progress.set(None);
        return None;
    }
    if let Err(code) = prepared {
        progress.set(None);
        return Some(Record::failed(
            Outcome::of_failure(&code),
            Some(&code),
            began,
        ));
    }
    progress.set(Some(Phase::Cleaning));
    // The clock of the job starts here: the load above is not part of it.
    let began = Instant::now();
    let attempt = cleaner.clean(rule, timeout, token);
    progress.set(None);
    match attempt {
        Attempt::Cancelled => None,
        Attempt::TimedOut => Some(Record::failed(Outcome::Timeout, None, began)),
        Attempt::Failed(code) => Some(Record::failed(
            Outcome::of_failure(&code),
            Some(&code),
            began,
        )),
        Attempt::Done(text) => Some(match guard::check(rule, &text) {
            Ok(text) => Record {
                outcome: Outcome::Used,
                code: Outcome::Used.code(None),
                text: Some(text),
                ms: elapsed_ms(began),
            },
            Err(_) => Record::failed(Outcome::GuardRejected, Some(LLM_GUARD_REJECTED), began),
        }),
    }
}

fn elapsed_ms(began: Instant) -> u64 {
    u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// How many tokens a cleaned text may take: the guard's length limit, at about 2 characters a
/// token so that a language with long words is not cut short.
fn max_tokens(text: &str) -> u32 {
    let chars = text.chars().count();
    let tokens = (chars * 3 / 2 + 40) / 2;
    u32::try_from(tokens.clamp(MIN_TOKENS, MAX_TOKENS)).unwrap_or(MAX_TOKENS as u32)
}

/// The cleaner for the provider that the settings name, read at the start of each dictation so
/// a change takes effect on the next one. `None` means the local worker is not running.
pub fn cleaner_for(
    storage: &Storage,
    host: &LlmHost,
    transport: &Arc<dyn Transport>,
) -> Option<Box<dyn Cleaner>> {
    let endpoint = storage
        .settings
        .get("cleanup.llm.provider")
        .is_some_and(|provider| provider.as_str() == Some("endpoint"));
    if endpoint {
        return Some(Box::new(EndpointCleaner::from_storage(
            storage,
            Arc::clone(transport),
        )));
    }
    HostCleaner::of(host).map(|cleaner| Box::new(cleaner) as Box<dyn Cleaner>)
}

/// The worker behind [`LlmHost`], with the load that brings its model back after an idle unload.
pub struct HostCleaner {
    client: LlmClient,
    spec: LlmSpec,
}

impl HostCleaner {
    /// `None` while AI cleanup is off or has no verified model, so no worker runs.
    pub fn of(host: &LlmHost) -> Option<Self> {
        let (client, spec) = host.running()?;
        Some(Self { client, spec })
    }

    fn load(&self) -> Result<LlmLoaded, String> {
        self.client
            .load_model(self.spec.clone())
            .wait()
            .map_err(|failure| failure.code)
    }
}

impl Cleaner for HostCleaner {
    fn prepare(&self, token: &CancelToken, _progress: &Progress) -> Result<(), String> {
        let deadline = Instant::now() + LOAD_WAIT;
        loop {
            if token.is_cancelled() {
                return Ok(());
            }
            let status = self.client.status();
            if status.model.is_some() {
                return Ok(());
            }
            match status.state {
                LlmState::Backoff | LlmState::Stopped => return Err(LLM_UNAVAILABLE.to_owned()),
                LlmState::Ready => {
                    // The worker dropped the model after the idle time. Loading it here keeps
                    // the load out of the cleanup timeout.
                    return self.load().map(drop);
                }
                LlmState::Starting | LlmState::Loading => {}
            }
            if Instant::now() >= deadline {
                return Err(LLM_UNAVAILABLE.to_owned());
            }
            self.client.wait_for(POLL, |status| {
                status.model.is_some()
                    || !matches!(status.state, LlmState::Starting | LlmState::Loading)
            });
        }
    }

    fn clean(&self, text: &str, timeout: Duration, token: &CancelToken) -> Attempt {
        let handle = self.client.run(LlmWork::Cleanup {
            system: CLEANUP_SYSTEM.to_owned(),
            user: text.to_owned(),
            max_tokens: max_tokens(text),
        });
        let id = handle.id();
        let client = self.client.clone();
        token.on_cancel(move || client.cancel(id));
        match handle.wait_timeout(timeout) {
            Some(LlmOutcome::Done(done)) => Attempt::Done(done.text),
            Some(LlmOutcome::Cancelled) => Attempt::Cancelled,
            Some(LlmOutcome::Failed(failure)) => Attempt::Failed(failure.code),
            None => {
                // The rule text goes in now; the worker settles the cancel on its own time.
                handle.cancel();
                if token.is_cancelled() {
                    Attempt::Cancelled
                } else {
                    Attempt::TimedOut
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::error::{LLM_CRASHED, LLM_LOAD_FAILED};
    use std::sync::Mutex;

    struct Fake {
        prepare: Result<(), String>,
        attempt: Attempt,
        seen: Mutex<Vec<String>>,
        phase_during_clean: Mutex<Option<Phase>>,
        progress: Mutex<Option<std::sync::Arc<Progress>>>,
    }

    impl Fake {
        fn answering(attempt: Attempt) -> Self {
            Self {
                prepare: Ok(()),
                attempt,
                seen: Mutex::default(),
                phase_during_clean: Mutex::default(),
                progress: Mutex::default(),
            }
        }
    }

    impl Cleaner for Fake {
        fn prepare(&self, _: &CancelToken, progress: &Progress) -> Result<(), String> {
            assert_eq!(progress.get(), Some(Phase::Preparing));
            self.prepare.clone()
        }

        fn clean(&self, text: &str, _: Duration, _: &CancelToken) -> Attempt {
            self.seen.lock().unwrap().push(text.to_owned());
            if let Some(progress) = &*self.progress.lock().unwrap() {
                *self.phase_during_clean.lock().unwrap() = progress.get();
            }
            self.attempt.clone()
        }
    }

    const RULE: &str = "So I think we should meet on Tuesday, no wait, Wednesday at three PM.";

    fn go(fake: &Fake) -> Option<Record> {
        run(
            Some(fake),
            RULE,
            Duration::from_secs(3),
            &CancelToken::default(),
            &Progress::default(),
        )
    }

    #[test]
    fn a_good_answer_is_used() {
        let fake = Fake::answering(Attempt::Done(
            "I think we should meet on Wednesday at 3 PM.".into(),
        ));
        let record = go(&fake).unwrap();
        assert_eq!(record.outcome, Outcome::Used);
        assert_eq!(
            record.text.as_deref(),
            Some("I think we should meet on Wednesday at 3 PM.")
        );
        assert_eq!(record.code, "OK");
        assert_eq!(*fake.seen.lock().unwrap(), [RULE]);
    }

    #[test]
    fn the_progress_reads_cleaning_while_the_job_runs_and_nothing_after() {
        let progress = std::sync::Arc::new(Progress::default());
        let fake = Fake::answering(Attempt::Done(
            "So I think we should meet on Wednesday.".into(),
        ));
        *fake.progress.lock().unwrap() = Some(progress.clone());
        run(
            Some(&fake),
            RULE,
            Duration::from_secs(3),
            &CancelToken::default(),
            &progress,
        );
        assert_eq!(
            *fake.phase_during_clean.lock().unwrap(),
            Some(Phase::Cleaning)
        );
        assert_eq!(progress.get(), None);
    }

    #[test]
    fn a_poem_is_rejected_by_the_guard() {
        let fake = Fake::answering(Attempt::Done("Soft paws on moonlit halls.".into()));
        let record = go(&fake).unwrap();
        assert_eq!(record.outcome, Outcome::GuardRejected);
        assert_eq!(record.text, None);
        assert_eq!(record.code, "LLM_GUARD_REJECTED");
    }

    #[test]
    fn empty_output_is_rejected_by_the_guard() {
        let record = go(&Fake::answering(Attempt::Done("  ".into()))).unwrap();
        assert_eq!(record.outcome, Outcome::GuardRejected);
    }

    #[test]
    fn a_timeout_gives_the_timeout_outcome() {
        let record = go(&Fake::answering(Attempt::TimedOut)).unwrap();
        assert_eq!(record.outcome, Outcome::Timeout);
        assert_eq!(record.code, "LLM_TIMEOUT");
    }

    #[test]
    fn a_crash_gives_the_error_outcome_with_its_code() {
        let record = go(&Fake::answering(Attempt::Failed(LLM_CRASHED.into()))).unwrap();
        assert_eq!(record.outcome, Outcome::Error);
        assert_eq!(record.code, "LLM_CRASHED");
    }

    #[test]
    fn a_failed_load_gives_an_error_and_no_job_is_run() {
        let mut fake = Fake::answering(Attempt::Done("never".into()));
        fake.prepare = Err(LLM_LOAD_FAILED.into());
        let record = go(&fake).unwrap();
        assert_eq!(record.outcome, Outcome::Error);
        assert_eq!(record.code, "LLM_LOAD_FAILED");
        assert!(fake.seen.lock().unwrap().is_empty());
    }

    #[test]
    fn a_worker_that_is_down_gives_unavailable() {
        let mut fake = Fake::answering(Attempt::Done("never".into()));
        fake.prepare = Err(LLM_UNAVAILABLE.into());
        let record = go(&fake).unwrap();
        assert_eq!(record.outcome, Outcome::Unavailable);
        assert_eq!(record.code, "LLM_UNAVAILABLE");
    }

    #[test]
    fn no_worker_at_all_gives_unavailable() {
        let record = run(
            None,
            RULE,
            Duration::from_secs(3),
            &CancelToken::default(),
            &Progress::default(),
        )
        .unwrap();
        assert_eq!(record.outcome, Outcome::Unavailable);
        assert_eq!(record.code, "LLM_UNAVAILABLE");
    }

    #[test]
    fn a_cancelled_step_gives_no_record() {
        let token = CancelToken::default();
        token.cancel();
        let fake = Fake::answering(Attempt::Done("never".into()));
        let progress = Progress::default();
        assert_eq!(
            run(Some(&fake), RULE, Duration::from_secs(3), &token, &progress),
            None
        );
        assert!(fake.seen.lock().unwrap().is_empty());
        assert_eq!(progress.get(), None);
        assert_eq!(
            run(
                Some(&Fake::answering(Attempt::Cancelled)),
                RULE,
                Duration::from_secs(3),
                &CancelToken::default(),
                &progress
            ),
            None
        );
    }

    #[test]
    fn the_provider_setting_picks_the_cleaner() {
        use crate::net::endpoint::UreqTransport;
        use serde_json::json;
        let tmp = tempfile::tempdir().unwrap();
        let data = hushpen_store::data_dir::DataDir::open(tmp.path().join("data")).unwrap();
        let storage = crate::storage::open(data).unwrap();
        let host = LlmHost::with_program(&storage.data, std::path::PathBuf::from("/bin/false"));
        let transport: Arc<dyn Transport> = Arc::new(UreqTransport);

        assert!(cleaner_for(&storage, &host, &transport).is_none());

        storage
            .settings
            .set("cleanup.llm.provider", json!("endpoint"))
            .unwrap();
        let cleaner =
            cleaner_for(&storage, &host, &transport).expect("the endpoint needs no worker");
        // No URL is set, so the request is refused before any socket opens.
        let attempt = cleaner.clean("hello", Duration::from_secs(1), &CancelToken::default());
        assert_eq!(attempt, Attempt::Failed("ENDPOINT_FAILED".into()));
    }

    #[test]
    fn the_token_budget_follows_the_text_length_within_bounds() {
        assert_eq!(max_tokens("hi"), 32);
        assert_eq!(max_tokens(&"a".repeat(400)), 320);
        assert_eq!(max_tokens(&"a".repeat(100_000)), 1024);
    }
}
