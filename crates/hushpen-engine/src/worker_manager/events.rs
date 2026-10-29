//! Events from the child: ready, loaded, progress, results, and errors, and the job lines of
//! the log.

use super::{LoadEntry, Manager, Restart};
use crate::client::Failure;
use crate::engine_log::Level;
use crate::worker::{Loaded, Outcome, Protocol, Reply, Signal};
use hushpen_core::protocol::Params;
use std::time::Instant;

impl<P: Protocol> Manager<P> {
    pub(super) fn event(&mut self, signal: Signal<P>) {
        match signal {
            Signal::Ready { protocol, .. } if protocol != P::PROTOCOL_VERSION => {
                self.down("protocol mismatch", Restart::Backoff);
            }
            Signal::Ready {
                version, backend, ..
            } => self.on_ready(&version, &backend),
            Signal::Pong => {
                if let Some(running) = self.running.as_mut() {
                    running.last_pong = Instant::now();
                }
            }
            Signal::Loaded(loaded) => self.on_loaded(loaded),
            Signal::Unloaded(reason) => {
                self.log.write(
                    Level::Info,
                    "UNLOADED",
                    &format!("reason={}", reason.as_str()),
                );
                self.model = None;
                self.unloaded = Some(reason);
                self.publish();
            }
            Signal::Progress { job, progress } => {
                if let Some(inflight) = self.inflight.get(&job) {
                    let _ = inflight.reply.send(Reply::Progress(progress));
                }
            }
            Signal::Done { job, done, counts } => self.finish(job, Outcome::Done(done), &counts),
            Signal::Cancelled { job } => self.finish(job, Outcome::Cancelled, ""),
            Signal::Error {
                job: Some(job),
                code,
                params,
            } => {
                let failure = failure_from(code, &params);
                self.finish(job, Outcome::Failed(failure), "");
            }
            Signal::Error {
                job: None,
                code,
                params,
            } => self.on_error(code, &params),
            Signal::Other => {}
        }
    }

    fn on_error(&mut self, code: String, params: &Params) {
        let failure = failure_from(code.clone(), params);
        if code == P::LOAD_FAILED
            && let Some(entry) = self.sent_loads.pop_front()
        {
            self.log.write(
                Level::Warn,
                "LOAD_FAILED",
                &format!("model={} code={code}", P::model_name(&entry.spec)),
            );
            self.publish();
            if let Some(reply) = entry.reply {
                let _ = reply.send(Err(failure));
            }
        } else {
            self.log
                .write(Level::Warn, "WORKER_ERROR", &format!("code={code}"));
        }
    }

    fn on_loaded(&mut self, loaded: Loaded) {
        // A reload after an idle unload arrives with no request waiting for it.
        let entry = self.sent_loads.pop_front();
        self.log.write(
            Level::Info,
            "LOADED",
            &format!(
                "model={} gpu={} backend={} ms={}",
                loaded.model,
                on_off(loaded.gpu),
                loaded.backend,
                loaded.ms
            ),
        );
        self.model = Some(loaded.model.clone());
        self.backend = Some(loaded.backend.clone());
        self.gpu = loaded.gpu;
        self.unloaded = None;
        let reply = entry.and_then(|entry| {
            self.current = Some(entry.spec);
            entry.reply
        });
        // The status is published before the answer, so a caller that reads it next sees the model.
        self.publish();
        if let Some(reply) = reply {
            let _ = reply.send(Ok(loaded));
        }
    }

    fn on_ready(&mut self, version: &str, backend: &str) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        if running.ready {
            return;
        }
        let now = Instant::now();
        running.ready = true;
        running.last_pong = now;
        running.next_ping = now + self.timing.ping_interval;
        let pid = running.child.id();
        self.log.write(
            Level::Info,
            "READY",
            &format!("pid={pid} version={version} backend={backend}"),
        );
        // A restart brings the model back without anyone asking, unless it had been dropped.
        if self.unsent_loads.is_empty()
            && self.warm
            && let Some(spec) = self.current.clone()
        {
            self.unsent_loads.push_back(LoadEntry { spec, reply: None });
        }
        while let Some(entry) = self.unsent_loads.pop_front() {
            self.send_load(entry);
        }
        while let Some(pending) = self.pending.pop_front() {
            self.send_job(pending.job, &pending.work, pending.reply);
        }
        self.publish();
    }

    /// Answers a job and writes its line to the log. The line holds counts, never text.
    pub(super) fn finish(&mut self, job: u64, outcome: Outcome<P::Done>, counts: &str) {
        let Some(inflight) = self.inflight.remove(&job) else {
            return;
        };
        // The user asked to stop this job, so a result that raced the cancel is dropped.
        let outcome = if inflight.cancel_deadline.is_some() && matches!(outcome, Outcome::Done(_)) {
            Outcome::Cancelled
        } else {
            outcome
        };
        let (code, level) = match &outcome {
            Outcome::Done(_) => ("OK", Level::Info),
            Outcome::Cancelled => ("CANCELLED", Level::Info),
            Outcome::Failed(failure) => (failure.code.as_str(), Level::Warn),
        };
        self.log.write(
            level,
            "JOB",
            &format!(
                "job={job} kind={} model={} backend={} gpu={} {counts} total_ms={} code={code}",
                inflight.kind,
                inflight.model,
                self.backend.as_deref().unwrap_or("-"),
                on_off(self.gpu),
                inflight.sent.elapsed().as_millis()
            ),
        );
        let _ = inflight.reply.send(Reply::End(outcome));
    }
}

fn failure_from(code: String, params: &Params) -> Failure {
    let (detail, reason) = hushpen_core::protocol::error_texts(params);
    Failure {
        code,
        detail,
        reason,
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}
