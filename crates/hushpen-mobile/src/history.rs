//! The phone's own dictation history: the shared `history.db`, written and read on this phone only.
//!
//! The shared table has no column for "took" (the seconds from the end of speech to the text),
//! so the phone keeps it in a small table of its own that goes with the row.

use crate::{MobileError, lock};
use hushpen_store::db::Database;
use hushpen_store::history::{self, Cursor, Row};
use rusqlite::params;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const PHONE_RUN: &str = "CREATE TABLE IF NOT EXISTS phone_run (
  id TEXT PRIMARY KEY REFERENCES transcript(id) ON DELETE CASCADE,
  took_ms INTEGER NOT NULL
)";

/// A finished dictation to keep.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NewHistoryEntry {
    pub created_ms: i64,
    pub spoken_ms: u64,
    pub took_ms: u64,
    /// The speech engine's words, before cleanup. Empty when unknown.
    pub raw_text: String,
    pub text: String,
    pub model_id: String,
    pub language: String,
    /// The cleanup outcome key: `used`, `off`, `unavailable`, `error`, or `guard_rejected`.
    pub cleanup: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HistoryEntry {
    pub id: String,
    pub created_ms: i64,
    pub text: String,
    pub raw_text: String,
    pub spoken_ms: u64,
    pub took_ms: u64,
    pub words: u32,
    pub model_id: String,
    pub language: String,
    pub cleanup: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HistoryCursor {
    pub created_ms: i64,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HistoryPage {
    pub entries: Vec<HistoryEntry>,
    /// `None` on the last page.
    pub next: Option<HistoryCursor>,
}

/// One dictation for the numbers: when, how many words, how long the speech was, and where it ran.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HistoryActivity {
    pub created_ms: i64,
    pub words: u32,
    pub spoken_ms: u64,
    /// Empty for this phone, else the name of the device that made the text.
    pub device: String,
}

#[derive(uniffi::Object)]
pub struct History {
    db: Mutex<Database>,
}

fn words(text: &str) -> u32 {
    u32::try_from(text.split_whitespace().count()).unwrap_or(u32::MAX)
}

fn non_empty(text: &str) -> Option<String> {
    Some(text.to_owned()).filter(|text| !text.is_empty())
}

fn entry_of(row: Row, took_ms: i64) -> HistoryEntry {
    let text = row.final_text.unwrap_or_default();
    HistoryEntry {
        id: row.id,
        created_ms: row.created_at,
        words: words(&text),
        text,
        raw_text: row.raw_text.unwrap_or_default(),
        spoken_ms: row.duration_ms.max(0).cast_unsigned(),
        took_ms: took_ms.max(0).cast_unsigned(),
        model_id: row.model_id.unwrap_or_default(),
        language: row.language_detected.unwrap_or_default(),
        cleanup: row.llm_outcome.unwrap_or_default(),
    }
}

#[uniffi::export]
impl History {
    /// Opens `history/history.db` in the phone data folder, making it when it is new.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>, MobileError> {
        let data = PathBuf::from(data_dir);
        if !data.is_absolute() {
            return Err(MobileError::invalid("data folder must be absolute"));
        }
        let db = Database::open(&data.join("history/history.db")).map_err(MobileError::failed)?;
        // Deleted dictations are zeroed in the file, not just marked free.
        db.connection()
            .pragma_update(None, "secure_delete", "ON")
            .map_err(MobileError::failed)?;
        db.connection()
            .execute_batch(PHONE_RUN)
            .map_err(MobileError::failed)?;
        Ok(Arc::new(Self { db: Mutex::new(db) }))
    }

    /// Keeps one finished dictation and returns its id. Text with no words is not kept.
    pub fn save(&self, entry: NewHistoryEntry) -> Result<String, MobileError> {
        if entry.text.trim().is_empty() {
            return Err(MobileError::invalid("there is no text to keep"));
        }
        let id = history::new_id(entry.created_ms.max(0).cast_unsigned());
        let row = Row {
            id: id.clone(),
            created_at: entry.created_ms,
            kind: "dictation".into(),
            status: "completed".into(),
            duration_ms: i64::try_from(entry.spoken_ms).unwrap_or(i64::MAX),
            model_id: non_empty(&entry.model_id),
            language_detected: non_empty(&entry.language),
            raw_text: non_empty(&entry.raw_text),
            final_text: Some(entry.text),
            llm_outcome: non_empty(&entry.cleanup),
            ..Row::default()
        };
        let db = lock(&self.db);
        history::save(&db, &row, &[]).map_err(MobileError::failed)?;
        db.connection()
            .execute(
                "INSERT INTO phone_run (id, took_ms) VALUES (?1, ?2)",
                params![id, i64::try_from(entry.took_ms).unwrap_or(i64::MAX)],
            )
            .map_err(MobileError::failed)?;
        Ok(id)
    }

    /// Up to `limit` entries, newest first. A blank `query` lists all of them.
    pub fn page(
        &self,
        query: String,
        after: Option<HistoryCursor>,
        limit: u32,
    ) -> Result<HistoryPage, MobileError> {
        let db = lock(&self.db);
        let after = after.map(|cursor| Cursor {
            created_at: cursor.created_ms,
            id: cursor.id,
        });
        let page = history::page(&db, &query, after.as_ref(), limit.max(1) as usize)
            .map_err(MobileError::failed)?;
        let mut entries = Vec::with_capacity(page.rows.len());
        for row in page.rows {
            let took = took_ms(&db, &row.id)?;
            entries.push(entry_of(row, took));
        }
        Ok(HistoryPage {
            entries,
            next: page.next.map(|cursor| HistoryCursor {
                created_ms: cursor.created_at,
                id: cursor.id,
            }),
        })
    }

    pub fn get(&self, id: String) -> Result<Option<HistoryEntry>, MobileError> {
        let db = lock(&self.db);
        let Some(row) = history::get(&db, &id).map_err(MobileError::failed)? else {
            return Ok(None);
        };
        let took = took_ms(&db, &id)?;
        Ok(Some(entry_of(row, took)))
    }

    /// Every finished dictation since `since_ms`, oldest first, for the numbers.
    pub fn activity(&self, since_ms: i64) -> Result<Vec<HistoryActivity>, MobileError> {
        let db = lock(&self.db);
        let mut statement = db
            .connection()
            .prepare(
                "SELECT created_at, final_text, duration_ms, source_name FROM transcript \
                 WHERE status = 'completed' AND kind = 'dictation' AND created_at >= ?1 \
                 ORDER BY created_at",
            )
            .map_err(MobileError::failed)?;
        statement
            .query_map([since_ms], |row| {
                let text: Option<String> = row.get(1)?;
                let spoken: i64 = row.get(2)?;
                Ok(HistoryActivity {
                    created_ms: row.get(0)?,
                    words: words(&text.unwrap_or_default()),
                    spoken_ms: spoken.max(0).cast_unsigned(),
                    device: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                })
            })
            .map_err(MobileError::failed)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(MobileError::failed)
    }

    /// Removes one entry. An id that is gone is not an error.
    pub fn delete(&self, id: String) -> Result<(), MobileError> {
        history::delete(&lock(&self.db), &id)
            .map(drop)
            .map_err(MobileError::failed)
    }

    pub fn clear(&self) -> Result<(), MobileError> {
        history::clear(&lock(&self.db))
            .map(drop)
            .map_err(MobileError::failed)
    }

    /// Removes entries made before `before_ms`, for "Keep History".
    pub fn prune(&self, before_ms: i64) -> Result<(), MobileError> {
        let db = lock(&self.db);
        if db.read_only() || db.is_locked_read_only() {
            return Err(MobileError::failed("the history database is read-only"));
        }
        db.connection()
            .execute("DELETE FROM transcript WHERE created_at < ?1", [before_ms])
            .map(drop)
            .map_err(MobileError::failed)
    }
}

