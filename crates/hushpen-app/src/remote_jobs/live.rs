use super::{CancelToken, EngineClient, Failure, JOB_TIMEOUT, JobOutcome, Routing, TranscribeSpec};
use crate::models::Spawner;
use hushpen_core::error::ENGINE_UNAVAILABLE;
use hushpen_engine::{JobHandle, LoadSpec, PartialSink};
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

pub(crate) enum LiveJob {
    Local(JobHandle),
    Routed {
        closed: Arc<AtomicBool>,
        answer: Receiver<JobOutcome>,
    },
}

impl LiveJob {
    pub fn finish(self, token: &CancelToken) -> JobOutcome {
        match self {
            Self::Local(job) => {
                job.finish_live();
                job.wait()
            }
            Self::Routed { closed, answer } => {
                closed.store(true, Ordering::Release);
                answer
                    .recv_timeout(super::LOAD_TIMEOUT + JOB_TIMEOUT)
                    .unwrap_or_else(|_| {
                        token.cancel();
                        unavailable("The paired speech job did not finish.")
                    })
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Capture {
    closed: Arc<AtomicBool>,
    until: Instant,
    partial: PartialSink,
}

impl Capture {
    pub fn new(partial: PartialSink) -> Self {
        Self {
            closed: Arc::new(AtomicBool::new(false)),
            until: Instant::now() + Duration::from_secs(61 * 60),
            partial,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start(
        self,
        route: Routing,
        load: LoadSpec,
        spec: TranscribeSpec,
        local: Option<EngineClient>,
        token: Arc<CancelToken>,
        notice: impl FnMut(String) + Send + 'static,
        spawn: &Spawner,
    ) -> io::Result<LiveJob> {
        let (reply, answer) = mpsc::channel();
        let closed = Arc::clone(&self.closed);
        spawn(Box::new(move || {
            let local_capture = self.clone();
            let local = Arc::new(move |spec: TranscribeSpec, token: Arc<CancelToken>| {
                let Some(client) = &local else {
                    return unavailable(
                        "The selected speech model is unavailable on this computer.",
                    );
                };
                local_capture
                    .decode(client, &spec, &token, true)
                    .unwrap_or_else(|| unavailable("The local speech job timed out."))
            });
            let result = route.speech(load, spec, token, local, Some(&self), notice);
            let _ = reply.send(result);
        }))?;
        Ok(LiveJob::Routed { closed, answer })
    }

    pub(super) fn decode(
        &self,
        client: &EngineClient,
        spec: &TranscribeSpec,
        token: &CancelToken,
        supports_live: bool,
    ) -> Option<JobOutcome> {
        let job = if supports_live {
            let job = client.transcribe_live(spec.clone(), Arc::clone(&self.partial));
            cancel_with(token, client, &job);
            Some(job)
        } else {
            None
        };
        let poll = Duration::from_millis(25);
        while !self.closed.load(Ordering::Acquire) {
            if token.is_cancelled() {
                return Some(JobOutcome::Cancelled);
            }
            if Instant::now() >= self.until {
                if let Some(job) = &job {
                    job.cancel();
                }
                return None;
            }
            if let Some(job) = &job {
                if let Some(done) = job.wait_timeout(poll) {
                    return Some(done);
                }
            } else {
                std::thread::sleep(poll);
            }
        }
        if token.is_cancelled() {
            return Some(JobOutcome::Cancelled);
        }
        match job {
            Some(job) => {
                job.finish_live();
                let result = job.wait_timeout(JOB_TIMEOUT);
                if result.is_none() {
                    job.cancel();
                }
                result
            }
            None => batch(client, spec, token),
        }
    }
}

pub(super) fn batch(
    client: &EngineClient,
    spec: &TranscribeSpec,
    token: &CancelToken,
) -> Option<JobOutcome> {
    let job = client.transcribe(spec.clone());
    cancel_with(token, client, &job);
    job.wait_timeout(JOB_TIMEOUT)
}

fn cancel_with(token: &CancelToken, client: &EngineClient, job: &JobHandle) {
    let id = job.id();
    let client = client.clone();
    token.on_cancel(move || client.cancel(id));
}

fn unavailable(message: &'static str) -> JobOutcome {
    JobOutcome::Failed(Failure::new(ENGINE_UNAVAILABLE, message))
}
