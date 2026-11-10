//! Authenticated model serving over the worker frame protocol.

#[cfg(test)]
mod cancel_tests;
#[cfg(test)]
mod idle_tests;
mod session;
pub use session::serve_session;

use hushpen_core::idle::{Idle, IdleAction};
use hushpen_core::remote::UnloadRule;
use hushpen_engine::{ChildSpec, EngineClient, EngineLog, LlmClient, Timing};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

pub struct Workers {
    pub speech: EngineClient,
    pub cleanup: LlmClient,
    pub(crate) speech_busy: AtomicBool,
    pub(crate) cleanup_busy: AtomicBool,
    pub(crate) speech_session: Arc<Mutex<()>>,
    pub(crate) cleanup_session: Mutex<()>,
    pub(crate) speech_rule: Mutex<UnloadRule>,
    pub(crate) cleanup_rule: Mutex<UnloadRule>,
    pub(crate) os: hushpen_core::os::Os,
    pub(crate) system_cleanup: bool,
    activity: Mutex<Activity>,
    memory_changed: Condvar,
    memory_epoch: Mutex<u64>,
}

type ActivityCallback = Arc<dyn Fn(bool) + Send + Sync>;

#[derive(Default)]
struct Activity {
    count: usize,
    callback: Option<ActivityCallback>,
}

pub(crate) struct JobActivity(Arc<Workers>);

impl Drop for JobActivity {
    fn drop(&mut self) {
        let mut activity = lock(&self.0.activity);
        activity.count -= 1;
        if activity.count == 0
            && let Some(callback) = &activity.callback
        {
            callback(false);
        }
    }
}

impl Workers {
    pub fn start(asr: PathBuf, llm: PathBuf, logs: &Path) -> io::Result<Arc<Self>> {
        let speech_log = EngineLog::open(&logs.join("engine.log")).map_err(io::Error::other)?;
        let llm_log =
            EngineLog::open_scoped(&logs.join("llm.log"), "llm").map_err(io::Error::other)?;
        let speech = EngineClient::start(
            ChildSpec {
                program: asr,
                args: vec![],
            },
            Timing::default(),
            speech_log,
        );
        let cleanup = LlmClient::start(
            ChildSpec {
                program: llm,
                args: vec![],
            },
            Timing::llm(),
            llm_log,
        );
        Ok(Self::from_clients(
            speech,
            cleanup,
            hushpen_core::os::Os::current(),
            false,
        ))
    }

    /// Phone threads use the same authenticated sessions as server processes.
    pub fn from_clients(
        speech: EngineClient,
        cleanup: LlmClient,
        os: hushpen_core::os::Os,
        system_cleanup: bool,
    ) -> Arc<Self> {
        Self::from_clients_with_speech_session(
            speech,
            cleanup,
            os,
            system_cleanup,
            Arc::new(Mutex::new(())),
        )
    }

