//! The dictation history in the `transcript` and `segment` tables of the history database.
//!
//! A page is 50 rows by the callers' choice, newest first. The cursor is the `(created_at, id)`
//! of the last row of the page before, so a page never repeats or skips a row, also while rows
//! arrive at the top. Search uses the trigram index for queries of 3 characters or more and a
//! `LIKE` scan below that, because a trigram index cannot hold less than a trigram.

use crate::db::Database;
use crate::{Error, Result, time};
use rusqlite::types::Value;
use rusqlite::{OptionalExtension as _, Row as SqlRow, params, params_from_iter};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher as _, Hasher as _};
use std::io;

const COLUMNS: &str = "id, created_at, kind, status, error_code, duration_ms, model_id, \
    language_requested, language_detected, prompt, raw_text, rule_text, llm_text, llm_outcome, \
    final_text, instruction, insert_outcome, target_app, source_name, audio_path, audio_removed_at";

/// One `transcript` row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub id: String,
    /// Unix milliseconds.
    pub created_at: i64,
    pub kind: String,
    pub status: String,
    pub error_code: Option<String>,
    pub duration_ms: i64,
    pub model_id: Option<String>,
    pub language_requested: Option<String>,
    pub language_detected: Option<String>,
    pub prompt: Option<String>,
    pub raw_text: Option<String>,
    pub rule_text: Option<String>,
    pub llm_text: Option<String>,
    pub llm_outcome: Option<String>,
    pub final_text: Option<String>,
    pub instruction: Option<String>,
    pub insert_outcome: Option<String>,
    pub target_app: Option<String>,
    pub source_name: Option<String>,
    /// Relative to the data folder: `audio/<id>.wav`.
    pub audio_path: Option<String>,
    pub audio_removed_at: Option<i64>,
}

/// One `segment` row: a piece of the text with its time span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub idx: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// A row with its segments: what a delete takes out and an undo puts back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub row: Row,
    pub segments: Vec<Segment>,
}

/// Where the next page starts: after this row in the newest-first order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub created_at: i64,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub rows: Vec<Row>,
    /// `None` on the last page.
    pub next: Option<Cursor>,
}

/// The new result of a reprocess. The row becomes `completed` with no error, and the AI fields
/// are cleared because the old AI text belonged to the old words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reprocessed {
    pub model_id: Option<String>,
    pub language_detected: Option<String>,
    pub prompt: Option<String>,
    pub raw_text: String,
    pub rule_text: String,
    pub final_text: String,
    pub segments: Vec<Segment>,
}

fn row_of(row: &SqlRow<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: row.get(0)?,
        created_at: row.get(1)?,
        kind: row.get(2)?,
        status: row.get(3)?,
        error_code: row.get(4)?,
        duration_ms: row.get(5)?,
        model_id: row.get(6)?,
        language_requested: row.get(7)?,
        language_detected: row.get(8)?,
        prompt: row.get(9)?,
        raw_text: row.get(10)?,
        rule_text: row.get(11)?,
        llm_text: row.get(12)?,
        llm_outcome: row.get(13)?,
        final_text: row.get(14)?,
        instruction: row.get(15)?,
        insert_outcome: row.get(16)?,
        target_app: row.get(17)?,
        source_name: row.get(18)?,
        audio_path: row.get(19)?,
        audio_removed_at: row.get(20)?,
    })
}

/// A sortable 26-character id: 10 characters of time, 16 of randomness (Crockford base32).
pub fn new_id(unix_ms: u64) -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut id = String::with_capacity(26);
    for shift in (0..10).rev() {
        id.push(ALPHABET[((unix_ms >> (shift * 5)) & 31) as usize] as char);
    }
    // `RandomState` is seeded from the operating system, one seed for each value.
    let random = |state: RandomState| {
        let mut hasher = state.build_hasher();
        hasher.write_u64(unix_ms);
        hasher.finish()
    };
    let bits = [random(RandomState::new()), random(RandomState::new())];
    for index in 0..16 {
        let word = bits[index / 8];
        id.push(ALPHABET[((word >> ((index % 8) * 5)) & 31) as usize] as char);
    }
    id
}

