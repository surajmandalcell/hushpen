//! The cleanup step that asks the user's OpenAI-compatible endpoint instead of the local model.

use super::{Attempt, Cleaner, Progress, max_tokens};
use crate::controller::CancelToken;
use crate::net::endpoint::{Config, Kind, Transport};
use crate::storage::Storage;
use hushpen_core::endpoint::{cleanup_system, transcript_user};
use hushpen_core::error::ENDPOINT_FAILED;
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant};

/// How often a waiting step looks at the cancel token.
const POLL: Duration = Duration::from_millis(50);
/// The transport enforces the timeout itself. This is the margin after which the step stops
/// waiting for a transport that did not.
const GRACE: Duration = Duration::from_millis(500);

pub struct EndpointCleaner {
    transport: Arc<dyn Transport>,
    config: Config,
}

impl EndpointCleaner {
    pub fn new(transport: Arc<dyn Transport>, config: Config) -> Self {
        Self { transport, config }
    }

    /// The endpoint as the settings and the secrets file hold it now.
    pub fn from_storage(storage: &Storage, transport: Arc<dyn Transport>) -> Self {
        let text = |key: &str| {
            storage
                .settings
                .get(key)
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        Self::new(
            transport,
            Config {
                url: text("cleanup.llm.endpointUrl"),
                model: text("cleanup.llm.endpointModel"),
                key: storage.secrets.endpoint_api_key(),
                timeout: Duration::ZERO,
            },
        )
    }
}

impl Cleaner for EndpointCleaner {
    fn prepare(&self, _: &CancelToken, _: &Progress) -> Result<(), String> {
        Ok(())
    }

    fn clean(&self, text: &str, timeout: Duration, token: &CancelToken) -> Attempt {
        let (sender, receiver) = channel();
        let transport = Arc::clone(&self.transport);
        let config = Config {
            timeout,
            ..self.config.clone()
        };
        let user = transcript_user(text);
        let tokens = max_tokens(text);
        let began = Instant::now();
        let spawned = std::thread::Builder::new()
            .name("endpoint-cleanup".into())
            .spawn(move || {
                let _ = sender.send(transport.complete(&config, &cleanup_system(), &user, tokens));
            });
        if spawned.is_err() {
            return Attempt::Failed(ENDPOINT_FAILED.to_owned());
        }
        loop {
            match receiver.recv_timeout(POLL) {
                Ok(Ok(reply)) => return Attempt::Done(reply),
                Ok(Err(failure)) if failure.kind == Kind::Timeout => return Attempt::TimedOut,
                Ok(Err(failure)) => return Attempt::Failed(failure.code().to_owned()),
                Err(RecvTimeoutError::Disconnected) => {
                    return Attempt::Failed(ENDPOINT_FAILED.to_owned());
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            if token.is_cancelled() {
                return Attempt::Cancelled;
            }
            if began.elapsed() >= timeout + GRACE {
                return Attempt::TimedOut;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::endpoint::Failure;
    use std::sync::Mutex;

    struct Scripted {
        answer: Mutex<Option<Result<String, Failure>>>,
        delay: Duration,
        seen: Mutex<Vec<(Config, String, String, u32)>>,
    }

    impl Scripted {
        fn new(answer: Result<String, Failure>, delay: Duration) -> Arc<Self> {
            Arc::new(Self {
                answer: Mutex::new(Some(answer)),
                delay,
                seen: Mutex::default(),
            })
        }
    }

    impl Transport for Scripted {
        fn test_connection(&self, _: &Config) -> Result<(), Failure> {
            unreachable!("a cleanup never tests the connection")
        }

        fn complete(
            &self,
            config: &Config,
            system: &str,
            user: &str,
            max_tokens: u32,
        ) -> Result<String, Failure> {
            self.seen.lock().unwrap().push((
                config.clone(),
                system.to_owned(),
                user.to_owned(),
                max_tokens,
            ));
            std::thread::sleep(self.delay);
            self.answer.lock().unwrap().take().unwrap()
        }
    }

    fn failure(kind: Kind) -> Failure {
        Failure {
            kind,
            host: "localhost".into(),
        }
    }

    fn cleaner(transport: &Arc<Scripted>) -> EndpointCleaner {
        EndpointCleaner::new(
            transport.clone(),
            Config {
                url: "http://localhost:11434/v1".into(),
                model: "stub-model".into(),
                key: Some("sk-test-123".into()),
                timeout: Duration::ZERO,
            },
        )
    }

    #[test]
    fn the_request_carries_the_wrapped_text_the_timeout_and_the_key() {
        let transport = Scripted::new(Ok("Hello.".into()), Duration::ZERO);
        let attempt = cleaner(&transport).clean(
            "a <b> c",
            Duration::from_millis(6_000),
            &CancelToken::default(),
        );
        assert_eq!(attempt, Attempt::Done("Hello.".into()));
        let seen = transport.seen.lock().unwrap();
        let (config, system, user, tokens) = &seen[0];
        assert_eq!(config.model, "stub-model");
        assert_eq!(config.key.as_deref(), Some("sk-test-123"));
        assert_eq!(config.timeout, Duration::from_millis(6_000));
        assert_eq!(system, &cleanup_system());
        assert_eq!(user, "<transcript>a &lt;b&gt; c</transcript>");
        assert_eq!(*tokens, max_tokens("a <b> c"));
    }

    #[test]
    fn a_timeout_from_the_transport_is_a_timeout() {
        let transport = Scripted::new(Err(failure(Kind::Timeout)), Duration::ZERO);
        let attempt =
            cleaner(&transport).clean("text", Duration::from_secs(6), &CancelToken::default());
        assert_eq!(attempt, Attempt::TimedOut);
    }

    #[test]
    fn every_other_failure_carries_the_endpoint_code() {
        for kind in [Kind::Down, Kind::Status(500), Kind::BadReply] {
            let transport = Scripted::new(Err(failure(kind)), Duration::ZERO);
            let attempt =
                cleaner(&transport).clean("text", Duration::from_secs(6), &CancelToken::default());
            assert_eq!(
                attempt,
                Attempt::Failed("ENDPOINT_FAILED".into()),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn a_transport_that_ignores_its_timeout_is_given_up_on() {
        let transport = Scripted::new(Ok("late".into()), Duration::from_secs(3));
        let began = Instant::now();
        let attempt =
            cleaner(&transport).clean("text", Duration::from_millis(200), &CancelToken::default());
        assert_eq!(attempt, Attempt::TimedOut);
        assert!(began.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_cancel_ends_the_wait_at_once() {
        let transport = Scripted::new(Ok("late".into()), Duration::from_secs(3));
        let token = CancelToken::default();
        token.cancel();
        let began = Instant::now();
        let attempt = cleaner(&transport).clean("text", Duration::from_secs(6), &token);
        assert_eq!(attempt, Attempt::Cancelled);
        assert!(began.elapsed() < Duration::from_secs(1));
    }
}