fn took_ms(db: &Database, id: &str) -> Result<i64, MobileError> {
    match db
        .connection()
        .query_row("SELECT took_ms FROM phone_run WHERE id = ?1", [id], |row| {
            row.get(0)
        }) {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(error) => Err(MobileError::failed(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(created_ms: i64, text: &str, took_ms: u64) -> NewHistoryEntry {
        NewHistoryEntry {
            created_ms,
            spoken_ms: 7_000,
            took_ms,
            raw_text: "raw words".into(),
            text: text.into(),
            model_id: "parakeet".into(),
            language: "en".into(),
            cleanup: "used".into(),
        }
    }

    #[test]
    fn it_keeps_pages_counts_and_removes_dictations() {
        let dir = tempfile::tempdir().unwrap();
        let history = History::new(dir.path().to_str().unwrap().into()).unwrap();
        let wipes: i64 = lock(&history.db)
            .connection()
            .query_row("PRAGMA secure_delete", [], |row| row.get(0))
            .unwrap();
        assert_eq!(wipes, 1);
        let first = history.save(entry(1_000, "one two three", 1_400)).unwrap();
        history.save(entry(2_000, "four five", 900)).unwrap();
        history.save(entry(3_000, "six", 500)).unwrap();
        assert!(history.save(entry(4_000, "  ", 0)).is_err());

        let page = history.page(String::new(), None, 2).unwrap();
        let texts: Vec<_> = page.entries.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts, ["six", "four five"]);
        let rest = history.page(String::new(), page.next, 2).unwrap();
        assert_eq!(rest.entries.len(), 1);
        assert!(rest.next.is_none());
        let found = &rest.entries[0];
        assert_eq!(
            (
                found.words,
                found.spoken_ms,
                found.took_ms,
                found.raw_text.as_str()
            ),
            (3, 7_000, 1_400, "raw words")
        );
        assert_eq!(
            history.page("two".into(), None, 10).unwrap().entries.len(),
            1
        );

        let activity = history.activity(1_500).unwrap();
        assert_eq!(
            activity
                .iter()
                .map(|a| (a.words, a.device.as_str()))
                .collect::<Vec<_>>(),
            [(2, ""), (1, "")]
        );

        history.delete(first.clone()).unwrap();
        assert!(history.get(first).unwrap().is_none());
        history.prune(3_000).unwrap();
        assert_eq!(
            history.page(String::new(), None, 10).unwrap().entries.len(),
            1
        );
        history.clear().unwrap();
        assert!(
            history
                .page(String::new(), None, 10)
                .unwrap()
                .entries
                .is_empty()
        );
        let reopened = History::new(dir.path().to_str().unwrap().into()).unwrap();
        assert!(reopened.activity(0).unwrap().is_empty());
    }
}