pub fn now_ms() -> i64 {
    time::now_unix_ms() as i64
}

/// Fails before any write when the file or its folder is read-only, so a history that cannot
/// be written is reported the same way on every platform and for every user.
fn ensure_writable(db: &Database) -> Result<()> {
    if db.read_only() || db.is_locked_read_only() {
        return Err(Error::io(
            "the history database is read-only",
            io::Error::from(io::ErrorKind::PermissionDenied),
        ));
    }
    Ok(())
}

/// Saves one row and its segments in one step.
pub fn save(db: &Database, row: &Row, segments: &[Segment]) -> Result<()> {
    ensure_writable(db)?;
    in_savepoint(db, |conn| {
        insert(conn, row)?;
        insert_segments(conn, &row.id, segments)
    })
}

fn in_savepoint<T>(
    db: &Database,
    work: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T>,
) -> Result<T> {
    let conn = db.connection();
    conn.execute_batch("SAVEPOINT history_write")?;
    match work(conn) {
        Ok(value) => {
            conn.execute_batch("RELEASE history_write")?;
            Ok(value)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK TO history_write; RELEASE history_write");
            Err(error.into())
        }
    }
}

fn insert(conn: &rusqlite::Connection, row: &Row) -> rusqlite::Result<()> {
    conn.execute(
        &format!(
            "INSERT INTO transcript ({COLUMNS}) VALUES \
             (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)"
        ),
        params![
            row.id,
            row.created_at,
            row.kind,
            row.status,
            row.error_code,
            row.duration_ms,
            row.model_id,
            row.language_requested,
            row.language_detected,
            row.prompt,
            row.raw_text,
            row.rule_text,
            row.llm_text,
            row.llm_outcome,
            row.final_text,
            row.instruction,
            row.insert_outcome,
            row.target_app,
            row.source_name,
            row.audio_path,
            row.audio_removed_at,
        ],
    )?;
    Ok(())
}

fn insert_segments(
    conn: &rusqlite::Connection,
    id: &str,
    segments: &[Segment],
) -> rusqlite::Result<()> {
    let mut statement = conn.prepare_cached(
        "INSERT INTO segment (transcript_id, idx, start_ms, end_ms, text) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for segment in segments {
        statement.execute(params![
            id,
            segment.idx,
            segment.start_ms,
            segment.end_ms,
            segment.text
        ])?;
    }
    Ok(())
}

pub fn get(db: &Database, id: &str) -> Result<Option<Row>> {
    Ok(db
        .connection()
        .query_row(
            &format!("SELECT {COLUMNS} FROM transcript WHERE id = ?1"),
            [id],
            row_of,
        )
        .optional()?)
}

