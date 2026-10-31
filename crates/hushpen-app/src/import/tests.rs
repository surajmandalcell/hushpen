use super::*;
use crate::controller::testkit::{PARAKEET_ID, Rig, SMALL_ID, TURBO_ID, rig, settle};
use gpui_kit::{AppContext as _, TestAppContext};
use hushpen_core::error;
use hushpen_engine::{Failure, JobOutcome, Segment as EngineSegment, Transcription};
use hushpen_store::history as store_history;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fake {
    outcomes: Mutex<VecDeque<JobOutcome>>,
    specs: Mutex<Vec<ImportSpec>>,
    tokens: Mutex<Vec<Arc<CancelToken>>>,
    running: AtomicUsize,
    most_at_once: AtomicUsize,
    /// What each job reports before it ends.
    steps: Vec<f32>,
}

/// What the first report of each job says the file lasts.
const LENGTH_MS: u64 = 90_000;

struct Fixture {
    import: Entity<Import>,
    rig: Rig,
    fake: Arc<Fake>,
    folder: tempfile::TempDir,
}

fn done(text: &str, language: &str) -> JobOutcome {
    JobOutcome::Done(Transcription {
        text: text.into(),
        language: language.into(),
        segments: vec![
            EngineSegment {
                start_ms: 0,
                end_ms: 2_000,
                text: "first piece".into(),
            },
            EngineSegment {
                start_ms: 2_000,
                end_ms: 4_500,
                text: "second piece".into(),
            },
        ],
        audio_ms: 4_500,
        decode_ms: 20,
    })
}

fn failed(code: &str, detail: &str) -> JobOutcome {
    JobOutcome::Failed(Failure {
        code: code.into(),
        detail: detail.into(),
        reason: None,
    })
}

/// The model of the rig is the first of `files`, or `base` when there are none.
fn open_with(cx: &mut TestAppContext, files: &[&str], steps: Vec<f32>) -> Fixture {
    let rig = rig(cx, files, files.first().copied().unwrap_or("base"));
    let fake = Arc::new(Fake {
        outcomes: Mutex::default(),
        specs: Mutex::default(),
        tokens: Mutex::default(),
        running: AtomicUsize::new(0),
        most_at_once: AtomicUsize::new(0),
        steps,
    });
    let run: ImportRunner = {
        let fake = Arc::clone(&fake);
        Arc::new(move |spec, progress, token| {
            let now = fake.running.fetch_add(1, Ordering::SeqCst) + 1;
            fake.most_at_once.fetch_max(now, Ordering::SeqCst);
            fake.specs.lock().unwrap().push(spec);
            fake.tokens.lock().unwrap().push(token);
            // Like the engine child, the first report gives the length from the headers.
            progress(ImportProgress {
                fraction: 0.0,
                audio_ms: Some(LENGTH_MS),
            });
            for step in &fake.steps {
                progress(ImportProgress {
                    fraction: *step,
                    audio_ms: None,
                });
            }
            let outcome = fake
                .outcomes
                .lock()
                .unwrap()
                .pop_front()
                .expect("a queued outcome");
            fake.running.fetch_sub(1, Ordering::SeqCst);
            outcome
        })
    };
    let work = Rc::clone(&rig.work);
    let spawn: crate::models::Spawner = Rc::new(move |job| {
        work.borrow_mut().push_back(job);
        Ok(())
    });
    let clock: crate::controller::Clock = {
        let now = Rc::clone(&rig.now);
        Rc::new(move || now.get())
    };
    let import = cx.new(|cx| {
        Import::new(
            Parts {
                storage: Rc::clone(&rig.storage),
                models: rig.models.clone(),
                controller: rig.controller.clone(),
                run,
                spawn,
                clock,
            },
            cx,
        )
    });
    Fixture {
        import,
        rig,
        fake,
        folder: tempfile::tempdir().unwrap(),
    }
}

fn open(cx: &mut TestAppContext) -> Fixture {
    open_with(cx, &["base"], vec![])
}

fn file(fixture: &Fixture, name: &str) -> PathBuf {
    let path = fixture.folder.path().join(name);
    std::fs::write(&path, b"audio").unwrap();
    path
}

fn add(cx: &mut TestAppContext, fixture: &Fixture, paths: Vec<PathBuf>) -> usize {
    fixture
        .import
        .update(cx, |import, cx| import.add_paths(paths, cx))
}

fn say(fixture: &Fixture, outcome: JobOutcome) {
    fixture.fake.outcomes.lock().unwrap().push_back(outcome);
}

