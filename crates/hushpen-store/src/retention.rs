//! How long the audio of a history row is kept, and the sweep that removes the audio that is
//! too old. The text of a row stays after its audio goes; the row only remembers when the
//! audio was removed.

use crate::db::Database;
use crate::history::ensure_writable;
use crate::{Error, Result};
use rusqlite::params;
use std::fs;
use std::io;
use std::path::{Component, Path};
use std::time::Duration;

pub const SETTING: &str = "history.audioRetention";
pub const DAY_MS: i64 = 86_400_000;
/// How long audio is kept when the setting says `30d`, and the limit for the audio of rows
/// that did not finish when the setting says `never`.
pub const DEFAULT_DAYS: i64 = 30;
/// The time between two sweeps while the app runs.
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retention {
    /// Audio of a finished row goes at once. Audio of a failed or cancelled row stays until it
    /// is 30 days old, so Reprocess can still run on it.
    Never,
    /// Audio goes when the row is this many days old.
    Days(i64),
    Forever,
}

impl Retention {
    /// An unknown value reads as the default, so a hand-edited file never keeps audio longer
    /// than the owner meant.
    pub fn from_setting(value: &str) -> Self {
        match value {
            "never" => Self::Never,
            "forever" => Self::Forever,
            _ => Self::Days(DEFAULT_DAYS),
        }
    }

    /// True when the audio of a row with this status is kept after the run.
    pub fn keeps(self, status: &str) -> bool {
        match self {
            Self::Never => status != "completed",
            _ => true,
        }
    }

    /// Rows created before this time lose their audio, or `None` when age never removes it.
    fn cutoff(self, now_ms: i64) -> Option<i64> {
        match self {
            Self::Never => Some(now_ms - DEFAULT_DAYS * DAY_MS),
            Self::Days(days) => Some(now_ms - days * DAY_MS),
            Self::Forever => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepReport {
    pub removed: usize,
    /// Files that could not be removed. Their rows keep the path, so the next sweep tries again.
    pub failed: usize,
}

/// Removes the audio of every row that `retention` no longer covers.
pub fn sweep(db: &Database, root: &Path, retention: Retention, now_ms: i64) -> Result<SweepReport> {
    ensure_writable(db)?;
    let cutoff = retention.cutoff(now_ms);
    let finished = matches!(retention, Retention::Never);
    if cutoff.is_none() && !finished {
        return Ok(SweepReport::default());
    }
    let due: Vec<(String, String)> = {
        let mut statement = db.connection().prepare(
            "SELECT id, audio_path FROM transcript WHERE audio_path IS NOT NULL \
             AND (created_at < ?1 OR (?2 AND status = 'completed')) ORDER BY created_at",
        )?;
        statement
            .query_map(params![cutoff.unwrap_or(i64::MIN), finished], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut report = SweepReport::default();
    for (id, path) in due {
        match remove_audio(db, root, &id, &path, now_ms) {
            Ok(()) => report.removed += 1,
            Err(error) => {
                log::warn!("the audio of a history row was not removed: {error}");
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

/// Deletes the audio file of one row and records when. A file that is already gone counts as
/// removed.
pub fn remove_audio(
    db: &Database,
    root: &Path,
    id: &str,
    audio_path: &str,
    now_ms: i64,
) -> Result<()> {
    ensure_writable(db)?;
    let relative = Path::new(audio_path);
    // The path comes from a file the owner can edit, so it never leaves the data folder.
    if !relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(Error::io(
            format!("the audio path of row {id} leaves the data folder"),
            io::Error::from(io::ErrorKind::InvalidInput),
        ));
    }
    let file = root.join(relative);
    match fs::remove_file(&file) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(Error::io(
                format!("could not remove {}", file.display()),
                error,
            ));
        }
    }
    db.connection().execute(
        "UPDATE transcript SET audio_path = NULL, audio_removed_at = ?2 WHERE id = ?1",
        params![id, now_ms],
    )?;
    Ok(())
}

/// When the next sweep is due. The first one is due at once, at start; the next ones follow
/// every `SWEEP_INTERVAL` after the last.
#[derive(Debug, Default)]
pub struct Schedule {
    last_ms: Option<i64>,
}

impl Schedule {
    /// How long until a sweep is due. Zero means now. A clock that moved back counts as due,
    /// so the schedule starts again from the new time.
    pub fn wait(&self, now_ms: i64) -> Duration {
        let interval = SWEEP_INTERVAL.as_millis() as i64;
        match self.last_ms {
            Some(last) if now_ms >= last && now_ms - last < interval => {
                Duration::from_millis((interval - (now_ms - last)) as u64)
            }
            _ => Duration::ZERO,
        }
    }

    pub fn ran(&mut self, now_ms: i64) {
        self.last_ms = Some(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_values_map_to_the_three_policies() {
        assert_eq!(Retention::from_setting("never"), Retention::Never);
        assert_eq!(Retention::from_setting("30d"), Retention::Days(30));
        assert_eq!(Retention::from_setting("forever"), Retention::Forever);
        assert_eq!(Retention::from_setting("garbage"), Retention::Days(30));
    }

    #[test]
    fn never_keeps_audio_only_for_rows_that_did_not_finish() {
        assert!(!Retention::Never.keeps("completed"));
        assert!(Retention::Never.keeps("failed"));
        assert!(Retention::Never.keeps("cancelled"));
        assert!(Retention::Days(30).keeps("completed"));
        assert!(Retention::Forever.keeps("completed"));
    }

    #[test]
    fn the_first_sweep_is_due_at_start_and_the_next_one_24_hours_later() {
        let mut schedule = Schedule::default();
        let start = 1_000_000_000_000;
        assert_eq!(schedule.wait(start), Duration::ZERO);

        schedule.ran(start);
        assert_eq!(schedule.wait(start), SWEEP_INTERVAL);
        let almost = start + DAY_MS - 1;
        assert_eq!(schedule.wait(almost), Duration::from_millis(1));
        assert_eq!(schedule.wait(start + DAY_MS), Duration::ZERO);

        schedule.ran(start + DAY_MS);
        assert_eq!(
            schedule.wait(start + DAY_MS + 5_000),
            SWEEP_INTERVAL - Duration::from_secs(5)
        );
        assert_eq!(schedule.wait(start + 2 * DAY_MS), Duration::ZERO);
    }

    #[test]
    fn a_clock_that_moves_back_makes_a_sweep_due_again() {
        let mut schedule = Schedule::default();
        schedule.ran(10 * DAY_MS);
        assert_eq!(schedule.wait(3 * DAY_MS), Duration::ZERO);
    }
}
