//! Commands from the app: load a model, start a job, cancel a job, unload.

use super::{Inflight, LoadEntry, Manager, Pending};
use crate::engine_log::Level;
use crate::worker::{Cmd, Outcome, Protocol, Reply};
use std::sync::mpsc::Sender;
use std::time::Instant;

impl<P: Protocol> Manager<P> {
    pub(super) fn command(&mut self, command: Cmd<P>) {
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
            Cmd::Run { job, work, reply } => {
                if self.is_ready() {
                    self.send_job(job, &work, reply);
                } else {
                    self.pending.push_back(Pending {
                        job,
                        work,
                        reply,
                        deadline: Instant::now() + self.timing.submit_wait,
                    });
                }
            }
            Cmd::Cancel { job } => self.cancel(job),
            Cmd::Unload => {
                if self.is_ready() {
                    self.write(&P::unload());
                }
            }
            Cmd::Shutdown => {}
        }
    }

    fn cancel(&mut self, job: u64) {
        if let Some(position) = self.pending.iter().position(|p| p.job == job)
            && let Some(pending) = self.pending.remove(position)
        {
            let _ = pending.reply.send(Reply::End(Outcome::Cancelled));
            self.log.write(
                Level::Info,
                "JOB",
                &format!(
                    "job={job} kind={} model=- code=CANCELLED queued",
                    P::kind(&pending.work)
                ),
            );
            return;
        }
        let grace = self.timing.cancel_grace;
        let Some(inflight) = self.inflight.get_mut(&job) else {
            return;
        };
        if inflight.cancel_deadline.is_none() {
            inflight.cancel_deadline = Some(Instant::now() + grace);
            self.write(&P::cancel(job));
        }
    }

    pub(super) fn send_load(&mut self, entry: LoadEntry<P::Spec>) {
        let request = P::load(&entry.spec);
        if let Some(running) = self.running.as_mut() {
            running.configured = true;
        }
        self.sent_loads.push_back(entry);
        self.write(&request);
        self.publish();
    }

    pub(super) fn send_job(&mut self, job: u64, work: &P::Work, reply: Sender<Reply<P>>) {
        // After a restart that left the model unloaded, the new child learns the model here.
        if self.running.as_ref().is_some_and(|r| !r.configured)
            && let Some(spec) = self.current.clone()
        {
            self.send_load(LoadEntry { spec, reply: None });
        }
        let model = self
            .sent_loads
            .back()
            .map(|entry| &entry.spec)
            .or(self.current.as_ref())
            .map_or_else(|| "-".to_owned(), P::model_name);
        self.inflight.insert(
            job,
            Inflight {
                reply,
                sent: Instant::now(),
                cancel_deadline: None,
                kind: P::kind(work),
                model,
            },
        );
        self.write(&P::job(job, work));
    }
}