/// Runs the one job that is waiting for a thread, then applies its events.
fn run_one(cx: &mut TestAppContext, fixture: &Fixture) {
    cx.run_until_parked();
    let job = fixture.rig.work.borrow_mut().pop_front().expect("a job");
    job();
    cx.run_until_parked();
}

fn items(cx: &mut TestAppContext, fixture: &Fixture) -> Vec<Value> {
    let state = cx.update(|cx| fixture.import.read(cx).state_json(cx));
    state["items"].as_array().unwrap().clone()
}

fn state_of(cx: &mut TestAppContext, fixture: &Fixture) -> Vec<String> {
    items(cx, fixture)
        .iter()
        .map(|item| item["state"].as_str().unwrap().to_owned())
        .collect()
}

fn rows(fixture: &Fixture) -> Vec<store_history::Row> {
    store_history::page(&fixture.rig.storage.database, "", None, 50)
        .unwrap()
        .rows
}

fn notice(cx: &mut TestAppContext, fixture: &Fixture) -> Option<String> {
    fixture
        .import
        .read_with(cx, |import, _| import.notice().map(str::to_owned))
}

#[gpui_kit::test]
fn a_file_is_queued_run_and_saved_as_an_import_row(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let path = file(&fixture, "speech-short.wav");
    say(&fixture, done("The quick brown fox.", "en"));

    assert_eq!(add(cx, &fixture, vec![path.clone()]), 1);
    assert_eq!(state_of(cx, &fixture), ["running"]);
    run_one(cx, &fixture);

    assert_eq!(state_of(cx, &fixture), ["done"]);
    let [row] = rows(&fixture).try_into().unwrap();
    assert_eq!(row.kind, "import");
    assert_eq!(row.status, "completed");
    assert_eq!(row.source_name.as_deref(), Some("speech-short.wav"));
    assert_eq!(row.final_text.as_deref(), Some("The quick brown fox."));
    assert_eq!(row.language_detected.as_deref(), Some("en"));
    assert_eq!(row.language_requested, None);
    assert_eq!(row.model_id.as_deref(), Some("base"));
    assert_eq!(row.duration_ms, 4_500);
    assert_eq!(row.audio_path, None, "an import keeps no audio copy");
    let segments = store_history::segments(&fixture.rig.storage.database, &row.id).unwrap();
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.text.as_str()))
            .collect::<Vec<_>>(),
        [(0, 2_000, "first piece"), (2_000, 4_500, "second piece")]
    );
    let spec = fixture.fake.specs.lock().unwrap()[0].clone();
    assert_eq!(spec.path, path);
    assert_eq!(spec.language, None, "auto detects the language");
}

#[gpui_kit::test]
fn files_run_in_the_order_they_were_added_and_never_together(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let names = ["a.wav", "b.mp3", "c.flac"];
    let paths = names.iter().map(|name| file(&fixture, name)).collect();
    for text in ["one", "two", "three"] {
        say(&fixture, done(text, "en"));
    }

    assert_eq!(add(cx, &fixture, paths), 3);
    assert_eq!(state_of(cx, &fixture), ["running", "queued", "queued"]);
    assert_eq!(
        fixture.rig.work.borrow().len(),
        1,
        "one job waits for a thread"
    );
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["done", "running", "queued"]);
    run_one(cx, &fixture);
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["done", "done", "done"]);

    assert_eq!(fixture.fake.most_at_once.load(Ordering::SeqCst), 1);
    let order: Vec<String> = fixture
        .fake
        .specs
        .lock()
        .unwrap()
        .iter()
        .map(|spec| {
            spec.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(order, names);
    let mut saved: Vec<_> = rows(&fixture)
        .into_iter()
        .map(|row| (row.created_at, row.source_name.unwrap()))
        .collect();
    saved.sort();
    assert_eq!(
        saved.into_iter().map(|(_, name)| name).collect::<Vec<_>>(),
        names
    );
}

#[test]
fn progress_only_goes_up_and_stays_inside_0_to_1() {
    let mut progress = Progress::default();
    assert!(progress.raise(0.25));
    assert!(!progress.raise(0.251), "the same percent is not news");
    assert!(progress.raise(0.5));
    assert!(!progress.raise(0.4), "a report that goes back is ignored");
    assert_eq!(progress.fraction(), 0.5);
    assert!(progress.raise(7.0));
    assert_eq!(progress.fraction(), 1.0);
    let mut nan = Progress::default();
    assert!(!nan.raise(f32::NAN));
    assert_eq!(nan.percent(), 0);
}

#[gpui_kit::test]
fn the_progress_of_a_job_reaches_its_item(cx: &mut TestAppContext) {
    let fixture = open_with(cx, &["base"], vec![0.25, 0.5, 0.4]);
    say(&fixture, failed(error::ENGINE_CRASHED, "stopped"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    run_one(cx, &fixture);
    // The failure keeps the last percent the job reached, and the length of the first report.
    let item = items(cx, &fixture)[0].clone();
    assert_eq!(item["progress"], 50);
    assert_eq!(item["duration_ms"], LENGTH_MS);
}

#[gpui_kit::test]
fn a_cancelled_job_leaves_no_row_and_the_next_file_runs(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let paths = vec![file(&fixture, "long.wav"), file(&fixture, "short.wav")];
    say(&fixture, JobOutcome::Cancelled);
    say(&fixture, done("short words", "en"));
    add(cx, &fixture, paths);
    let id = items(cx, &fixture)[0]["id"].as_u64().unwrap();
    fixture
        .import
        .update(cx, |import, cx| import.cancel(id, cx))
        .unwrap();
    assert_eq!(
        state_of(cx, &fixture),
        ["cancelling", "queued"],
        "the item waits for the engine to answer"
    );
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["cancelled", "running"]);
    run_one(cx, &fixture);

    assert_eq!(state_of(cx, &fixture), ["cancelled", "done"]);
    let saved = rows(&fixture);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].source_name.as_deref(), Some("short.wav"));
    assert!(fixture.fake.tokens.lock().unwrap()[0].is_cancelled());
}

