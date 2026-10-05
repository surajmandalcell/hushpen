//! In-memory logs the hook serves: the network request log and the event log.
//!
//! The app records into the process-wide logs; the router reads them. Each
//! entry carries its wall-clock time.

use hushpen_store::time::{iso_millis, now_unix_ms};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};

const CAPACITY: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetEntry {
    pub t_ms: u64,
    /// `ModelDownload`, `UpdateCheck`, `UpdateDownload`, or `Endpoint`.
    pub purpose: String,
    pub host: String,
    /// `ok`, `blocked: <why>`, or `error: <why>`.
    pub result: String,
}

impl NetEntry {
    pub fn to_json(&self) -> Value {
        json!({
            "time": iso_millis(self.t_ms),
            "t_ms": self.t_ms,
            "purpose": self.purpose,
            "host": self.host,
            "result": self.result,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub t_ms: u64,
    pub kind: String,
    pub detail: String,
}

impl Event {
    pub fn to_json(&self) -> Value {
        json!({
            "time": iso_millis(self.t_ms),
            "t_ms": self.t_ms,
            "kind": self.kind,
            "detail": self.detail,
        })
    }
}

/// A bounded log. The oldest entry drops first.
pub struct Log<T> {
    entries: Mutex<VecDeque<T>>,
    capacity: usize,
}

impl<T: Clone> Log<T> {
    pub const fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            capacity,
        }
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<T>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn push(&self, entry: T) {
        let mut entries = self.lock();
        if entries.len() == self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    pub fn snapshot(&self) -> Vec<T> {
        self.lock().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.lock().clear();
    }
}

fn net() -> &'static Log<NetEntry> {
    static LOG: OnceLock<Log<NetEntry>> = OnceLock::new();
    LOG.get_or_init(|| Log::new(CAPACITY))
}

fn events() -> &'static Log<Event> {
    static LOG: OnceLock<Log<Event>> = OnceLock::new();
    LOG.get_or_init(|| Log::new(CAPACITY))
}

/// Records one network request. Call it where the request is made, with the
/// result known: the log never invents requests.
pub fn record_net(purpose: &str, host: &str, result: &str) {
    net().push(NetEntry {
        t_ms: now_unix_ms(),
        purpose: purpose.to_string(),
        host: host.to_string(),
        result: result.to_string(),
    });
}

pub fn net_entries() -> Vec<NetEntry> {
    net().snapshot()
}

pub fn record_event(kind: &str, detail: &str) {
    events().push(Event {
        t_ms: now_unix_ms(),
        kind: kind.to_string(),
        detail: detail.to_string(),
    });
}

pub fn event_entries() -> Vec<Event> {
    events().snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_log_drops_the_oldest_entry() {
        let log = Log::new(3);
        for n in 0..5 {
            log.push(n);
        }
        assert_eq!(log.snapshot(), [2, 3, 4]);
    }

    #[test]
    fn clear_empties_the_log() {
        let log = Log::new(3);
        log.push(1);
        log.clear();
        assert!(log.snapshot().is_empty());
    }

    #[test]
    fn a_net_entry_prints_time_purpose_host_and_result() {
        let entry = NetEntry {
            t_ms: 1_791_203_696_789,
            purpose: "ModelDownload".into(),
            host: "huggingface.co".into(),
            result: "ok".into(),
        };
        let value = entry.to_json();
        assert_eq!(value["time"], "2026-10-05T12:34:56.789Z");
        assert_eq!(value["purpose"], "ModelDownload");
        assert_eq!(value["host"], "huggingface.co");
        assert_eq!(value["result"], "ok");
    }

    #[test]
    fn recording_a_request_adds_it_to_the_global_log() {
        record_net("UpdateCheck", "logs-test.example", "blocked: updates off");
        let found = net_entries()
            .into_iter()
            .any(|entry| entry.host == "logs-test.example" && entry.purpose == "UpdateCheck");
        assert!(found);
    }

    #[test]
    fn recording_an_event_adds_it_to_the_global_log() {
        record_event("logs-test", "one");
        assert!(
            event_entries()
                .iter()
                .any(|event| event.kind == "logs-test" && event.detail == "one")
        );
    }
}
