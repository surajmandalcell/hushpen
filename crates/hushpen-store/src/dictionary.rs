//! The personal dictionary in the `dictionary_entry` table of the history database.

use crate::db::Database;
use crate::{Error, time};
use hushpen_core::dictionary::{Entry, Refusal, validate};
use rusqlite::params;
use std::fmt;

#[derive(Debug)]
pub enum DictionaryError {
    /// The phrase was not saved, and the user can fix it.
    Refused(Refusal),
    /// The entry is gone, for example deleted in the meantime.
    Missing,
    Store(Error),
}

impl fmt::Display for DictionaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => f.write_str(&refusal.message()),
            Self::Missing => f.write_str("That entry is no longer in the dictionary."),
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DictionaryError {}

impl From<rusqlite::Error> for DictionaryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl From<Refusal> for DictionaryError {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

type Result<T> = std::result::Result<T, DictionaryError>;

/// Every entry, oldest first.
pub fn list(db: &Database) -> Result<Vec<Entry>> {
    let mut statement = db
        .connection()
        .prepare("SELECT id, phrase, heard_as FROM dictionary_entry ORDER BY id")?;
    let entries = statement
        .query_map([], |row| {
            Ok(Entry {
                id: row.get(0)?,
                phrase: row.get(1)?,
                heard_as: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(entries)
}

/// Saves a new entry. The texts are trimmed; a blank `heard_as` makes a prompt word.
pub fn add(db: &Database, phrase: &str, heard_as: Option<&str>) -> Result<Entry> {
    let (phrase, heard_as) = validate(phrase, heard_as, &list(db)?, None)?;
    db.connection().execute(
        "INSERT INTO dictionary_entry (phrase, heard_as, created_at) VALUES (?1, ?2, ?3)",
        params![phrase, heard_as, time::now_unix_ms() as i64],
    )?;
    Ok(Entry {
        id: db.connection().last_insert_rowid(),
        phrase,
        heard_as,
    })
}

/// Changes both texts of an entry and keeps its place in the list.
pub fn update(db: &Database, id: i64, phrase: &str, heard_as: Option<&str>) -> Result<Entry> {
    let existing = list(db)?;
    if !existing.iter().any(|entry| entry.id == id) {
        return Err(DictionaryError::Missing);
    }
    let (phrase, heard_as) = validate(phrase, heard_as, &existing, Some(id))?;
    db.connection().execute(
        "UPDATE dictionary_entry SET phrase = ?1, heard_as = ?2 WHERE id = ?3",
        params![phrase, heard_as, id],
    )?;
    Ok(Entry {
        id,
        phrase,
        heard_as,
    })
}

pub fn delete(db: &Database, id: i64) -> Result<()> {
    let removed = db
        .connection()
        .execute("DELETE FROM dictionary_entry WHERE id = ?1", [id])?;
    if removed == 0 {
        return Err(DictionaryError::Missing);
    }
    Ok(())
}