#[gpui_kit::test]
fn a_result_that_lands_after_a_cancel_is_not_saved(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, done("late words", "en"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    let id = items(cx, &fixture)[0]["id"].as_u64().unwrap();
    fixture
        .import
        .update(cx, |import, cx| import.cancel(id, cx))
        .unwrap();
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["cancelled"]);
    assert!(rows(&fixture).is_empty());
}

#[gpui_kit::test]
fn a_queued_item_can_be_cancelled_and_never_runs(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let paths = vec![
        file(&fixture, "first.wav"),
        file(&fixture, "second.wav"),
        file(&fixture, "third.wav"),
    ];
    say(&fixture, done("one", "en"));
    say(&fixture, done("three", "en"));
    add(cx, &fixture, paths);
    let second = items(cx, &fixture)[1]["id"].as_u64().unwrap();
    fixture
        .import
        .update(cx, |import, cx| import.cancel(second, cx))
        .unwrap();
    assert_eq!(state_of(cx, &fixture), ["running", "cancelled", "queued"]);
    run_one(cx, &fixture);
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["done", "cancelled", "done"]);
    assert_eq!(fixture.fake.specs.lock().unwrap().len(), 2);
}

#[gpui_kit::test]
fn an_unsupported_format_fails_the_item_with_its_code_and_saves_no_row(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(
        &fixture,
        failed(
            error::IMPORT_UNSUPPORTED_CODEC,
            "Opus is not supported yet.",
        ),
    );
    say(&fixture, done("fine", "en"));
    add(
        cx,
        &fixture,
        vec![file(&fixture, "a.opus"), file(&fixture, "b.wav")],
    );
    run_one(cx, &fixture);

    let first = items(cx, &fixture)[0].clone();
    assert_eq!(first["state"], "failed");
    assert_eq!(first["code"], error::IMPORT_UNSUPPORTED_CODEC);
    assert!(
        first["message"].as_str().unwrap().contains("Opus"),
        "{first}"
    );
    assert!(rows(&fixture).is_empty());
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["failed", "done"]);
    assert_eq!(rows(&fixture).len(), 1);
}

#[gpui_kit::test]
fn a_failed_file_says_why_in_words_with_no_error_code(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(
        &fixture,
        failed(
            error::IMPORT_UNSUPPORTED_CODEC,
            "Opus is not supported yet.",
        ),
    );
    say(
        &fixture,
        failed(error::ENGINE_CRASHED, "the engine stopped"),
    );
    add(
        cx,
        &fixture,
        vec![file(&fixture, "a.opus"), file(&fixture, "b.wav")],
    );
    run_one(cx, &fixture);
    run_one(cx, &fixture);

    let lines: Vec<String> = fixture.import.read_with(cx, |import, _| {
        import
            .items()
            .iter()
            .map(|item| panel::detail_of(item, None, 0))
            .collect()
    });
    assert_eq!(lines[0], "Opus is not supported yet.");
    assert_eq!(
        lines[1],
        "The speech engine stopped while reading this file. Try again."
    );
}

#[gpui_kit::test]
fn a_file_that_cannot_be_read_fails_with_its_own_code(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(
        &fixture,
        failed(error::IMPORT_DECODE_FAILED, "the stream has no audio"),
    );
    add(cx, &fixture, vec![file(&fixture, "broken.mp3")]);
    run_one(cx, &fixture);
    let item = items(cx, &fixture)[0].clone();
    assert_eq!(item["code"], error::IMPORT_DECODE_FAILED);
    assert!(
        item["message"]
            .as_str()
            .unwrap()
            .contains("could not be read")
    );
    assert!(rows(&fixture).is_empty());
}

