//! Authenticated model serving over the worker frame protocol.

#[cfg(test)]
mod cancel_tests;
mod session;
pub use session::serve_session;

use hushpen_core::idle::{Idle, IdleAction};
use hushpen_core::remote::UnloadRule;
use hushpen_engine::{ChildSpec, EngineClient, EngineLog, LlmClient, Timing};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub struct Workers {
    pub speech: EngineClient,
    pub cleanup: LlmClient,
    pub(crate) speech_busy: AtomicBool,
    pub(crate) cleanup_busy: AtomicBool,
    pub(crate) speech_session: Mutex<()>,
    pub(crate) cleanup_session: Mutex<()>,
    pub(crate) speech_rule: Mutex<UnloadRule>,
    pub(crate) cleanup_rule: Mutex<UnloadRule>,
}

impl Workers {
    pub fn start(asr: PathBuf, llm: PathBuf, logs: &Path) -> io::Result<Arc<Self>> {
        let speech_log = EngineLog::open(&logs.join("engine.log")).map_err(io::Error::other)?;
        let llm_log =
            EngineLog::open_scoped(&logs.join("llm.log"), "llm").map_err(io::Error::other)?;
        Ok(Arc::new(Self {
            speech: EngineClient::start(
                ChildSpec {
                    program: asr,
                    args: vec![],
                },
                Timing::default(),
                speech_log,
            ),
            cleanup: LlmClient::start(
                ChildSpec {
                    program: llm,
                    args: vec![],
                },
                Timing::llm(),
                llm_log,
            ),
            speech_busy: AtomicBool::new(false),
            cleanup_busy: AtomicBool::new(false),
            speech_session: Mutex::new(()),
            cleanup_session: Mutex::new(()),
            speech_rule: Mutex::new(UnloadRule::KeepLoaded),
            cleanup_rule: Mutex::new(UnloadRule::KeepLoaded),
        }))
    }

    pub fn unload(&self) {
        if !self.speech_busy.load(Ordering::Acquire) {
            self.speech.unload();
        }
        if !self.cleanup_busy.load(Ordering::Acquire) {
            self.cleanup.unload();
        }
    }

    pub fn shutdown(&self) {
        self.speech.shutdown();
        self.cleanup.shutdown();
    }

    #[cfg(target_os = "linux")]
    fn unload_for_memory(&self) {
        if *lock(&self.speech_rule) == UnloadRule::WhenMemoryLow
            && !self.speech_busy.load(Ordering::Acquire)
        {
            self.speech.unload();
        }
        if *lock(&self.cleanup_rule) == UnloadRule::WhenMemoryLow
            && !self.cleanup_busy.load(Ordering::Acquire)
        {
            self.cleanup.unload();
        }
    }
}

pub fn idle(rule: UnloadRule) -> Idle {
    Idle {
        secs: match rule {
            UnloadRule::AfterIdle(minutes) => u64::from(minutes).saturating_mul(60),
            _ => 0,
        },
        action: IdleAction::Unload,
    }
}

/// The Linux memory threshold leaves room for the system and another job.
pub fn memory_low(meminfo: &str, pressure: &str) -> bool {
    let mut total = None;
    let mut available = None;
    for line in meminfo.lines() {
        let mut fields = line.split_whitespace();
        let value = fields.next().zip(fields.next());
        match value {
            Some(("MemTotal:", value)) => total = value.parse::<u64>().ok(),
            Some(("MemAvailable:", value)) => available = value.parse::<u64>().ok(),
            _ => {}
        }
    }
    if total.zip(available).is_some_and(|(total, available)| {
        total > 0 && u128::from(available) * 100 < u128::from(total) * 15
    }) {
        return true;
    }
    pressure.lines().any(|line| {
        line.starts_with("full ")
            && line
                .split_whitespace()
                .find_map(|field| field.strip_prefix("avg10="))
                .and_then(|value| value.parse::<f64>().ok())
                .is_some_and(|value| value >= 1.0)
    })
}

pub fn watch_memory(workers: Arc<Workers>, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            #[cfg(target_os = "linux")]
            if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
                let pressure = std::fs::read_to_string("/proc/pressure/memory").unwrap_or_default();
                if memory_low(&info, &pressure) {
                    workers.unload_for_memory();
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = &workers;
            std::thread::sleep(Duration::from_secs(1));
        }
    })
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_threshold_is_strict_and_pressure_can_trigger_it() {
        assert!(!memory_low("MemTotal: 1000 kB\nMemAvailable: 150 kB", ""));
        assert!(memory_low("MemTotal: 1000 kB\nMemAvailable: 149 kB", ""));
        assert!(memory_low(
            "MemTotal: 1000 kB\nMemAvailable: 900 kB",
            "full avg10=1.00 avg60=0 total=1"
        ));
        assert!(!memory_low("bad", "full avg10=nan"));
    }
    #[test]
    fn idle_minutes_become_worker_seconds() {
        assert_eq!(idle(UnloadRule::AfterIdle(5)).secs, 300);
        assert_eq!(idle(UnloadRule::KeepLoaded).secs, 0);
    }

    #[test]
    fn worker_start_creates_the_log_folder_and_shutdown_settles_missing_binaries() {
        let directory = tempfile::tempdir().unwrap();
        let logs = directory.path().join("logs");
        let missing = directory.path().join("missing-worker");
        let workers = Workers::start(missing.clone(), missing, &logs).unwrap();
        workers.shutdown();
        assert!(logs.join("engine.log").is_file());
        assert!(logs.join("llm.log").is_file());
    }
}
