//! The deadlines of the supervisor: the ready timeout, the ping watchdog, the cancel grace, and
//! the wait of a queued job.

use super::{Manager, Restart};
use crate::client::Failure;
use crate::engine_log::Level;
use crate::worker::{Outcome, Protocol, Reply};
use std::collections::VecDeque;
use std::time::Instant;

impl<P: Protocol> Manager<P> {
    pub(super) fn tick(&mut self) {
        let now = Instant::now();
        if let Some(reason) = self.broken.take() {
            self.down(&reason, Restart::Backoff);
        }
        if self.running.is_none() && self.next_start.is_some_and(|at| now >= at) {
            self.start_child();
        }
        self.watch_child(now);
        self.expire_pending(now);
    }

    fn watch_child(&mut self, now: Instant) {
        let Some(running) = self.running.as_ref() else {
            return;
        };
        if !running.ready {
            if now.duration_since(running.spawned) >= self.timing.ready_timeout {
                self.down("no ready message", Restart::Backoff);
            }
            return;
        }
        if self.in_process_idle() {
            return;
        }
        if now.duration_since(running.last_pong) >= self.timing.pong_timeout {
            self.log
                .write(Level::Warn, "WATCHDOG", "no pong, killing the worker");
            self.down("no pong", Restart::Backoff);
            return;
        }
        let cancel_expired = self
            .inflight
            .values()
            .any(|job| job.cancel_deadline.is_some_and(|deadline| now >= deadline));
        if cancel_expired {
            self.down("cancel did not settle", Restart::Now);
            return;
        }
        if now >= running.next_ping {
            let next = now + self.timing.ping_interval;
            if let Some(running) = self.running.as_mut() {
                running.next_ping = next;
            }
            self.write(&P::ping());
        }
    }

    fn expire_pending(&mut self, now: Instant) {
        let mut kept = VecDeque::new();
        for pending in self.pending.drain(..) {
            if now >= pending.deadline {
                let failure = Failure::new(P::UNAVAILABLE, "the worker did not come back in time");
                self.log.write(
                    Level::Warn,
                    "JOB",
                    &format!(
                        "job={} kind={} model=- code={}",
                        pending.job,
                        P::kind(&pending.work),
                        P::UNAVAILABLE
                    ),
                );
                let _ = pending.reply.send(Reply::End(Outcome::Failed(failure)));
            } else {
                kept.push_back(pending);
            }
        }
        self.pending = kept;
    }

    pub(super) fn next_wakeup(&self) -> Option<Instant> {
        let mut next: Option<Instant> = None;
        let mut consider = |at: Instant| next = Some(next.map_or(at, |next| next.min(at)));
        match &self.running {
            None => {
                if let Some(at) = self.next_start {
                    consider(at);
                }
            }
            Some(running) if !running.ready => {
                consider(running.spawned + self.timing.ready_timeout);
            }
            Some(running) => {
                if !self.in_process_idle() {
                    consider(running.next_ping);
                    consider(running.last_pong + self.timing.pong_timeout);
                }
                for job in self.inflight.values() {
                    if let Some(at) = job.cancel_deadline {
                        consider(at);
                    }
                }
            }
        }
        for pending in &self.pending {
            consider(pending.deadline);
        }
        if self.broken.is_some() {
            consider(Instant::now());
        }
        next
    }
}