#[gpui_kit::test]
fn an_engine_that_dies_fails_the_item_and_the_next_one_runs(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(
        &fixture,
        failed(error::ENGINE_CRASHED, "the engine stopped"),
    );
    say(&fixture, done("next words", "en"));
    add(
        cx,
        &fixture,
        vec![file(&fixture, "a.wav"), file(&fixture, "b.wav")],
    );
    run_one(cx, &fixture);
    assert_eq!(items(cx, &fixture)[0]["code"], error::ENGINE_CRASHED);
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["failed", "done"]);
}

#[gpui_kit::test]
fn a_device_a_folder_and_a_pipe_are_refused_without_a_job(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let fifo = fixture.folder.path().join("in.wav");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let folder = fixture.folder.path().to_path_buf();
    for refused in [
        PathBuf::from("/dev/zero"),
        fifo,
        folder,
        PathBuf::from("/no/such/file.wav"),
    ] {
        let name = refused.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(add(cx, &fixture, vec![refused]), 0, "{name}");
        let said = notice(cx, &fixture).expect("a notice");
        assert!(said.contains("cannot be opened"), "{said}");
        assert!(said.contains(&name), "{said}");
    }
    assert!(items(cx, &fixture).is_empty());
    assert!(fixture.rig.work.borrow().is_empty());
    assert!(rows(&fixture).is_empty());

    // The queue still works afterwards.
    say(&fixture, done("fine", "en"));
    add(cx, &fixture, vec![file(&fixture, "ok.wav")]);
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["done"]);
}

#[gpui_kit::test]
fn a_good_file_next_to_a_refused_one_is_still_queued(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, done("fine", "en"));
    let queued = add(
        cx,
        &fixture,
        vec![PathBuf::from("/dev/zero"), file(&fixture, "ok.wav")],
    );
    assert_eq!(queued, 1);
    assert!(notice(cx, &fixture).is_some());
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["done"]);
}

#[gpui_kit::test]
fn without_a_speech_model_nothing_is_queued_and_the_notice_says_why(cx: &mut TestAppContext) {
    let fixture = open_with(cx, &[], vec![]);
    assert_eq!(add(cx, &fixture, vec![file(&fixture, "a.wav")]), 0);
    let said = notice(cx, &fixture).unwrap();
    assert!(said.contains("speech model"), "{said}");
    assert!(items(cx, &fixture).is_empty());
}

#[gpui_kit::test]
fn the_files_language_and_the_dictionary_apply_to_an_import(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let settings = &fixture.rig.storage.settings;
    // Dictation keeps its own language.
    settings
        .set("dictation.language", serde_json::json!("fr"))
        .unwrap();
    fixture
        .import
        .update(cx, |import, cx| import.set_language("es", cx))
        .unwrap();
    assert_eq!(
        settings.get(LANGUAGE_SETTING),
        Some(serde_json::json!("es"))
    );
    hushpen_store::dictionary::add(&fixture.rig.storage.database, "Paulina", None).unwrap();
    say(&fixture, done("Hola Paulina.", "es"));
    add(cx, &fixture, vec![file(&fixture, "speech-es.wav")]);
    run_one(cx, &fixture);

    let spec = fixture.fake.specs.lock().unwrap()[0].clone();
    assert_eq!(spec.language.as_deref(), Some("es"));
    assert!(spec.prompt.unwrap().contains("Paulina"));
    let [row] = rows(&fixture).try_into().unwrap();
    assert_eq!(row.language_requested.as_deref(), Some("es"));
    assert_eq!(row.language_detected.as_deref(), Some("es"));
}

#[gpui_kit::test]
fn a_detected_language_is_kept_on_the_row_when_the_language_is_auto(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, done("Hola Paulina.", "es"));
    add(cx, &fixture, vec![file(&fixture, "speech-es.wav")]);
    run_one(cx, &fixture);
    let [row] = rows(&fixture).try_into().unwrap();
    assert_eq!(row.language_detected.as_deref(), Some("es"));
    assert_eq!(row.language_requested, None);
}

#[gpui_kit::test]
fn a_file_with_no_words_fails_with_no_speech_and_saves_no_row(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, done("[BLANK_AUDIO]", "en"));
    add(cx, &fixture, vec![file(&fixture, "silence.wav")]);
    run_one(cx, &fixture);
    let item = items(cx, &fixture)[0].clone();
    assert_eq!(item["state"], "failed");
    assert_eq!(item["code"], error::ENGINE_NO_SPEECH);
    assert!(rows(&fixture).is_empty());
}

