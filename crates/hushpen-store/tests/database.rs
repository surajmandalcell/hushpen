use hushpen_store::db::{Database, MIGRATIONS, SCHEMA_VERSION, check_fts5_trigram};
use hushpen_store::history::{self, Row, Segment};
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

#[test]
fn a_checkpoint_folds_the_wal_into_the_database_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history").join("history.db");
    let db = Database::open(&path).unwrap();
    db.connection()
        .execute(
            "INSERT INTO transcript (id, created_at, kind, status, duration_ms) VALUES ('one', 1, 'dictation', 'completed', 5)",
            [],
        )
        .unwrap();
    let wal = tmp.path().join("history").join("history.db-wal");
    assert!(wal.exists(), "a write in WAL mode leaves a journal");

    db.checkpoint().unwrap();

    let wal_bytes = std::fs::metadata(&wal).map(|meta| meta.len()).unwrap_or(0);
    assert_eq!(wal_bytes, 0, "the journal is empty after the checkpoint");
    // The main file alone now holds the row, so a plain copy is complete.
    let bytes = std::fs::read(&path).unwrap();
    let copy = tmp.path().join("copy.db");
    std::fs::write(&copy, &bytes).unwrap();
    let check = Connection::open(&copy).unwrap();
    let count: i64 = check
        .query_row("SELECT COUNT(*) FROM transcript", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn version_2_keeps_every_older_row_and_takes_read_aloud_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history").join("history.db");
    let dictation = Row {
        id: "01".to_owned(),
        created_at: 1,
        kind: "dictation".to_owned(),
        status: "completed".to_owned(),
        duration_ms: 1_200,
        model_id: Some("tiny.en".to_owned()),
        raw_text: Some("un cafe noir".to_owned()),
        final_text: Some("Un café noir".to_owned()),
        insert_outcome: Some("pasted".to_owned()),
        target_app: Some("Notes".to_owned()),
        audio_path: Some("audio/01.wav".to_owned()),
        ..Row::default()
    };
    let segments = [Segment {
        idx: 0,
        start_ms: 0,
        end_ms: 1_200,
        text: "Un café noir".to_owned(),
    }];
    let import = Row {
        id: "02".to_owned(),
        created_at: 2,
        kind: "import".to_owned(),
        final_text: Some("Minutes of the meeting".to_owned()),
        source_name: Some("meeting.m4a".to_owned()),
        ..dictation.clone()
    };
    let read = Row {
        id: "03".to_owned(),
        created_at: 3,
        kind: "read".to_owned(),
        status: "completed".to_owned(),
        final_text: Some("Read this aloud".to_owned()),
        model_id: Some("chatterbox-turbo".to_owned()),
        source_name: Some("Narrator".to_owned()),
        ..Row::default()
    };
    let old = Database::open_with_migrations(&path, &MIGRATIONS[..1]).unwrap();
    history::save(&old, &dictation, &segments).unwrap();
    history::save(&old, &import, &[]).unwrap();
    assert!(
        history::save(&old, &read, &[]).is_err(),
        "version 1 has no read rows"
    );
    drop(old);

    let db = Database::open(&path).unwrap();
    assert_eq!(db.user_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(history::get(&db, "01").unwrap(), Some(dictation));
    assert_eq!(history::get(&db, "02").unwrap(), Some(import));
    assert_eq!(history::segments(&db, "01").unwrap(), segments);
    let found = |query: &str| -> Vec<String> {
        history::page(&db, query, None, 10)
            .unwrap()
            .rows
            .into_iter()
            .map(|row| row.id)
            .collect()
    };
    assert_eq!(
        found("cafe"),
        ["01"],
        "the search index still finds old rows"
    );

    history::save(&db, &read, &[]).unwrap();
    assert_eq!(found("narrator"), ["03"], "a new row goes into the index");
    let keys: i64 = db
        .connection()
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(keys, 1);
    history::delete(&db, "01").unwrap();
    assert!(
        history::segments(&db, "01").unwrap().is_empty(),
        "the segments still belong to their row"
    );
    let backups = std::fs::read_dir(tmp.path().join("history").join("backups"))
        .unwrap()
        .count();
    assert_eq!(backups, 1);
}