    /// A phone shares speech ownership with its local dictation API.
    pub fn from_clients_with_speech_session(
        speech: EngineClient,
        cleanup: LlmClient,
        os: hushpen_core::os::Os,
        system_cleanup: bool,
        speech_session: Arc<Mutex<()>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            speech,
            cleanup,
            speech_busy: AtomicBool::new(false),
            cleanup_busy: AtomicBool::new(false),
            speech_session,
            cleanup_session: Mutex::new(()),
            speech_rule: Mutex::new(UnloadRule::KeepLoaded),
            cleanup_rule: Mutex::new(UnloadRule::KeepLoaded),
            os,
            system_cleanup,
            activity: Mutex::new(Activity::default()),
            memory_changed: Condvar::new(),
            memory_epoch: Mutex::new(0),
        })
    }

    /// The callback must not call back into these workers.
    pub fn set_job_activity(&self, callback: Option<ActivityCallback>) {
        let mut activity = lock(&self.activity);
        if let Some(previous) = activity.callback.take() {
            previous(false);
        }
        if let Some(callback) = &callback {
            callback(activity.count > 0);
        }
        activity.callback = callback;
    }

    pub(crate) fn job_started(self: &Arc<Self>) -> JobActivity {
        let mut activity = lock(&self.activity);
        activity.count += 1;
        if activity.count == 1
            && let Some(callback) = &activity.callback
        {
            callback(true);
        }
        JobActivity(Arc::clone(self))
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

    pub fn set_unload_rule(&self, rule: UnloadRule) {
        *lock(&self.speech_rule) = rule;
        *lock(&self.cleanup_rule) = rule;
        self.rule_changed();
    }

    pub(crate) fn rule_changed(&self) {
        let mut epoch = lock(&self.memory_epoch);
        *epoch = epoch.wrapping_add(1);
        self.memory_changed.notify_all();
    }

    /// Platform memory callbacks use the same per-worker rule as Linux pressure checks.
    pub fn memory_low(&self) {
        self.unload_for_memory();
    }

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

pub struct MemoryWatch {
    workers: Arc<Workers>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MemoryWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.workers.rule_changed();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn watch_memory(workers: Arc<Workers>) -> MemoryWatch {
    let stop = Arc::new(AtomicBool::new(false));
    let running = Arc::clone(&stop);
    let watched = Arc::clone(&workers);
    let thread = std::thread::spawn(move || {
        let workers = watched;
        let stop = running;
        let mut quiet = 0_u32;
        let mut epoch = lock(&workers.memory_epoch);
        while !stop.load(Ordering::Acquire) {
            let watches = *lock(&workers.speech_rule) == UnloadRule::WhenMemoryLow
                || *lock(&workers.cleanup_rule) == UnloadRule::WhenMemoryLow;
            if !watches {
                quiet = 0;
                epoch = workers
                    .memory_changed
                    .wait(epoch)
                    .unwrap_or_else(|error| error.into_inner());
                continue;
            }
            #[cfg(target_os = "linux")]
            if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
                let pressure = std::fs::read_to_string("/proc/pressure/memory").unwrap_or_default();
                if memory_low(&info, &pressure) {
                    workers.unload_for_memory();
                    quiet = 0;
                }
            }
            let generation = *epoch;
            let (next, _) = workers
                .memory_changed
                .wait_timeout(
                    epoch,
                    Duration::from_secs(hushpen_core::idle::pressure_check_secs(quiet)),
                )
                .unwrap_or_else(|error| error.into_inner());
            quiet = if generation == *next {
                quiet.saturating_add(1)
            } else {
                0
            };
            epoch = next;
        }
    });
    MemoryWatch {
        workers,
        stop,
        thread: Some(thread),
    }
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
    fn an_idle_memory_watch_stops_without_waiting_for_a_poll() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing-worker");
        let workers = Workers::start(missing.clone(), missing, directory.path()).unwrap();
        let watch = watch_memory(Arc::clone(&workers));
        let (sent, received) = std::sync::mpsc::channel();
        let rules = Arc::clone(&workers);
        let shutdown = std::thread::spawn(move || {
            for _ in 0..10 {
                rules.set_unload_rule(UnloadRule::WhenMemoryLow);
                rules.set_unload_rule(UnloadRule::KeepLoaded);
            }
            drop(watch);
            sent.send(()).unwrap();
        });
        received.recv_timeout(Duration::from_secs(1)).unwrap();
        shutdown.join().unwrap();
        workers.shutdown();
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

    #[test]
    fn activity_stays_on_until_the_last_job_finishes() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing-worker");
        let workers = Workers::start(missing.clone(), missing, directory.path()).unwrap();
        let changes = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&changes);
        workers.set_job_activity(Some(Arc::new(move |active| {
            lock(&observed).push(active);
        })));
        let speech = workers.job_started();
        let cleanup = workers.job_started();
        drop(speech);
        assert_eq!(*lock(&changes), [false, true]);
        drop(cleanup);
        assert_eq!(*lock(&changes), [false, true, false]);
        let job = workers.job_started();
        workers.set_job_activity(None);
        drop(job);
        assert_eq!(*lock(&changes), [false, true, false, true, false]);
        workers.shutdown();
    }
}
