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
use hushpen_core::llm_prompt::{Profile, PromptFamily, Target, Tuned, cleanup_prompt};
use hushpen_engine::{LlmClient, LlmLoaded, LlmOutcome, LlmSpec, LlmState, LlmWork};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// The profile the cleanup uses, by key.
pub const PROFILE_SETTING: &str = "cleanup.profile";
/// The user's changes to the profiles: `<profile>.<field>` to the new value.
pub const PROFILE_EDITS_SETTING: &str = "cleanup.profileEdits";

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

    /// Cleans `text`. `keep` are dictionary words in it that the model must not change. The
    /// `timeout` counts from the moment the job is sent.
    fn clean(&self, text: &str, keep: &[String], timeout: Duration, token: &CancelToken)
    -> Attempt;

    /// Command Mode: edits `selection` as the spoken `instruction` says.
    fn edit(
        &self,
        instruction: &str,
        selection: &str,
        timeout: Duration,
        token: &CancelToken,
    ) -> Attempt;
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

/// Runs the AI step for `rule`, the text the rules produced. `keep` are the dictionary words
/// that the rule text has: an answer that drops one is rejected. A `None` result means the user
/// cancelled and nothing should be inserted.
pub fn run(
    cleaner: Option<&dyn Cleaner>,
    rule: &str,
    keep: &[String],
    timeout: Duration,
    token: &CancelToken,
    progress: &Progress,
) -> Option<Record> {
    step(
        cleaner,
        token,
        progress,
        |cleaner| cleaner.clean(rule, keep, timeout, token),
        |text| guard::check_keeping(rule, text, keep),
    )
}

/// Runs the model on a selection for Command Mode. The answer is checked against the selection,
/// and every outcome but `Used` means the selection stays as it is.
pub fn run_edit(
    cleaner: Option<&dyn Cleaner>,
    instruction: &str,
    selection: &str,
    timeout: Duration,
    token: &CancelToken,
    progress: &Progress,
) -> Option<Record> {
    step(
        cleaner,
        token,
        progress,
        |cleaner| cleaner.edit(instruction, selection, timeout, token),
        |text| guard::check_command(selection, text),
    )
}

