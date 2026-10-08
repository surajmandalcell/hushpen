//! `history/history.db`: SQLite in WAL mode, versioned with `user_version`.
//!
//! `MIGRATIONS[n]` takes a database from version `n` to `n + 1`. Each runs in
//! one transaction together with its `user_version` bump. A database that
//! needs a migration is copied to `history/backups/` first. A database with a
//! newer `user_version` than this build knows opens read-only
//! (`HISTORY_NEWER_SCHEMA`).

use crate::{Error, Result, time};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SCHEMA_V1: &str = "
CREATE TABLE transcript (
  id TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('dictation','command','import')),
  status TEXT NOT NULL CHECK (status IN ('completed','cancelled','failed')),
  error_code TEXT,
  duration_ms INTEGER NOT NULL,
  model_id TEXT,
  language_requested TEXT,
  language_detected TEXT,
  prompt TEXT,
  raw_text TEXT,
  rule_text TEXT,
  llm_text TEXT,
  llm_outcome TEXT,
  final_text TEXT,
  instruction TEXT,
  insert_outcome TEXT,
  target_app TEXT,
  source_name TEXT,
  audio_path TEXT,
  audio_removed_at INTEGER
);
CREATE INDEX transcript_created ON transcript(created_at DESC);
CREATE TABLE segment (
  transcript_id TEXT NOT NULL REFERENCES transcript(id) ON DELETE CASCADE,
  idx INTEGER NOT NULL,
  start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL,
  text TEXT NOT NULL,
  PRIMARY KEY (transcript_id, idx)
);
CREATE TABLE dictionary_entry (
  id INTEGER PRIMARY KEY,
  phrase TEXT NOT NULL UNIQUE COLLATE NOCASE,
  heard_as TEXT,
  created_at INTEGER NOT NULL
);
CREATE VIRTUAL TABLE transcript_fts USING fts5(
  final_text, source_name,
  content='transcript', content_rowid='rowid',
  tokenize='trigram remove_diacritics 1'
);
CREATE TRIGGER transcript_fts_insert AFTER INSERT ON transcript BEGIN
  INSERT INTO transcript_fts(rowid, final_text, source_name)
  VALUES (new.rowid, new.final_text, new.source_name);
END;
CREATE TRIGGER transcript_fts_delete AFTER DELETE ON transcript BEGIN
  INSERT INTO transcript_fts(transcript_fts, rowid, final_text, source_name)
  VALUES ('delete', old.rowid, old.final_text, old.source_name);
END;
CREATE TRIGGER transcript_fts_update AFTER UPDATE ON transcript BEGIN
  INSERT INTO transcript_fts(transcript_fts, rowid, final_text, source_name)
  VALUES ('delete', old.rowid, old.final_text, old.source_name);
  INSERT INTO transcript_fts(rowid, final_text, source_name)
  VALUES (new.rowid, new.final_text, new.source_name);
END;
";

const MIGRATIONS: &[&str] = &[SCHEMA_V1];

/// The `user_version` this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// What the start-up log line reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub sqlite_version: String,
    pub journal_mode: String,
    pub user_version: i64,
    pub fts5_trigram_ok: bool,
    pub read_only: bool,
}

pub struct Database {
    conn: Connection,
    report: Report,
    path: PathBuf,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_migrations(path, MIGRATIONS)
    }

    pub fn open_with_migrations(path: &Path, migrations: &[&str]) -> Result<Self> {
        let sqlite_version = check_fts5_trigram()?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| Error::io(format!("could not create {}", dir.display()), e))?;
        }
        let target = migrations.len() as i64;
        let conn = Connection::open(path)?;
        let found: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if found > target {
            drop(conn);
            let conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            return Ok(Self::finish(conn, path, sqlite_version, found, true));
        }

        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        if found > 0 && found < target {
            back_up(&conn, path, found)?;
        }
        for (version, sql) in migrations.iter().enumerate().skip(found as usize) {
            migrate(&conn, sql, version as i64 + 1)?;
        }
        Ok(Self::finish(conn, path, sqlite_version, target, false))
    }

    fn finish(
        conn: Connection,
        path: &Path,
        sqlite_version: String,
        user_version: i64,
        read_only: bool,
    ) -> Self {
        let journal_mode = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap_or_default();
        Self {
            conn,
            path: path.to_path_buf(),
            report: Report {
                sqlite_version,
                journal_mode,
                user_version,
                fts5_trigram_ok: true,
                read_only,
            },
        }
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn read_only(&self) -> bool {
        self.report.read_only
    }

    /// True when the database file or its folder has no write permission. A connection that was
    /// opened before the permission changed would still write, so callers ask first.
    pub fn is_locked_read_only(&self) -> bool {
        let readonly =
            |path: &Path| fs::metadata(path).is_ok_and(|meta| meta.permissions().readonly());
        readonly(&self.path) || self.path.parent().is_some_and(readonly)
    }

    pub fn user_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    pub fn report(&self) -> &Report {
        &self.report
    }
}

fn migrate(conn: &Connection, sql: &str, version: i64) -> Result<()> {
    let transaction = format!("BEGIN IMMEDIATE;\n{sql}\nPRAGMA user_version = {version};\nCOMMIT;");
    if let Err(error) = conn.execute_batch(&transaction) {
        let _ = conn.execute_batch("ROLLBACK");
        return Err(error.into());
    }
    Ok(())
}

fn back_up(conn: &Connection, db_path: &Path, from_version: i64) -> Result<()> {
    let dir = db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups");
    fs::create_dir_all(&dir)
        .map_err(|e| Error::io(format!("could not create {}", dir.display()), e))?;
    let target: PathBuf = dir.join(format!(
        "history-v{from_version}-{}.db",
        time::now_unix_ms()
    ));
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])?;
    Ok(())
}

/// Proves the bundled SQLite can build a `trigram remove_diacritics 1` index
/// and that it finds "Café" for `cafe`. Returns the SQLite version.
pub fn check_fts5_trigram() -> Result<String> {
    let fail = |reason: String| Error::Fts5TrigramUnavailable(reason);
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE VIRTUAL TABLE trigram_check USING fts5(t, tokenize='trigram remove_diacritics 1');
         INSERT INTO trigram_check(t) VALUES ('Café');",
    )
    .map_err(|e| fail(e.to_string()))?;
    let hits: i64 = conn
        .query_row(
            "SELECT count(*) FROM trigram_check WHERE trigram_check MATCH '\"cafe\"'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| fail(e.to_string()))?;
    if hits != 1 {
        return Err(fail("'cafe' did not match 'Café'".into()));
    }
    Ok(conn.query_row("SELECT sqlite_version()", [], |row| row.get(0))?)
}
