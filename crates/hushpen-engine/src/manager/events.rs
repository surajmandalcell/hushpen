//! Events from the child: ready, progress, loaded, results, and errors, and the job lines of
//! `engine.log`.

use super::{Inflight, LoadEntry, Manager, Restart};
use crate::client::{Failure, ImportProgress, JobOutcome, Loaded, Transcription};
use crate::engine_log::Level;
use crate::server::model_name;
use hushpen_core::error;
use hushpen_core::protocol::{Event, PROTOCOL_VERSION};
use std::time::Instant;

impl Manager {
    pub(super) fn event(&mut self, event: Event) {
        match event {
            Event::Ready { protocol, .. } if protocol != PROTOCOL_VERSION => {
                self.down("protocol mismatch", Restart::Backoff);
            }
            Event::Ready { version, gpu, .. } => self.on_ready(&version, gpu),
            Event::Pong => {
                if let Some(running) = self.running.as_mut() {
                    running.last_pong = Instant::now();
                }
            }
            Event::Loading { .. } | Event::Segment { .. } => {}
            Event::Progress {
                job,
                fraction,
                audio_ms,
            } => {
                if let Some(inflight) = self.inflight.get(&job)
                    && inflight.cancel_deadline.is_none()
                    && let Some(progress) = &inflight.progress
                {
                    progress(ImportProgress {
                        fraction: fraction.clamp(0.0, 1.0) as f32,
                        audio_ms,
                    });
                }
            }
            Event::Loaded { model, ms, gpu } => {
                let Some(entry) = self.sent_loads.pop_front() else {
                    return;
                };
                self.log.write(
                    Level::Info,
                    "LOADED",
                    &format!(
                        "model={model} threads={} gpu={} ms={ms}",
                        entry.spec.threads,
                        on_off(gpu)
                    ),
                );
                self.model = Some(model.clone());
                self.gpu = gpu;
                self.current = Some(entry.spec);
                // The status is published before the answer, so a caller that reads it next sees the model.
                self.publish();
                if let Some(reply) = entry.reply {
                    let _ = reply.send(Ok(Loaded { model, ms, gpu }));
                }
            }
            Event::Result {
                job,
                text,
                language,
                segments,
                audio_ms,
                ms,
            } => self.finish(
                job,
                JobOutcome::Done(Transcription {
                    text,
                    language,
                    segments,
                    audio_ms,
                    decode_ms: ms,
                }),
                Some((audio_ms, ms)),
            ),
            Event::Cancelled { job } => self.finish(job, JobOutcome::Cancelled, None),
            Event::Error {
                job: Some(job),
                code,
                params,
            } => {
                let failure = failure_from(code, &params);
                self.finish(job, JobOutcome::Failed(failure), None);
            }
            Event::Error {
                job: None,
                code,
                params,
            } => {
                let failure = failure_from(code.clone(), &params);
                if code == error::ENGINE_LOAD_FAILED
                    && let Some(entry) = self.sent_loads.pop_front()
                {
                    self.log.write(
                        Level::Warn,
                        "LOAD_FAILED",
                        &format!("model={} code={code}", model_name(&entry.spec.path)),
                    );
                    self.publish();
                    if let Some(reply) = entry.reply {
                        let _ = reply.send(Err(failure));
                    }
                } else {
                    self.log
                        .write(Level::Warn, "ENGINE_ERROR", &format!("code={code}"));
                }
            }
        }
    }

    fn on_ready(&mut self, version: &str, gpu: bool) {
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
            &format!("pid={pid} version={version} gpu_available={}", on_off(gpu)),
        );
        // A restart brings the dictation model back without anyone asking.
        if self.unsent_loads.is_empty()
            && let Some(spec) = self.current.clone()
        {
            self.unsent_loads.push_back(LoadEntry { spec, reply: None });
        }
        while let Some(entry) = self.unsent_loads.pop_front() {
            self.send_load(entry);
        }
        while let Some(pending) = self.pending.pop_front() {
            self.send_job(pending.job, &pending.work, pending.progress, pending.reply);
        }
        self.publish();
    }

    /// Answers a job and writes its line to `engine.log`.
    pub(super) fn finish(&mut self, job: u64, outcome: JobOutcome, timings: Option<(u64, u64)>) {
        let Some(inflight) = self.inflight.remove(&job) else {
            return;
        };
        // The user asked to stop this job, so a result that raced the cancel is dropped.
        let outcome =
            if inflight.cancel_deadline.is_some() && matches!(outcome, JobOutcome::Done(_)) {
                JobOutcome::Cancelled
            } else {
                outcome
            };
        self.log_job(job, &inflight, &outcome, timings);
        let _ = inflight.reply.send(outcome);
    }

    fn log_job(
        &self,
        job: u64,
        inflight: &Inflight,
        outcome: &JobOutcome,
        timings: Option<(u64, u64)>,
    ) {
        let (code, level) = match outcome {
            JobOutcome::Done(_) => ("OK", Level::Info),
            JobOutcome::Cancelled => ("CANCELLED", Level::Info),
            JobOutcome::Failed(failure) => (failure.code.as_str(), Level::Warn),
        };
        let (audio, decode) = timings.map_or_else(
            || ("-".to_owned(), "-".to_owned()),
            |(audio, decode)| (audio.to_string(), decode.to_string()),
        );
        self.log.write(
            level,
            "JOB",
            &format!(
                "job={job} model={} threads={} gpu={} audio_ms={audio} decode_ms={decode} total_ms={} code={code}",
                inflight.model,
                inflight.threads,
                on_off(self.gpu),
                inflight.sent.elapsed().as_millis()
            ),
        );
    }
}

fn failure_from(code: String, params: &hushpen_core::protocol::Params) -> Failure {
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
