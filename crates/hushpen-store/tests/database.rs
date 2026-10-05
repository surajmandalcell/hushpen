use hushpen_store::db::{Database, SCHEMA_VERSION, check_fts5_trigram};
use rusqlite::Connection;

fn fts_matches(conn: &Connection, query: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT t FROM docs WHERE docs MATCH ?1")
        .unwrap();
    stmt.query_map([query], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn trigram_index_folds_case_and_diacritics_and_matches_substrings() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE docs USING fts5(t, tokenize='trigram remove_diacritics 1');
         INSERT INTO docs(t) VALUES ('Café'), ('the quick brown fox');",
    )
    .unwrap();
    assert_eq!(fts_matches(&conn, "\"cafe\""), ["Café"]);
    assert_eq!(fts_matches(&conn, "\"CAFÉ\""), ["Café"]);
    assert_eq!(fts_matches(&conn, "\"rown f\""), ["the quick brown fox"]);
}

#[test]
fn the_startup_check_passes_and_names_the_sqlite_version() {
    let version = check_fts5_trigram().unwrap();
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap());
    let (major, minor) = (parts.next().unwrap(), parts.next().unwrap());
    assert!((major, minor) >= (3, 45), "{version}");
}

#[test]
fn a_new_database_is_wal_with_the_current_schema_version() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history").join("history.db");
    let db = Database::open(&path).unwrap();
    let mode: String = db
        .connection()
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    assert_eq!(db.user_version().unwrap(), SCHEMA_VERSION);
    assert!(!db.read_only());
    assert!(db.report().fts5_trigram_ok);
    drop(db);

    let db = Database::open(&path).unwrap();
    assert_eq!(db.user_version().unwrap(), SCHEMA_VERSION);
    assert!(!tmp.path().join("history").join("backups").exists());
}

#[test]
fn the_history_schema_indexes_final_text_with_trigrams() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Database::open(&tmp.path().join("history.db")).unwrap();
    let conn = db.connection();
    conn.execute(
        "INSERT INTO transcript(id, created_at, kind, status, duration_ms, final_text)
         VALUES ('01', 1, 'dictation', 'completed', 10, 'Un café noir')",
        [],
    )
    .unwrap();
    let hits: i64 = conn
        .query_row(
            "SELECT count(*) FROM transcript_fts WHERE transcript_fts MATCH '\"cafe\"'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1);
    conn.execute("DELETE FROM transcript WHERE id = '01'", [])
        .unwrap();
    let hits: i64 = conn
        .query_row(
            "SELECT count(*) FROM transcript_fts WHERE transcript_fts MATCH '\"cafe\"'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hits, 0);
}

#[test]
fn a_migration_runs_once_and_backs_up_the_old_database() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history").join("history.db");
    let v1: &[&str] = &["CREATE TABLE a(x INTEGER);"];
    let v2: &[&str] = &[
        "CREATE TABLE a(x INTEGER);",
        "ALTER TABLE a ADD COLUMN y TEXT;",
    ];

    let db = Database::open_with_migrations(&path, v1).unwrap();
    db.connection()
        .execute("INSERT INTO a(x) VALUES (7)", [])
        .unwrap();
    drop(db);
    assert!(!tmp.path().join("history").join("backups").exists());

    let db = Database::open_with_migrations(&path, v2).unwrap();
    assert_eq!(db.user_version().unwrap(), 2);
    let x: i64 = db
        .connection()
        .query_row("SELECT x FROM a", [], |r| r.get(0))
        .unwrap();
    assert_eq!(x, 7);
    drop(db);

    let backups: Vec<_> = std::fs::read_dir(tmp.path().join("history").join("backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(backups.len(), 1);
    let old = Connection::open(&backups[0]).unwrap();
    let version: i64 = old
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 1);

    Database::open_with_migrations(&path, v2).unwrap();
    let count = std::fs::read_dir(tmp.path().join("history").join("backups"))
        .unwrap()
        .count();
    assert_eq!(count, 1, "no backup when nothing migrates");
}

#[test]
fn a_failed_migration_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history.db");
    let good: &[&str] = &["CREATE TABLE a(x INTEGER);"];
    drop(Database::open_with_migrations(&path, good).unwrap());

    let bad: &[&str] = &["CREATE TABLE a(x INTEGER);", "CREATE TABLE b(y); NOT SQL;"];
    assert!(Database::open_with_migrations(&path, bad).is_err());

    let db = Database::open_with_migrations(&path, good).unwrap();
    assert_eq!(db.user_version().unwrap(), 1);
    let tables: i64 = db
        .connection()
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'b'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0);
}

#[test]
fn a_newer_database_opens_read_only() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history.db");
    let newer: &[&str] = &["CREATE TABLE a(x INTEGER);", "CREATE TABLE b(y INTEGER);"];
    drop(Database::open_with_migrations(&path, newer).unwrap());

    let older: &[&str] = &["CREATE TABLE a(x INTEGER);"];
    let db = Database::open_with_migrations(&path, older).unwrap();
    assert!(db.read_only());
    assert_eq!(db.user_version().unwrap(), 2);
    assert!(
        db.connection()
            .execute("INSERT INTO a(x) VALUES (1)", [])
            .is_err()
    );
}