#[gpui_kit::test]
fn the_view_is_busy_from_the_add_until_the_last_result_is_saved(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let busy = |cx: &mut TestAppContext| fixture.import.read_with(cx, |import, _| import.busy());
    assert!(!busy(cx));
    say(&fixture, done("words", "en"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    assert!(busy(cx));
    run_one(cx, &fixture);
    assert!(!busy(cx));
}

#[gpui_kit::test]
fn finished_items_can_be_cleared_and_the_running_one_stays(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, done("one", "en"));
    say(&fixture, done("two", "en"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    run_one(cx, &fixture);
    add(cx, &fixture, vec![file(&fixture, "b.wav")]);
    assert_eq!(state_of(cx, &fixture), ["done", "running"]);
    fixture
        .import
        .update(cx, |import, cx| import.clear_finished(cx));
    assert_eq!(state_of(cx, &fixture), ["running"]);
}

#[gpui_kit::test]
fn a_new_row_makes_the_history_reload(cx: &mut TestAppContext) {
    let fixture = open(cx);
    let before = fixture
        .rig
        .controller
        .read_with(cx, |controller, _| controller.history_revision());
    say(&fixture, done("words", "en"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    run_one(cx, &fixture);
    let after = fixture
        .rig
        .controller
        .read_with(cx, |controller, _| controller.history_revision());
    assert!(after > before);
}

#[gpui_kit::test]
fn the_row_of_a_finished_item_is_named_in_the_state(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, done("The quick brown fox.", "en"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    run_one(cx, &fixture);
    let item = items(cx, &fixture)[0].clone();
    let [row] = rows(&fixture).try_into().unwrap();
    assert_eq!(item["row_id"], row.id);
    assert_eq!(item["text"], "The quick brown fox.");
    assert_eq!(item["name"], "a.wav");
    settle(cx, &fixture.rig);
}

fn id_of(cx: &mut TestAppContext, fixture: &Fixture, index: usize) -> u64 {
    items(cx, fixture)[index]["id"].as_u64().unwrap()
}

fn finished(cx: &mut TestAppContext, text: &str) -> (Fixture, u64) {
    let fixture = open(cx);
    say(&fixture, done(text, "en"));
    add(cx, &fixture, vec![file(&fixture, "talk.wav")]);
    run_one(cx, &fixture);
    let id = id_of(cx, &fixture, 0);
    (fixture, id)
}

#[gpui_kit::test]
fn a_row_shows_the_size_at_once_and_the_length_of_the_audio_when_done(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, done("words", "en"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    let queued = items(cx, &fixture)[0].clone();
    assert_eq!(queued["size_bytes"], 5);
    assert_eq!(queued["duration_ms"], Value::Null);
    run_one(cx, &fixture);
    assert_eq!(items(cx, &fixture)[0]["duration_ms"], 4_500);
}

#[gpui_kit::test]
fn copy_puts_the_transcript_on_the_clipboard(cx: &mut TestAppContext) {
    let (fixture, id) = finished(cx, "The quick brown fox.");
    fixture
        .import
        .update(cx, |import, cx| import.copy(id, cx))
        .unwrap();
    assert_eq!(
        crate::controller::testkit::clipboard(cx).as_deref(),
        Some("The quick brown fox.")
    );
    assert!(
        fixture
            .import
            .read_with(cx, |import, _| import.info().is_some())
    );
}

#[gpui_kit::test]
fn export_writes_the_transcript_in_the_chosen_format(cx: &mut TestAppContext) {
    let (fixture, id) = finished(cx, "first piece second piece");
    let out = tempfile::tempdir().unwrap();
    let path = out.path().join("talk.srt");
    fixture
        .import
        .update(cx, |import, cx| import.export(id, Format::Srt, cx))
        .unwrap();
    cx.run_until_parked();
    let chosen = path.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen.clone()));
    cx.run_until_parked();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("00:00:02,000 --> 00:00:04,500"),
        "{written}"
    );
    let state = cx.update(|cx| fixture.import.read(cx).state_json(cx));
    assert_eq!(state["exporting"], false);
}

#[gpui_kit::test]
fn show_in_history_asks_for_the_row_of_the_item(cx: &mut TestAppContext) {
    let (fixture, id) = finished(cx, "words");
    let asked = Rc::new(std::cell::RefCell::new(Vec::new()));
    cx.update(|cx| {
        let asked = Rc::clone(&asked);
        cx.subscribe(&fixture.import, move |_, event: &ImportRequest, _| {
            let ImportRequest::ShowInHistory(row) = event;
            asked.borrow_mut().push(row.clone());
        })
        .detach();
    });
    fixture
        .import
        .update(cx, |import, cx| import.show_in_history(id, cx))
        .unwrap();
    let [row] = rows(&fixture).try_into().unwrap();
    assert_eq!(*asked.borrow(), [row.id]);
}

#[gpui_kit::test]
fn a_failed_item_can_be_retried_and_then_saved(cx: &mut TestAppContext) {
    let fixture = open(cx);
    say(&fixture, failed(error::ENGINE_CRASHED, "stopped"));
    say(&fixture, done("words", "en"));
    add(cx, &fixture, vec![file(&fixture, "a.wav")]);
    run_one(cx, &fixture);
    let id = id_of(cx, &fixture, 0);
    assert!(
        fixture
            .import
            .update(cx, |import, cx| import.copy(id, cx))
            .is_err(),
        "a failed item has no transcript"
    );
    fixture
        .import
        .update(cx, |import, cx| import.retry(id, cx))
        .unwrap();
    run_one(cx, &fixture);
    assert_eq!(state_of(cx, &fixture), ["done"]);
    assert_eq!(rows(&fixture).len(), 1);
}

#[gpui_kit::test]
fn translate_reaches_the_job_and_is_kept(cx: &mut TestAppContext) {
    let fixture = open(cx);
    fixture
        .import
        .update(cx, |import, cx| import.set_translate(true, cx))
        .unwrap();
    say(&fixture, done("Hello.", "en"));
    add(cx, &fixture, vec![file(&fixture, "speech-es.wav")]);
    run_one(cx, &fixture);
    assert!(fixture.fake.specs.lock().unwrap()[0].translate);
    assert_eq!(
        fixture.rig.storage.settings.get(TRANSLATE_SETTING),
        Some(serde_json::json!(true))
    );
}

#[gpui_kit::test]
fn an_english_only_model_turns_translate_and_the_language_off(cx: &mut TestAppContext) {
    let fixture = open_with(cx, &["tiny.en"], vec![]);
    let refused = fixture
        .import
        .update(cx, |import, cx| import.set_translate(true, cx));
    assert!(refused.unwrap_err().contains("English-only"));
    let state = cx.update(|cx| fixture.import.read(cx).state_json(cx));
    assert!(state["translate_off"].is_string());
    assert!(state["language_off"].is_string());
    assert_eq!(state["translate"], false);
}

fn choose_model(cx: &mut TestAppContext, fixture: &Fixture, id: &str) {
    fixture
        .rig
        .models
        .update(cx, |models, cx| models.select(id, cx))
        .unwrap();
    cx.run_until_parked();
}

fn state_json(cx: &mut TestAppContext, fixture: &Fixture) -> Value {
    cx.update(|cx| fixture.import.read(cx).state_json(cx))
}

fn language_notice(cx: &mut TestAppContext, fixture: &Fixture) -> Option<&'static str> {
    cx.update(|cx| fixture.import.read(cx).language_notice(cx))
}

#[gpui_kit::test]
fn translate_is_off_for_parakeet_and_turbo_and_comes_back_with_small(cx: &mut TestAppContext) {
    let fixture = open_with(cx, &[SMALL_ID, PARAKEET_ID, TURBO_ID], vec![]);
    fixture
        .import
        .update(cx, |import, cx| import.set_translate(true, cx))
        .unwrap();
    assert_eq!(state_json(cx, &fixture)["translate"], true);

    for (index, id) in [PARAKEET_ID, TURBO_ID].into_iter().enumerate() {
        choose_model(cx, &fixture, id);
        let state = state_json(cx, &fixture);
        assert_eq!(state["translate"], false, "{id}");
        assert!(state["translate_off"].is_string(), "{id}");
        assert!(
            !cx.update(|cx| fixture.import.read(cx).translate(cx)),
            "{id}"
        );
        assert_eq!(
            fixture.rig.storage.settings.get(TRANSLATE_SETTING),
            Some(serde_json::json!(true)),
            "{id} keeps the choice"
        );

        say(&fixture, done("Hello.", "en"));
        add(
            cx,
            &fixture,
            vec![file(&fixture, &format!("speech-{index}.wav"))],
        );
        run_one(cx, &fixture);
        assert!(!fixture.fake.specs.lock().unwrap()[index].translate, "{id}");
    }

    choose_model(cx, &fixture, SMALL_ID);
    let state = state_json(cx, &fixture);
    assert_eq!(state["translate"], true);
    assert!(state["translate_off"].is_null());
    say(&fixture, done("Hello.", "en"));
    add(cx, &fixture, vec![file(&fixture, "speech-small.wav")]);
    run_one(cx, &fixture);
    assert!(fixture.fake.specs.lock().unwrap()[2].translate);
}

#[gpui_kit::test]
fn the_dictionary_prompt_goes_to_whisper_files_and_not_to_parakeet(cx: &mut TestAppContext) {
    let fixture = open_with(cx, &[PARAKEET_ID, SMALL_ID], vec![]);
    hushpen_store::dictionary::add(&fixture.rig.storage.database, "Hushpen", None).unwrap();
    for (name, id) in [("a.wav", PARAKEET_ID), ("b.wav", SMALL_ID)] {
        choose_model(cx, &fixture, id);
        say(&fixture, done("Hello.", "en"));
        add(cx, &fixture, vec![file(&fixture, name)]);
        run_one(cx, &fixture);
    }
    let specs = fixture.fake.specs.lock().unwrap();
    assert_eq!(specs[0].prompt, None);
    assert_eq!(specs[1].prompt.as_deref(), Some("Hushpen"));
}

#[gpui_kit::test]
fn parakeet_has_no_language_notice_for_auto_or_spanish(cx: &mut TestAppContext) {
    let fixture = open_with(cx, &[PARAKEET_ID], vec![]);
    assert_eq!(language_notice(cx, &fixture), None);
    fixture
        .import
        .update(cx, |import, cx| import.set_language("es", cx))
        .unwrap();
    assert_eq!(language_notice(cx, &fixture), None);
}

#[gpui_kit::test]
fn parakeet_names_whisper_turbo_for_hindi_and_japanese_and_whisper_clears_it(
    cx: &mut TestAppContext,
) {
    let fixture = open_with(cx, &[PARAKEET_ID, SMALL_ID], vec![]);
    for code in ["hi", "ja"] {
        fixture
            .import
            .update(cx, |import, cx| import.set_language(code, cx))
            .unwrap();
        let notice = language_notice(cx, &fixture);
        assert!(
            notice.unwrap().contains("Whisper Large v3 Turbo Q5"),
            "{code}"
        );
        assert!(state_json(cx, &fixture)["language_off"].is_null(), "{code}");
    }
    choose_model(cx, &fixture, SMALL_ID);
    assert_eq!(language_notice(cx, &fixture), None);
}

mod view {
    use super::*;
    use crate::shell::Shell;
    use crate::theme::space;
    use crate::views::View;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AnyWindowHandle, Bounds, Point, WindowBounds, WindowOptions, px, size};

    fn shown(cx: &mut TestAppContext, fixture: &Fixture) -> AnyWindowHandle {
        cx.update(gpui_kit::init);
        let (handle, shell) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Shell::new(window, cx)),
            )
            .expect("open test window")
        });
        let files = cx.new(|cx| {
            crate::files::Files::new(
                Rc::clone(&fixture.rig.storage),
                fixture.import.clone(),
                None,
                cx,
            )
        });
        shell.update(cx, |shell, cx| {
            shell.attach_files(files, cx);
            shell.select(View::Manual, cx);
        });
        frame(cx, handle);
        handle
    }

    fn frame(cx: &mut TestAppContext, handle: AnyWindowHandle) {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }

    fn present(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> bool {
        cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
            .unwrap()
    }

    fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
        cx.update_window(handle, |_, window, cx| window.click(id, cx))
            .unwrap();
        frame(cx, handle);
    }

    #[gpui_kit::test]
    fn an_empty_queue_shows_the_drop_zone_and_the_options(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        for id in [
            "files.mode",
            "import.drop",
            "import.choose",
            "import.model",
            "import.language",
            "import.translate",
        ] {
            assert!(present(cx, window, id), "{id}");
        }
        assert!(!present(cx, window, "import.queue"));
        assert!(!present(cx, window, "import.blocker"));
    }

    #[gpui_kit::test]
    fn the_language_notice_shows_for_parakeet_with_hindi_and_the_picker_stays_on(
        cx: &mut TestAppContext,
    ) {
        let fixture = open_with(cx, &[PARAKEET_ID, SMALL_ID], vec![]);
        let window = shown(cx, &fixture);
        assert!(!present(cx, window, "import.language-notice"), "auto");
        fixture
            .import
            .update(cx, |import, cx| import.set_language("es", cx))
            .unwrap();
        frame(cx, window);
        assert!(!present(cx, window, "import.language-notice"), "es");

        fixture
            .import
            .update(cx, |import, cx| import.set_language("hi", cx))
            .unwrap();
        frame(cx, window);
        assert!(present(cx, window, "import.language-notice"));
        click(cx, window, "import.language");
        assert!(present(cx, window, "import.language.list"));

        choose_model(cx, &fixture, SMALL_ID);
        frame(cx, window);
        assert!(!present(cx, window, "import.language-notice"));
    }

    #[gpui_kit::test]
    fn files_dropped_on_the_window_open_the_files_view_and_are_queued(cx: &mut TestAppContext) {
        use gpui_kit::{ExternalPaths, FileDropEvent, InputEvent as _};
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        let path = file(&fixture, "talk.mp3");
        let position = Point::new(px(400.), px(300.));
        for event in [
            FileDropEvent::Entered {
                position,
                paths: ExternalPaths(vec![path].into()),
            },
            FileDropEvent::Submit { position },
        ] {
            cx.update_window(window, |_, window, cx| {
                window.dispatch_event(event.to_platform_input(), cx);
            })
            .unwrap();
            frame(cx, window);
        }
        assert_eq!(state_of(cx, &fixture), ["running"]);
        assert!(present(cx, window, "import.row.0"));
    }

    #[gpui_kit::test]
    fn a_done_line_offers_copy_export_show_and_remove(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        say(&fixture, done("words", "en"));
        add(cx, &fixture, vec![file(&fixture, "talk.mp3")]);
        run_one(cx, &fixture);
        frame(cx, window);
        for id in [
            "import.copy.0",
            "import.export.0",
            "import.show.0",
            "import.remove.0",
        ] {
            assert!(present(cx, window, id), "{id}");
        }
        click(cx, window, "import.export.0");
        assert!(present(cx, window, "import.export.option.srt"));
    }

    #[gpui_kit::test]
    fn esc_closes_the_export_menu_and_its_button_keeps_the_focus(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        say(&fixture, done("words", "en"));
        add(cx, &fixture, vec![file(&fixture, "talk.mp3")]);
        run_one(cx, &fixture);
        click(cx, window, "import.export.0");
        assert!(present(cx, window, "import.export.list"));
        cx.update_window(window, |_, window, cx| window.press("escape", cx))
            .unwrap();
        frame(cx, window);
        assert!(!present(cx, window, "import.export.list"));
        let focused = cx
            .update_window(window, |_, window, _| {
                window.find("import.export.0").focused()
            })
            .unwrap();
        assert_eq!(focused, Some(true));
    }

    #[gpui_kit::test]
    fn a_failed_line_offers_retry(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        say(&fixture, failed(error::ENGINE_CRASHED, "stopped"));
        say(&fixture, done("words", "en"));
        add(cx, &fixture, vec![file(&fixture, "talk.mp3")]);
        run_one(cx, &fixture);
        frame(cx, window);
        click(cx, window, "import.retry.0");
        run_one(cx, &fixture);
        assert_eq!(state_of(cx, &fixture), ["done"]);
    }

    #[gpui_kit::test]
    fn a_queued_file_shows_a_line_with_its_status_and_cancel(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        add(cx, &fixture, vec![file(&fixture, "talk.mp3")]);
        frame(cx, window);
        assert!(present(cx, window, "import.row.0"));
        assert!(present(cx, window, "import.cancel.0"));
        assert!(present(cx, window, "import.queue"));

        click(cx, window, "import.cancel.0");
        assert_eq!(state_of(cx, &fixture), ["cancelling"]);
    }

    #[gpui_kit::test]
    fn a_failed_line_offers_remove_and_clear_finished_empties_the_list(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        say(&fixture, failed(error::IMPORT_DECODE_FAILED, "bad"));
        add(cx, &fixture, vec![file(&fixture, "broken.mp3")]);
        run_one(cx, &fixture);
        frame(cx, window);
        assert!(present(cx, window, "import.remove.0"));
        assert!(!present(cx, window, "import.cancel.0"));

        click(cx, window, "import.clear");
        assert!(!present(cx, window, "import.row.0"));
        assert!(!present(cx, window, "import.queue"));
    }

    #[gpui_kit::test]
    fn the_notice_of_a_refused_file_is_a_line_of_the_view(cx: &mut TestAppContext) {
        let fixture = open(cx);
        let window = shown(cx, &fixture);
        add(cx, &fixture, vec![PathBuf::from("/dev/zero")]);
        frame(cx, window);
        assert!(present(cx, window, "import.notice"));
        assert!(!present(cx, window, "import.row.0"));
    }

    #[gpui_kit::test]
    fn with_no_model_the_view_says_so_and_the_picker_is_off(cx: &mut TestAppContext) {
        let fixture = open_with(cx, &[], vec![]);
        let window = shown(cx, &fixture);
        assert!(present(cx, window, "import.blocker"));
        click(cx, window, "import.choose");
        assert!(!fixture.import.read_with(cx, |import, _| import.choosing()));
    }
}
