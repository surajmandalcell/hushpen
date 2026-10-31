//! Commands from the app: load a model, start a job, cancel a job.

use super::{Inflight, LoadEntry, Manager, Pending};
use crate::client::{Cmd, JobOutcome, ProgressSink, Work};
use crate::engine_log::Level;
use crate::server::model_name;
use hushpen_core::protocol::Request;
use std::sync::mpsc::Sender;
use std::time::Instant;

impl Manager {
    pub(super) fn command(&mut self, command: Cmd) {
        match command {
            Cmd::Load { spec, reply } => {
                let entry = LoadEntry {
                    spec,
                    reply: Some(reply),
                };
                if self.is_ready() {
                    self.send_load(entry);
                } else {
                    self.unsent_loads.push_back(entry);
                    self.publish();
                }
            }
            Cmd::Transcribe {
                job,
                work,
                progress,
                reply,
            } => {
                if self.is_ready() {
                    self.send_job(job, &work, progress, reply);
                } else {
                    self.pending.push_back(Pending {
                        job,
                        work,
                        progress,
                        reply,
                        deadline: Instant::now() + self.timing.submit_wait,
                    });
                }
            }
            Cmd::Cancel { job } => self.cancel(job),
            Cmd::Shutdown => {}
        }
    }

    fn cancel(&mut self, job: u64) {
        if let Some(position) = self.pending.iter().position(|p| p.job == job)
            && let Some(pending) = self.pending.remove(position)
        {
            let _ = pending.reply.send(JobOutcome::Cancelled);
            self.log.write(
                Level::Info,
                "JOB",
                &format!("job={job} model=- threads=- code=CANCELLED queued"),
            );
            return;
        }
        let grace = self.timing.cancel_grace;
        let Some(inflight) = self.inflight.get_mut(&job) else {
            return;
        };
        if inflight.cancel_deadline.is_none() {
            inflight.cancel_deadline = Some(Instant::now() + grace);
            self.write(&Request::Cancel { job });
        }
    }

    pub(super) fn send_load(&mut self, entry: LoadEntry) {
        let request = Request::LoadModel {
            path: entry.spec.path.to_string_lossy().into_owned(),
            gpu: entry.spec.gpu,
            threads: u32::try_from(entry.spec.threads).unwrap_or(u32::MAX),
            idle_secs: entry.spec.idle.secs,
            idle_action: entry.spec.idle.action,
        };
        self.sent_loads.push_back(entry);
        self.write(&request);
        self.publish();
    }

    pub(super) fn send_job(
        &mut self,
        job: u64,
        work: &Work,
        progress: Option<ProgressSink>,
        reply: Sender<JobOutcome>,
    ) {
        let (model, threads) = self
            .sent_loads
            .back()
            .map(|entry| &entry.spec)
            .or(self.current.as_ref())
            .map_or_else(
                || ("-".to_owned(), 0),
                |load| (model_name(&load.path), load.threads),
            );
        self.inflight.insert(
            job,
            Inflight {
                reply,
                progress,
                sent: Instant::now(),
                cancel_deadline: None,
                model,
                threads,
            },
        );
        let request = match work {
            Work::Wav(spec) => Request::Transcribe {
                job,
                wav_path: spec.wav_path.to_string_lossy().into_owned(),
                language: spec.language.clone(),
                prompt: spec.prompt.clone(),
            },
            Work::Import(spec) => Request::ImportFile {
                job,
                path: spec.path.to_string_lossy().into_owned(),
                language: spec.language.clone(),
                prompt: spec.prompt.clone(),
                translate: spec.translate,
            },
        };
        self.write(&request);
    }
}