fn step(
    cleaner: Option<&dyn Cleaner>,
    token: &CancelToken,
    progress: &Progress,
    job: impl FnOnce(&dyn Cleaner) -> Attempt,
    check: impl FnOnce(&str) -> Result<String, guard::Rejection>,
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
    let attempt = job(cleaner);
    progress.set(None);
    match attempt {
        Attempt::Cancelled => None,
        Attempt::TimedOut => Some(Record::failed(Outcome::Timeout, None, began)),
        Attempt::Failed(code) => Some(Record::failed(
            Outcome::of_failure(&code),
            Some(&code),
            began,
        )),
        Attempt::Done(text) => Some(match check(&text) {
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

/// How many tokens an edit of `selection` may take: the edit can be longer than the text, so
/// the limit is twice that of a cleanup, up to the same ceiling.
fn edit_max_tokens(selection: &str) -> u32 {
    max_tokens(selection)
        .saturating_mul(2)
        .min(MAX_TOKENS as u32)
}

/// The chosen profile with the user's changes.
pub fn profile(storage: &Storage) -> Tuned {
    let chosen = storage
        .settings
        .get(PROFILE_SETTING)
        .and_then(|value| value.as_str().and_then(Profile::parse))
        .unwrap_or(Profile::General);
    profile_of(storage, chosen)
}

/// `profile` with the user's changes.
pub fn profile_of(storage: &Storage, profile: Profile) -> Tuned {
    let edits = storage.settings.get(PROFILE_EDITS_SETTING);
    Tuned::from_edits(profile, |key| {
        edits
            .as_ref()
            .and_then(|edits| edits.get(key))
            .and_then(|value| value.as_str().map(str::to_owned))
    })
}

/// The stored changes with `tuned` in place of what they hold for its profile. A field with the
/// built-in value is dropped, so a reset profile leaves nothing behind.
pub fn edits_with(storage: &Storage, tuned: &Tuned) -> serde_json::Value {
    let mut edits = storage
        .settings
        .get(PROFILE_EDITS_SETTING)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    for (key, value) in tuned.edits() {
        match value {
            Some(value) => {
                edits.insert(key, serde_json::Value::String(value));
            }
            None => {
                edits.remove(&key);
            }
        }
    }
    serde_json::Value::Object(edits)
}

/// The prompt family of a local cleanup model, by its catalog id or file name.
pub fn family_of(model: &str) -> PromptFamily {
    let name = if model.ends_with(".gguf") {
        model.to_owned()
    } else {
        format!("{model}.gguf")
    };
    PromptFamily::from_model_name(&name).unwrap_or(PromptFamily::Qwen35)
}

/// The cleaner for the provider that the settings name, read at the start of each dictation so
/// a change of provider or profile takes effect on the next one. `None` means the local worker
/// is not running.
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
    HostCleaner::of(host, profile(storage)).map(|cleaner| Box::new(cleaner) as Box<dyn Cleaner>)
}

/// The worker behind [`LlmHost`], with the load that brings its model back after an idle unload.
pub struct HostCleaner {
    client: LlmClient,
    spec: LlmSpec,
    profile: Tuned,
}

impl HostCleaner {
    /// `None` while AI cleanup is off or has no verified model, so no worker runs.
    pub fn of(host: &LlmHost, profile: Tuned) -> Option<Self> {
        let (client, spec) = host.running()?;
        Some(Self {
            client,
            spec,
            profile,
        })
    }

    fn family(&self) -> PromptFamily {
        self.spec
            .family
            .as_deref()
            .and_then(PromptFamily::parse)
            .unwrap_or_else(|| {
                let name = self.spec.path.file_name().unwrap_or_default();
                family_of(&name.to_string_lossy())
            })
    }

    /// The job of a cleanup: the prompt the Prompt sheet shows for this model and profile.
    fn cleanup_work(&self, text: &str, keep: &[String]) -> LlmWork {
        let prompt = cleanup_prompt(Target::Local(self.family()), &self.profile, keep);
        LlmWork::Cleanup {
            system: prompt.job_system(),
            user: text.to_owned(),
            max_tokens: max_tokens(text),
        }
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
        if token.is_cancelled() {
            return Ok(());
        }
        let status = self.client.status();
        if status.model.is_some() {
            return Ok(());
        }
        match status.state {
            // The worker dropped the model after the idle time. Loading it here keeps the load
            // out of the cleanup timeout.
            LlmState::Ready => self.load().map(drop),
            // A first load can take a minute (a Metal shader compile). The dictation takes the
            // rule text now; the supervisor keeps loading, and the next dictation uses the model.
            LlmState::Starting | LlmState::Loading | LlmState::Backoff | LlmState::Stopped => {
                Err(LLM_UNAVAILABLE.to_owned())
            }
        }
    }
    fn clean(
        &self,
        text: &str,
        keep: &[String],
        timeout: Duration,
        token: &CancelToken,
    ) -> Attempt {
        self.work(self.cleanup_work(text, keep), timeout, token)
    }

    fn edit(
        &self,
        instruction: &str,
        selection: &str,
        timeout: Duration,
        token: &CancelToken,
    ) -> Attempt {
        self.work(
            LlmWork::Command {
                instruction: instruction.to_owned(),
                selection: selection.to_owned(),
                max_tokens: edit_max_tokens(selection),
            },
            timeout,
            token,
        )
    }
}

impl HostCleaner {
    fn work(&self, work: LlmWork, timeout: Duration, token: &CancelToken) -> Attempt {
        let handle = self.client.run(work);
        let id = handle.id();
        let client = self.client.clone();
        token.on_cancel(move || client.cancel(id));
        match handle.wait_timeout(timeout) {
            Some(LlmOutcome::Done(done)) => Attempt::Done(done.text),
            Some(LlmOutcome::Cancelled) => Attempt::Cancelled,
            Some(LlmOutcome::Failed(failure)) => Attempt::Failed(failure.code),
            None => {
                // The old text stays now; the worker settles the cancel on its own time.
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
        kept: Mutex<Vec<String>>,
        edits: Mutex<Vec<(String, String)>>,
        phase_during_clean: Mutex<Option<Phase>>,
        progress: Mutex<Option<std::sync::Arc<Progress>>>,
    }

    impl Fake {
        fn answering(attempt: Attempt) -> Self {
            Self {
                prepare: Ok(()),
                attempt,
                seen: Mutex::default(),
                kept: Mutex::default(),
                edits: Mutex::default(),
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

        fn clean(&self, text: &str, keep: &[String], _: Duration, _: &CancelToken) -> Attempt {
            self.seen.lock().unwrap().push(text.to_owned());
            *self.kept.lock().unwrap() = keep.to_vec();
            if let Some(progress) = &*self.progress.lock().unwrap() {
                *self.phase_during_clean.lock().unwrap() = progress.get();
            }
            self.attempt.clone()
        }

        fn edit(
            &self,
            instruction: &str,
            selection: &str,
            _: Duration,
            _: &CancelToken,
        ) -> Attempt {
            self.edits
                .lock()
                .unwrap()
                .push((instruction.to_owned(), selection.to_owned()));
            self.attempt.clone()
        }
    }

    /// The cleanup budget of every test call; the fakes answer at once, so it never runs out.
    const TIMEOUT: Duration = Duration::from_secs(3);

    const RULE: &str = "So I think we should meet on Tuesday, no wait, Wednesday at three PM.";

    fn go(fake: &Fake) -> Option<Record> {
        run(
            Some(fake),
            RULE,
            &[],
            TIMEOUT,
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
            &[],
            TIMEOUT,
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
            &[],
            TIMEOUT,
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
            run(Some(&fake), RULE, &[], TIMEOUT, &token, &progress),
            None
        );
        assert!(fake.seen.lock().unwrap().is_empty());
        assert_eq!(progress.get(), None);
        assert_eq!(
            run(
                Some(&Fake::answering(Attempt::Cancelled)),
                RULE,
                &[],
                TIMEOUT,
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
        let attempt = cleaner.clean("hello", &[], TIMEOUT, &CancelToken::default());
        assert_eq!(attempt, Attempt::Failed("ENDPOINT_FAILED".into()));
    }

    #[test]
    fn an_answer_that_drops_a_dictionary_word_is_rejected_and_the_word_reaches_the_model() {
        let fake = Fake::answering(Attempt::Done("Meet on Wednesday at three.".into()));
        let rule = "Meet on Wednesbury at three.";
        let keep = ["Wednesbury".to_owned()];
        let record = run(
            Some(&fake),
            rule,
            &keep,
            TIMEOUT,
            &CancelToken::default(),
            &Progress::default(),
        )
        .unwrap();
        assert_eq!(record.outcome, Outcome::GuardRejected);
        assert_eq!(record.text, None);
        assert_eq!(*fake.kept.lock().unwrap(), keep);
        let kept = Fake::answering(Attempt::Done("Meet on Wednesbury at 3.".into()));
        let record = run(
            Some(&kept),
            rule,
            &keep,
            TIMEOUT,
            &CancelToken::default(),
            &Progress::default(),
        )
        .unwrap();
        assert_eq!(record.outcome, Outcome::Used);
    }

    fn edit(fake: &Fake) -> Option<Record> {
        run_edit(
            Some(fake),
            "make this formal",
            "hey can you send me the file",
            TIMEOUT,
            &CancelToken::default(),
            &Progress::default(),
        )
    }

    #[test]
    fn an_edit_gives_the_instruction_and_the_selection_to_the_model() {
        let fake = Fake::answering(Attempt::Done("Could you please send me the file?".into()));
        let record = edit(&fake).unwrap();
        assert_eq!(record.outcome, Outcome::Used);
        assert_eq!(
            record.text.as_deref(),
            Some("Could you please send me the file?")
        );
        assert_eq!(
            *fake.edits.lock().unwrap(),
            [(
                "make this formal".to_owned(),
                "hey can you send me the file".to_owned()
            )]
        );
    }

    #[test]
    fn an_edit_that_fails_or_is_refused_has_no_text() {
        for (attempt, outcome) in [
            (Attempt::TimedOut, Outcome::Timeout),
            (Attempt::Failed(LLM_CRASHED.into()), Outcome::Error),
            (Attempt::Done("  ".into()), Outcome::GuardRejected),
            (
                Attempt::Done("I can't help with that.".into()),
                Outcome::GuardRejected,
            ),
        ] {
            let record = edit(&Fake::answering(attempt)).unwrap();
            assert_eq!(record.outcome, outcome);
            assert_eq!(record.text, None);
        }
    }

    #[test]
    fn a_cancelled_edit_gives_no_record() {
        assert_eq!(edit(&Fake::answering(Attempt::Cancelled)), None);
    }

    #[test]
    fn the_edit_budget_is_twice_the_cleanup_budget_up_to_the_ceiling() {
        assert_eq!(edit_max_tokens("hi"), 64);
        assert_eq!(edit_max_tokens(&"a".repeat(100_000)), 1024);
    }

    #[test]
    fn a_dictation_during_the_model_load_does_not_wait_for_it() {
        use hushpen_engine::{ChildSpec, EngineLog, Timing};
        // A worker that never says ready stays in the starting state for the whole test.
        let client = LlmClient::start(
            ChildSpec {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "sleep 30".into()],
            },
            Timing::llm(),
            EngineLog::discard(),
        );
        assert!(client.wait_for(Duration::from_secs(5), |s| s.pid.is_some()));
        let cleaner = HostCleaner {
            client: client.clone(),
            spec: LlmSpec::new("/nonexistent/model.gguf".into()),
            profile: Tuned::built_in(Profile::General),
        };
        let (done, answer) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(cleaner.prepare(&CancelToken::default(), &Progress::default()));
        });
        let prepared = answer.recv_timeout(Duration::from_secs(2));
        client.shutdown();
        assert_eq!(
            prepared.expect("prepare returns at once while the model loads"),
            Err(LLM_UNAVAILABLE.to_owned())
        );
    }

    fn idle_cleaner(model: &str, profile: Tuned) -> (LlmClient, HostCleaner) {
        use hushpen_engine::{ChildSpec, EngineLog, Timing};
        let client = LlmClient::start(
            ChildSpec {
                program: "/bin/sh".into(),
                args: vec!["-c".into(), "sleep 30".into()],
            },
            Timing::llm(),
            EngineLog::discard(),
        );
        let cleaner = HostCleaner {
            client: client.clone(),
            spec: LlmSpec::new(std::path::PathBuf::from("/models").join(model)),
            profile,
        };
        (client, cleaner)
    }

    fn job_system(work: LlmWork) -> String {
        match work {
            LlmWork::Cleanup { system, .. } => system,
            other => panic!("expected a cleanup, got {other:?}"),
        }
    }

    #[test]
    fn s1_mini_gets_the_control_line_the_prompt_sheet_shows() {
        let mut tuned = Tuned::built_in(Profile::Notes);
        tuned.controls.styling = hushpen_core::llm_prompt::Styling::Casual;
        let (client, cleaner) = idle_cleaner("s1-mini-q4_k_m.gguf", tuned.clone());
        let keep = ["Wednesbury".to_owned()];
        let system = job_system(cleaner.cleanup_work("um hi", &keep));
        client.shutdown();
        let shown = cleanup_prompt(Target::Local(PromptFamily::S1Mini), &tuned, &keep);
        assert_eq!(system, shown.job_system());
        assert_eq!(
            system,
            "[Styling: casual] [Structure: lists] [Context: general]"
        );
        assert_eq!(
            PromptFamily::S1Mini.cleanup_prefix(&system),
            format!("{}{system}\n", PromptFamily::S1Mini.prefix(&shown.system()))
        );
    }

    #[test]
    fn qwen_gets_the_profile_prompt_the_prompt_sheet_shows() {
        let mut tuned = Tuned::built_in(Profile::Email);
        tuned.prompt = "Clean the dictation my way.".into();
        let (client, cleaner) = idle_cleaner("Qwen3.5-2B-Q4_K_M.gguf", tuned.clone());
        let keep = ["Zorvath".to_owned()];
        let system = job_system(cleaner.cleanup_work("um hi Zorvath", &keep));
        client.shutdown();
        let shown = cleanup_prompt(Target::Local(PromptFamily::Qwen35), &tuned, &keep);
        assert_eq!(system, shown.system());
        assert!(system.starts_with("Clean the dictation my way. These words"));
    }

    #[test]
    fn the_stored_profile_and_its_edits_are_read_from_the_settings() {
        use serde_json::json;
        let tmp = tempfile::tempdir().unwrap();
        let data = hushpen_store::data_dir::DataDir::open(tmp.path().join("data")).unwrap();
        let storage = crate::storage::open(data).unwrap();
        assert_eq!(profile(&storage), Tuned::built_in(Profile::General));
        storage
            .settings
            .set(PROFILE_SETTING, json!("message"))
            .unwrap();
        storage
            .settings
            .set(
                PROFILE_EDITS_SETTING,
                json!({"message.styling": "casual", "general.prompt": "Other."}),
            )
            .unwrap();
        let tuned = profile(&storage);
        assert_eq!(tuned.profile, Profile::Message);
        assert_eq!(
            tuned.controls.styling,
            hushpen_core::llm_prompt::Styling::Casual
        );
        assert_eq!(tuned.prompt, Profile::Message.prompt());
        assert_eq!(profile_of(&storage, Profile::General).prompt, "Other.");
        assert_eq!(family_of("s1-mini-q4_k_m"), PromptFamily::S1Mini);
        assert_eq!(family_of("qwen3.5-2b"), PromptFamily::Qwen35);
    }

    #[test]
    fn the_token_budget_follows_the_text_length_within_bounds() {
        assert_eq!(max_tokens("hi"), 32);
        assert_eq!(max_tokens(&"a".repeat(400)), 320);
        assert_eq!(max_tokens(&"a".repeat(100_000)), 1024);
    }
}