pub fn segments(db: &Database, id: &str) -> Result<Vec<Segment>> {
    let mut statement = db.connection().prepare_cached(
        "SELECT idx, start_ms, end_ms, text FROM segment WHERE transcript_id = ?1 ORDER BY idx",
    )?;
    let rows = statement
        .query_map([id], |row| {
            Ok(Segment {
                idx: row.get(0)?,
                start_ms: row.get(1)?,
                end_ms: row.get(2)?,
                text: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn count(db: &Database) -> Result<i64> {
    Ok(db
        .connection()
        .query_row("SELECT count(*) FROM transcript", [], |row| row.get(0))?)
}

/// The newest row, or `None` for an empty history.
pub fn newest(db: &Database) -> Result<Option<Row>> {
    Ok(db
        .connection()
        .query_row(
            &format!("SELECT {COLUMNS} FROM transcript ORDER BY created_at DESC, id DESC LIMIT 1"),
            [],
            row_of,
        )
        .optional()?)
}

/// One page of up to `limit` rows, newest first. A blank `query` lists every row.
pub fn page(db: &Database, query: &str, after: Option<&Cursor>, limit: usize) -> Result<Page> {
    let mut sql = format!("SELECT {COLUMNS} FROM transcript WHERE 1 = 1");
    let mut args: Vec<Value> = Vec::new();
    let query = query.trim();
    if !query.is_empty() {
        if query.chars().count() >= 3 {
            sql.push_str(
                " AND rowid IN (SELECT rowid FROM transcript_fts WHERE transcript_fts MATCH ?)",
            );
            args.push(Value::Text(format!("\"{}\"", query.replace('"', "\"\""))));
        } else {
            sql.push_str(" AND (final_text LIKE ? ESCAPE '\\' OR source_name LIKE ? ESCAPE '\\')");
            let pattern = Value::Text(format!("%{}%", escape_like(query)));
            args.push(pattern.clone());
            args.push(pattern);
        }
    }
    if let Some(cursor) = after {
        sql.push_str(" AND (created_at, id) < (?, ?)");
        args.push(Value::Integer(cursor.created_at));
        args.push(Value::Text(cursor.id.clone()));
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
    args.push(Value::Integer(limit as i64 + 1));

    let mut statement = db.connection().prepare_cached(&sql)?;
    let mut rows = statement
        .query_map(params_from_iter(args), row_of)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let next = (rows.len() > limit).then(|| {
        rows.truncate(limit);
        rows.last().map(|row| Cursor {
            created_at: row.created_at,
            id: row.id.clone(),
        })
    });
    Ok(Page {
        rows,
        next: next.flatten(),
    })
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Replaces the text of a row with a new result. The id, time, target app, and audio stay.
pub fn reprocess(db: &Database, id: &str, result: &Reprocessed) -> Result<()> {
    ensure_writable(db)?;
    in_savepoint(db, |conn| {
        let changed = conn.execute(
            "UPDATE transcript SET status = 'completed', error_code = NULL, model_id = ?2, \
             language_detected = ?3, prompt = ?4, raw_text = ?5, rule_text = ?6, llm_text = NULL, \
             llm_outcome = NULL, final_text = ?7 WHERE id = ?1",
            params![
                id,
                result.model_id,
                result.language_detected,
                result.prompt,
                result.raw_text,
                result.rule_text,
                result.final_text,
            ],
        )?;
        if changed == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        conn.execute("DELETE FROM segment WHERE transcript_id = ?1", [id])?;
        insert_segments(conn, id, &result.segments)
    })
}

/// Removes a row and its segments and returns them for an undo. `None` when it is not there.
pub fn delete(db: &Database, id: &str) -> Result<Option<Stored>> {
    ensure_writable(db)?;
    let Some(row) = get(db, id)? else {
        return Ok(None);
    };
    let segments = segments(db, id)?;
    in_savepoint(db, |conn| {
        conn.execute("DELETE FROM transcript WHERE id = ?1", [id])
    })?;
    Ok(Some(Stored { row, segments }))
}

/// Puts a deleted row back with its id, time, and segments.
pub fn restore(db: &Database, stored: &Stored) -> Result<()> {
    save(db, &stored.row, &stored.segments)
}

/// Removes every row and returns the audio paths they had.
pub fn clear(db: &Database) -> Result<Vec<String>> {
    ensure_writable(db)?;
    let paths = audio_paths(db)?;
    in_savepoint(db, |conn| conn.execute("DELETE FROM transcript", []))?;
    Ok(paths)
}

/// The audio path of every row that still has audio.
pub fn audio_paths(db: &Database) -> Result<Vec<String>> {
    let mut statement = db.connection().prepare(
        "SELECT audio_path FROM transcript WHERE audio_path IS NOT NULL ORDER BY created_at",
    )?;
    let paths = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(paths)
}
