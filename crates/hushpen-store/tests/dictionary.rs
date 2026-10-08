use hushpen_core::dictionary::Refusal;
use hushpen_store::db::Database;
use hushpen_store::dictionary::{self, DictionaryError};
use std::path::Path;

fn open(path: &Path) -> Database {
    Database::open(&path.join("history").join("history.db")).unwrap()
}

fn rows(db: &Database) -> Vec<(String, Option<String>)> {
    let mut stmt = db
        .connection()
        .prepare("SELECT phrase, heard_as FROM dictionary_entry ORDER BY id")
        .unwrap();
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn a_new_database_has_no_entries() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(dictionary::list(&open(tmp.path())).unwrap().is_empty());
}

#[test]
fn a_word_has_no_heard_as_and_a_replacement_keeps_both_texts() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let word = dictionary::add(&db, "Zyxtrel", None).unwrap();
    let replacement = dictionary::add(&db, "Foxtrel", Some("fox")).unwrap();
    assert!(word.id < replacement.id);
    assert_eq!(
        rows(&db),
        [
            ("Zyxtrel".to_owned(), None),
            ("Foxtrel".to_owned(), Some("fox".to_owned()))
        ]
    );
    assert_eq!(dictionary::list(&db).unwrap(), [word, replacement]);
}

#[test]
fn entries_survive_closing_and_opening_the_database() {
    let tmp = tempfile::tempdir().unwrap();
    {
        let db = open(tmp.path());
        dictionary::add(&db, "Zyxtrel", None).unwrap();
        dictionary::add(&db, "café-ß-🎤", Some("dog")).unwrap();
    }
    let db = open(tmp.path());
    let list = dictionary::list(&db).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].phrase, "Zyxtrel");
    assert_eq!(list[0].heard_as, None);
    assert_eq!(list[1].phrase, "café-ß-🎤");
    assert_eq!(list[1].heard_as.as_deref(), Some("dog"));
}

#[test]
fn an_empty_phrase_is_refused_and_nothing_is_saved() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    for phrase in ["", "   ", "\t"] {
        let error = dictionary::add(&db, phrase, Some("fox")).unwrap_err();
        assert!(matches!(
            error,
            DictionaryError::Refused(Refusal::EmptyPhrase)
        ));
    }
    assert!(rows(&db).is_empty());
}

#[test]
fn a_duplicate_phrase_is_refused_in_any_case() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    dictionary::add(&db, "fox", None).unwrap();
    let error = dictionary::add(&db, "FOX", None).unwrap_err();
    assert!(matches!(
        error,
        DictionaryError::Refused(Refusal::DuplicatePhrase(_))
    ));
    assert!(error.to_string().contains("already in the dictionary"));
    assert_eq!(rows(&db).len(), 1);
}

#[test]
fn phrases_are_trimmed_and_a_blank_heard_as_is_none() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let entry = dictionary::add(&db, "  New   York ", Some("  ")).unwrap();
    assert_eq!(entry.phrase, "New York");
    assert_eq!(entry.heard_as, None);
    assert_eq!(rows(&db), [("New York".to_owned(), None)]);
}

#[test]
fn an_edit_changes_the_row_and_keeps_its_id() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let entry = dictionary::add(&db, "Foxtrel", Some("fox")).unwrap();
    let edited = dictionary::update(&db, entry.id, "Vixen", Some("fox")).unwrap();
    assert_eq!(edited.id, entry.id);
    assert_eq!(rows(&db), [("Vixen".to_owned(), Some("fox".to_owned()))]);
    dictionary::update(&db, entry.id, "Vixen", None).unwrap();
    assert_eq!(rows(&db), [("Vixen".to_owned(), None)]);
}

#[test]
fn an_edit_may_not_take_another_entrys_phrase() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    dictionary::add(&db, "Alpha", None).unwrap();
    let beta = dictionary::add(&db, "Beta", None).unwrap();
    let error = dictionary::update(&db, beta.id, "alpha", None).unwrap_err();
    assert!(matches!(
        error,
        DictionaryError::Refused(Refusal::DuplicatePhrase(_))
    ));
    assert_eq!(rows(&db)[1].0, "Beta");
}

#[test]
fn an_edit_of_a_missing_entry_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let error = dictionary::update(&db, 99, "Alpha", None).unwrap_err();
    assert!(matches!(error, DictionaryError::Missing));
    assert!(rows(&db).is_empty());
}

#[test]
fn delete_removes_the_entry_and_reports_a_missing_one() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let entry = dictionary::add(&db, "Foxtrel", Some("fox")).unwrap();
    dictionary::delete(&db, entry.id).unwrap();
    assert!(rows(&db).is_empty());
    assert!(matches!(
        dictionary::delete(&db, entry.id),
        Err(DictionaryError::Missing)
    ));
}

#[test]
fn a_deleted_phrase_can_be_added_again() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let entry = dictionary::add(&db, "fox", None).unwrap();
    dictionary::delete(&db, entry.id).unwrap();
    dictionary::add(&db, "FOX", None).unwrap();
    assert_eq!(rows(&db), [("FOX".to_owned(), None)]);
}

#[test]
fn a_database_from_a_newer_build_cannot_be_changed() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history").join("history.db");
    drop(open(tmp.path()));
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    drop(conn);
    let db = Database::open(&path).unwrap();
    assert!(db.read_only());
    assert!(matches!(
        dictionary::add(&db, "Alpha", None),
        Err(DictionaryError::Store(_))
    ));
    assert!(dictionary::list(&db).unwrap().is_empty());
}
