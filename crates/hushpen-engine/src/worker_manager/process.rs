//! The child process: start, write to, take down, and stop it, and the threads that read its output.

use super::{ChildEvent, Manager, Msg, Restart, Running};
use crate::client::Failure;
use crate::engine_log::{EngineLog, Level};
use crate::worker::{Outcome, Protocol, Reply};
use hushpen_core::protocol::write_json;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::{ChildStderr, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

impl<P: Protocol> Manager<P> {
    pub(super) fn is_ready(&self) -> bool {
        self.running.as_ref().is_some_and(|running| running.ready)
    }

    pub(super) fn start_child(&mut self) {
        self.next_start = None;
        let spawned = Command::new(&self.spec.program)
            .args(&self.spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(failure) => {
                self.failed_start(&failure.to_string());
                return;
            }
        };
        let (Some(mut stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            let _ = child.wait();
            self.failed_start("the child has no pipes");
            return;
        };
        let now = Instant::now();
        self.log
            .write(Level::Info, "SPAWN", &format!("pid={}", child.id()));
        let hello = write_json(&mut stdin, &P::hello());
        self.spawn_readers(stdout, stderr);
        self.running = Some(Running {
            child,
            stdin,
            spawned: now,
            ready: false,
            configured: false,
            last_pong: now,
            next_ping: now,
        });
        if let Err(failure) = hello {
            self.broken = Some(format!("hello failed: {failure}"));
        }
        self.publish();
    }

    fn failed_start(&mut self, reason: &str) {
        let delay = self.backoff.next_delay(Duration::ZERO);
        self.restarts += 1;
        self.next_start = Some(Instant::now() + delay);
        self.log.write(
            Level::Warn,
            "SPAWN_FAILED",
            &format!("{reason} restart_in_ms={}", delay.as_millis()),
        );
        self.publish();
    }

    fn spawn_readers(&self, stdout: ChildStdout, stderr: ChildStderr) {
        let generation = self.generation;
        let sender = self.sender.clone();
        thread::spawn(move || read_events::<P>(stdout, &sender, generation));
        let log = Arc::clone(&self.log);
        thread::spawn(move || pump_stderr(stderr, &log));
    }

    pub(super) fn write(&mut self, request: &Value) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        if let Err(failure) = write_json(&mut running.stdin, request) {
            self.broken
                .get_or_insert_with(|| format!("write failed: {failure}"));
        }
    }

    /// Takes the child down (if it is not already), settles every request that waited on it,
    /// and schedules the next start.
    pub(super) fn down(&mut self, reason: &str, restart: Restart) {
        let Some(mut running) = self.running.take() else {
            return;
        };
        // A caller released below must not read the dead child's pid as the live worker.
        self.publish();
        self.generation += 1;
        let pid = running.child.id();
        let uptime = running.spawned.elapsed();
        let _ = running.child.kill();
        self.warm = self.model.is_some();

        let failure = Failure::new(P::CRASHED, reason);
        let jobs: Vec<u64> = self.inflight.keys().copied().collect();
        for job in jobs {
            let cancelled = self
                .inflight
                .get(&job)
                .is_some_and(|inflight| inflight.cancel_deadline.is_some());
            let outcome = if cancelled {
                Outcome::Cancelled
            } else {
                Outcome::Failed(failure.clone())
            };
            self.finish(job, outcome, "");
        }
        for entry in self.sent_loads.drain(..) {
            if let Some(reply) = entry.reply {
                let _ = reply.send(Err(failure.clone()));
            }
        }

        drop(running.stdin);
        let status = running
            .child
            .wait()
            .map_or_else(|failure| failure.to_string(), |status| status.to_string());
        let delay = match restart {
            Restart::Backoff => self.backoff.next_delay(uptime),
            Restart::Now => Duration::ZERO,
        };
        self.next_start = Some(Instant::now() + delay);
        self.restarts += 1;
        self.model = None;
        self.backend = None;
        self.gpu = false;
        let code = match restart {
            Restart::Backoff => "CRASH",
            Restart::Now => "CANCEL_KILL",
        };
        self.log.write(
            Level::Warn,
            code,
            &format!(
                "pid={pid} reason={reason} exit={status} uptime_ms={} restart_in_ms={}",
                uptime.as_millis(),
                delay.as_millis()
            ),
        );
        self.publish();
    }

    pub(super) fn stop(&mut self) {
        self.stopped = true;
        let failure = Failure::new(P::UNAVAILABLE, "the worker is shutting down");
        for pending in self.pending.drain(..) {
            let _ = pending
                .reply
                .send(Reply::End(Outcome::Failed(failure.clone())));
        }
        for (_, inflight) in self.inflight.drain() {
            let _ = inflight
                .reply
                .send(Reply::End(Outcome::Failed(failure.clone())));
        }
        for entry in self.sent_loads.drain(..).chain(self.unsent_loads.drain(..)) {
            if let Some(reply) = entry.reply {
                let _ = reply.send(Err(failure.clone()));
            }
        }
        if let Some(mut running) = self.running.take() {
            let _ = write_json(&mut running.stdin, &P::shutdown());
            drop(running.stdin);
            let deadline = Instant::now() + Duration::from_secs(1);
            let exited = loop {
                match running.child.try_wait() {
                    Ok(Some(_)) => break true,
                    Ok(None) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    _ => break false,
                }
            };
            if !exited {
                let _ = running.child.kill();
                let _ = running.child.wait();
            }
            self.log.write(
                Level::Info,
                "STOP",
                &format!("pid={} clean={exited}", running.child.id()),
            );
        }
        self.model = None;
        self.publish();
    }
}

fn read_events<P: Protocol>(mut stdout: ChildStdout, sender: &Sender<Msg<P>>, generation: u64) {
    loop {
        let event = match P::read(&mut stdout) {
            Ok(Some(event)) => ChildEvent::Event(event),
            Ok(None) => ChildEvent::Closed("the worker closed its output".into()),
            Err(problem) => ChildEvent::Closed(problem),
        };
        let last = matches!(event, ChildEvent::Closed(_));
        if sender.send(Msg::Child { generation, event }).is_err() || last {
            return;
        }
    }
}

fn pump_stderr(stderr: ChildStderr, log: &EngineLog) {
    for line in BufReader::new(stderr).split(b'\n') {
        let Ok(bytes) = line else { return };
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim();
        if !text.is_empty() {
            log.write(Level::Info, "STDERR", text);
        }
    }
}
