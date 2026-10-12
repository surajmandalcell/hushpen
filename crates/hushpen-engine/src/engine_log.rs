//! `logs/engine.log`: one line per job, plus the life of the child (spawn, load, crash, restart).
//!
//! Format: `<iso> <LEVEL> engine <CODE> <detail>`. The detail never holds transcript text, only
//! model names, thread counts, timings, and result codes.

use hushpen_store::log_file::{LOG_FILES, LOG_MAX_BYTES, RotatingFile};
use hushpen_store::time;
use std::path::Path;
use std::sync::{Arc, Mutex};

const DETAIL_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
        }
    }
}

pub struct EngineLog {
    sink: Option<Mutex<RotatingFile>>,
    scope: &'static str,
}

impl EngineLog {
    pub fn open(path: &Path) -> hushpen_store::Result<Arc<Self>> {
        Self::open_scoped(path, "engine")
    }

    /// A log whose lines carry `scope` after the level, for example `llm`.
    pub fn open_scoped(path: &Path, scope: &'static str) -> hushpen_store::Result<Arc<Self>> {
        let sink = RotatingFile::open(path, LOG_MAX_BYTES, LOG_FILES)?;
        Ok(Arc::new(Self {
            sink: Some(Mutex::new(sink)),
            scope,
        }))
    }

    /// A log that keeps nothing, for tests and for an app that could not open its log folder.
    pub fn discard() -> Arc<Self> {
        Arc::new(Self {
            sink: None,
            scope: "engine",
        })
    }

    pub fn write(&self, level: Level, code: &str, detail: &str) {
        let Some(sink) = &self.sink else { return };
        let line = format_line(time::now_unix_ms(), self.scope, level, code, detail);
        let mut sink = sink.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // A log that cannot be written must not take the supervisor down.
        let _ = sink.write_line(&line);
        if level == Level::Warn {
            let _ = sink.sync();
        }
    }
}

fn format_line(unix_ms: u64, scope: &str, level: Level, code: &str, detail: &str) -> String {
    let detail: String = detail
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(DETAIL_LIMIT)
        .collect();
    format!(
        "{} {} {scope} {code} {detail}\n",
        time::iso_millis(unix_ms),
        level.name()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_has_time_level_scope_code_and_detail() {
        let line = format_line(0, "engine", Level::Warn, "CRASH", "pid=7");
        assert_eq!(line, "1970-01-01T00:00:00.000Z WARN engine CRASH pid=7\n");
    }

    #[test]
    fn control_characters_cannot_split_a_line() {
        let line = format_line(0, "engine", Level::Info, "STDERR", "a\nb\tc");
        assert_eq!(line.matches('\n').count(), 1);
        assert!(line.contains("a b c"));
    }

    #[test]
    fn a_long_detail_is_cut() {
        let line = format_line(0, "engine", Level::Info, "STDERR", &"x".repeat(2000));
        assert!(line.len() < 600);
    }

    #[test]
    fn a_scoped_log_names_its_scope_in_each_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs/llm.log");
        let log = EngineLog::open_scoped(&path, "llm").unwrap();
        log.write(Level::Info, "READY", "pid=7");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(" INFO llm READY pid=7"), "{text}");
    }

    #[test]
    fn lines_reach_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs/engine.log");
        let log = EngineLog::open(&path).unwrap();
        log.write(Level::Info, "JOB", "model=tiny.en");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("INFO engine JOB model=tiny.en"));
    }
}
