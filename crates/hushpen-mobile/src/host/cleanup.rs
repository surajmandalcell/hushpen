use crate::{SystemCleanup, lock};
use hushpen_core::error;
use hushpen_core::llm_protocol::{
    JobStats, LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest, UnloadReason,
};
use hushpen_core::protocol::{read_frame, write_json};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;

type Output = Arc<Mutex<Box<dyn Write + Send>>>;

struct Pending {
    job: u64,
    cancelled: bool,
}

fn send(output: &Output, event: LlmEvent) {
    let _ = write_json(&mut *lock(output), &event.to_json());
}

pub(super) fn serve(
    mut input: Box<dyn Read + Send>,
    output: Box<dyn Write + Send>,
    system: Arc<SystemCleanup>,
) {
    let output = Arc::new(Mutex::new(output));
    let pending = Arc::new(Mutex::new(None::<Pending>));
    let mut loaded = false;
    while let Ok(Some(frame)) = read_frame(&mut input) {
        let request = match LlmRequest::from_frame(&frame) {
            Ok(request) => request,
            Err(_) => break,
        };
        match request {
            LlmRequest::Hello { protocol } if protocol == LLM_PROTOCOL_VERSION => send(
                &output,
                LlmEvent::Ready {
                    version: env!("CARGO_PKG_VERSION").into(),
                    protocol,
                    backend: "system".into(),
                },
            ),
            LlmRequest::Hello { .. } | LlmRequest::Shutdown => break,
            LlmRequest::Ping => send(&output, LlmEvent::Pong),
            LlmRequest::LoadModel { path, .. } => {
                loaded = system.ready_for_model(&path);
                send(
                    &output,
                    if loaded {
                        LlmEvent::Loaded {
                            model: "system".into(),
                            ms: 0,
                            gpu: false,
                            backend: "system".into(),
                        }
                    } else {
                        LlmEvent::error(
                            None,
                            error::LLM_LOAD_FAILED,
                            "system cleanup is unavailable",
                        )
                    },
                );
            }
            LlmRequest::Unload => {
                loaded = false;
                send(
                    &output,
                    LlmEvent::Unloaded {
                        reason: UnloadReason::Requested,
                    },
                );
            }
            LlmRequest::Cancel { job } => {
                if let Some(active) = lock(&pending).as_mut()
                    && active.job == job
                    && !active.cancelled
                {
                    active.cancelled = true;
                    send(&output, LlmEvent::Cancelled { job });
                }
            }
            LlmRequest::Command { job, .. } => send(
                &output,
                LlmEvent::error(
                    Some(job),
                    error::LLM_UNAVAILABLE,
                    "system cleanup does not support Command Mode",
                ),
            ),
            LlmRequest::Cleanup {
                job,
                system: instructions,
                user,
                ..
            } => {
                if !loaded {
                    send(
                        &output,
                        LlmEvent::error(
                            Some(job),
                            error::LLM_UNAVAILABLE,
                            "system cleanup is not loaded",
                        ),
                    );
                    continue;
                }
                let mut active = lock(&pending);
                if active.is_some() {
                    send(
                        &output,
                        LlmEvent::error(
                            Some(job),
                            error::LLM_UNAVAILABLE,
                            "system cleanup is busy",
                        ),
                    );
                    continue;
                }
                if user.len() > 1024 * 1024 || instructions.len() > 1024 * 1024 {
                    send(
                        &output,
                        LlmEvent::error(
                            Some(job),
                            error::LLM_UNAVAILABLE,
                            "cleanup text or instructions exceed 1 MiB",
                        ),
                    );
                    continue;
                }
                *active = Some(Pending {
                    job,
                    cancelled: false,
                });
                drop(active);
                let pending = Arc::clone(&pending);
                let output = Arc::clone(&output);
                let system = Arc::clone(&system);
                std::thread::spawn(move || {
                    let started = Instant::now();
                    let result = system.clean_with_instructions(user, instructions);
                    let mut active = lock(&pending);
                    if active.as_ref().is_some_and(|active| !active.cancelled) {
                        send(
                            &output,
                            match result {
                                Ok(result) => LlmEvent::Result {
                                    job,
                                    text: result.text,
                                    stats: JobStats {
                                        ms: started.elapsed().as_millis() as u64,
                                        ..JobStats::default()
                                    },
                                },
                                Err(_) => LlmEvent::error(
                                    Some(job),
                                    error::LLM_UNAVAILABLE,
                                    "system cleanup failed",
                                ),
                            },
                        );
                    }
                    *active = None;
                });
            }
        }
    }
    if let Some(active) = lock(&pending).as_mut() {
        active.cancelled = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MobileError, SystemLlm};
    use hushpen_engine::{
        EngineLog, InProcess, Link, LlmClient, LlmOutcome, LlmSpec, LlmState, LlmWork, Timing,
    };
    use std::sync::mpsc;
    use std::time::Duration;

    struct System;
    impl SystemLlm for System {
        fn available(&self) -> bool {
            true
        }
        fn cleanup(&self, text: String, _: String) -> Result<String, MobileError> {
            Ok(format!("{}!", text.trim_end_matches('.')))
        }
    }

    fn client(system: Arc<SystemCleanup>) -> LlmClient {
        let client = LlmClient::start_link(
            Link::InProcess(InProcess::new(move |input, output| {
                serve(input, output, Arc::clone(&system))
            })),
            Timing::default(),
            EngineLog::discard(),
        );
        assert!(
            client.wait_for(Duration::from_secs(2), |status| status.state
                == LlmState::Ready)
        );
        assert!(
            client
                .load_model(LlmSpec::new("system".into()))
                .wait_timeout(Duration::from_secs(2))
                .unwrap()
                .is_ok()
        );
        client
    }

    fn work() -> LlmWork {
        LlmWork::Cleanup {
            system: "Clean this text".into(),
            user: "Hello there.".into(),
            max_tokens: 32,
        }
    }

    #[test]
    fn callback_cleanup_uses_the_existing_worker_protocol() {
        let client = client(SystemCleanup::new(Arc::new(System)));
        match client
            .run(work())
            .wait_timeout(Duration::from_secs(2))
            .unwrap()
        {
            LlmOutcome::Done(done) => assert_eq!(done.text, "Hello there!"),
            other => panic!("expected callback output, got {other:?}"),
        }
        assert!(client.status().pid.is_none());
        client.shutdown();
    }

    #[test]
    fn hosted_cleanup_forwards_the_desktop_prompt_and_rule_text() {
        struct Capture(mpsc::SyncSender<(String, String)>);
        impl SystemLlm for Capture {
            fn available(&self) -> bool {
                true
            }
            fn cleanup(&self, text: String, instructions: String) -> Result<String, MobileError> {
                self.0.send((text.clone(), instructions)).unwrap();
                Ok(text)
            }
        }
        let (sent, received) = mpsc::sync_channel(1);
        let client = client(SystemCleanup::new(Arc::new(Capture(sent))));
        let instructions = hushpen_core::llm_prompt::cleanup_system_keeping(&["Zorvath".into()]);
        let user = "hello full stop";
        let outcome = client
            .run(LlmWork::Cleanup {
                system: instructions.clone(),
                user: user.into(),
                max_tokens: 32,
            })
            .wait_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(matches!(outcome, LlmOutcome::Done(done) if done.text == user));
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            (user.into(), instructions)
        );
        client.shutdown();
    }

    struct Slow {
        started: mpsc::SyncSender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl SystemLlm for Slow {
        fn available(&self) -> bool {
            true
        }
        fn cleanup(&self, text: String, _: String) -> Result<String, MobileError> {
            self.started.send(()).unwrap();
            lock(&self.release)
                .recv_timeout(Duration::from_secs(3))
                .unwrap();
            Ok(text)
        }
    }

    #[test]
    fn cancel_settles_while_the_native_callback_is_still_running() {
        let (started, observed) = mpsc::sync_channel(1);
        let (release, finish) = mpsc::sync_channel(1);
        let client = client(SystemCleanup::new(Arc::new(Slow {
            started,
            release: Mutex::new(finish),
        })));
        let job = client.run(work());
        observed.recv_timeout(Duration::from_secs(2)).unwrap();
        job.cancel();
        assert!(matches!(
            job.wait_timeout(Duration::from_secs(1)),
            Some(LlmOutcome::Cancelled)
        ));
        release.send(()).unwrap();
        client.shutdown();
    }
}
