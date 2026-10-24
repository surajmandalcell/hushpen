//! The History view: a paged, searchable list of every dictation outcome, and the detail view
//! of one row with Copy, Re-paste, Reprocess, and Delete.
//!
//! The rows live in the history database. The controller writes them and redoes the engine and
//! paste work; this view reads, searches, and deletes. A delete takes the row out of the
//! database at once and keeps it, with its audio, for the undo window. When the window ends,
//! or the app quits, or the app starts again, the audio file goes too.

mod delete;
mod detail;
pub mod export;
mod list;
pub mod panel;
pub mod playback;
mod save_dialog;
mod sweep;

use crate::controller::{Controller, ReprocessState, failure_for};
use crate::hook;
use crate::storage::Storage;
use delete::{Undo, sweep_trash};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext as _, Bounds, ClipboardItem, Context, Entity, FocusHandle, Pixels,
    ScrollHandle, Window,
};
use hushpen_store::history::{self, Cursor, Row};
use serde_json::{Value, json};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// Rows in one page.
pub const PAGE: usize = 50;
/// The height of one list row. Every row has this height, so the list can leave out the rows
/// that are far from the screen and stand in for them with space.
pub const ROW_HEIGHT: f32 = 48.0;
/// How long a deleted transcript can come back.
pub const UNDO_WINDOW: Duration = Duration::from_secs(8);

pub struct Focus {
    pub clear: FocusHandle,
    pub clear_confirm: FocusHandle,
    pub clear_cancel: FocusHandle,
    pub undo: FocusHandle,
    pub more: FocusHandle,
    pub back: FocusHandle,
    pub play: FocusHandle,
    pub seek: FocusHandle,
    pub copy: FocusHandle,
    pub repaste: FocusHandle,
    pub undo_cleanup: FocusHandle,
    pub copy_original: FocusHandle,
    pub reprocess: FocusHandle,
    pub delete: FocusHandle,
    pub select: FocusHandle,
    pub select_all: FocusHandle,
    pub export: FocusHandle,
    /// One for each of `Format::ALL`.
    pub export_format: [FocusHandle; 4],
}

pub struct History {
    storage: Rc<Storage>,
    controller: Entity<Controller>,
    search: Entity<InputState>,
    query: String,
    rows: Vec<Row>,
    next: Option<Cursor>,
    total: i64,
    search_ms: f64,
    detail: Option<Row>,
    /// The timed pieces of the open row, for the detail view of an import.
    detail_segments: Vec<history::Segment>,
    undo: Option<Undo>,
    undo_generation: u64,
    undo_window: Duration,
    confirm_clear: bool,
    message: Option<String>,
    /// What the last action did, in a neutral line. A problem goes in `message`.
    notice: Option<String>,
    /// Rows are chosen with a click instead of opened.
    selecting: bool,
    selection: Vec<String>,
    /// The format row of the detail view.
    export_menu: bool,
    export_busy: bool,
    export_dir: Option<PathBuf>,
    last_export: Option<export::Exported>,
    seen_revision: u64,
    /// The scroll position of the pane that holds the list.
    scroll: ScrollHandle,
    /// The scroll range at the time the next page was last asked for.
    asked_at: Cell<f32>,
    playback: Option<playback::Playback>,
    tick_generation: u64,
    /// Where the seek bar was drawn, so a click can tell how far along it landed.
    pub(crate) seek_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Tests play without a sound device.
    silent_playback: bool,
    pub(crate) focus: Focus,
    pub(crate) row_focus: Vec<FocusHandle>,
}

impl History {
    pub fn new(
        storage: Rc<Storage>,
        controller: Entity<Controller>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search transcripts"));
        cx.subscribe_in(&search, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().to_string();
                this.set_query(value, cx);
            }
        })
        .detach();
        cx.observe(&controller, |this, _, cx| this.follow_controller(cx))
            .detach();
        let handle = |cx: &mut Context<Self>| cx.focus_handle().tab_stop(true);
        let focus = Focus {
            clear: handle(cx),
            clear_confirm: handle(cx),
            clear_cancel: handle(cx),
            undo: handle(cx),
            more: handle(cx),
            back: handle(cx),
            play: handle(cx),
            seek: handle(cx),
            copy: handle(cx),
            repaste: handle(cx),
            undo_cleanup: handle(cx),
            copy_original: handle(cx),
            reprocess: handle(cx),
            delete: handle(cx),
            select: handle(cx),
            select_all: handle(cx),
            export: handle(cx),
            export_format: std::array::from_fn(|_| handle(cx)),
        };
        sweep_trash(&storage);
        let seen_revision = controller.read(cx).history_revision();
        let mut view = Self {
            storage,
            controller,
            search,
            query: String::new(),
            rows: Vec::new(),
            next: None,
            total: 0,
            search_ms: 0.0,
            detail: None,
            detail_segments: Vec::new(),
            undo: None,
            undo_generation: 0,
            undo_window: UNDO_WINDOW,
            confirm_clear: false,
            message: None,
            notice: None,
            selecting: false,
            selection: Vec::new(),
            export_menu: false,
            export_busy: false,
            export_dir: None,
            last_export: None,
            seen_revision,
            scroll: ScrollHandle::new(),
            asked_at: Cell::new(0.0),
            playback: None,
            tick_generation: 0,
            seek_bounds: Rc::new(Cell::new(Bounds::default())),
            silent_playback: cfg!(test),
            focus,
            row_focus: Vec::new(),
        };
        view.reload(cx);
        view.start_sweeps(cx);
        view
    }

    /// Takes the scroll position of the pane that shows the list.
    pub fn use_scroll(&mut self, scroll: ScrollHandle) {
        self.scroll = scroll;
    }

    #[cfg(test)]
    fn set_undo_window(&mut self, window: Duration) {
        self.undo_window = window;
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn has_more(&self) -> bool {
        self.next.is_some()
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn total(&self) -> i64 {
        self.total
    }

    pub fn detail(&self) -> Option<&Row> {
        self.detail.as_ref()
    }

    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    pub fn confirming_clear(&self) -> bool {
        self.confirm_clear
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn search_input(&self) -> &Entity<InputState> {
        &self.search
    }

    pub fn controller(&self) -> &Entity<Controller> {
        &self.controller
    }

    pub fn audio_available(&self, row: &Row, cx: &App) -> bool {
        self.controller.read(cx).audio_file(row).is_some()
    }

    /// What a reprocess of this row is doing now, for the detail view.
    pub fn reprocess_state(&self, id: &str, cx: &App) -> Option<ReprocessState> {
        self.controller
            .read(cx)
            .reprocess_state()
            .filter(|(owner, _)| owner == id)
            .map(|(_, state)| state.clone())
    }

    fn follow_controller(&mut self, cx: &mut Context<Self>) {
        let revision = self.controller.read(cx).history_revision();
        if revision != self.seen_revision {
            self.seen_revision = revision;
            self.reload(cx);
        } else {
            cx.notify();
        }
    }

    pub fn open(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let row = history::get(&self.storage.database, id)
            .map_err(|error| error.to_string())?
            .ok_or("That transcript is no longer in the history.")?;
        if self.detail.as_ref().is_none_or(|open| open.id != row.id) {
            self.playback = None;
        }
        self.detail_segments = self.segments_of(Some(&row));
        self.detail = Some(row);
        self.message = None;
        self.notice = None;
        self.export_menu = false;
        self.confirm_clear = false;
        cx.notify();
        Ok(())
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.playback = None;
        self.export_menu = false;
        self.detail_segments.clear();
        if self.detail.take().is_some() {
            cx.notify();
        }
    }

    fn row(&self, id: Option<&str>) -> Result<Row, String> {
        let id = id
            .or(self.detail.as_ref().map(|row| row.id.as_str()))
            .ok_or("Open a transcript first.")?;
        history::get(&self.storage.database, id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "That transcript is no longer in the history.".to_owned())
    }

    fn text_of(row: &Row) -> Result<String, String> {
        [&row.final_text, &row.rule_text, &row.raw_text]
            .into_iter()
            .flatten()
            .find(|text| !text.is_empty())
            .cloned()
            .ok_or_else(|| "This transcript has no text.".to_owned())
    }

    fn refuse(&mut self, message: String, cx: &mut Context<Self>) -> Result<(), String> {
        self.message = Some(message.clone());
        hook::record_event("history", "refused");
        cx.notify();
        Err(message)
    }

    pub fn copy(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        let text = self.row(id).and_then(|row| Self::text_of(&row));
        match text {
            Ok(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.message = None;
                hook::record_event("history", "copied");
                cx.notify();
                Ok(())
            }
            Err(message) => self.refuse(message, cx),
        }
    }

    /// Copies the text from before the AI cleanup: the rule text, or the raw text when the
    /// rules changed nothing. This is the way to the original words for a row that went into
    /// a terminal, where the bar offers no undo.
    pub fn copy_original(
        &mut self,
        id: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let text = self.row(id).and_then(|row| Self::original_of(&row));
        match text {
            Ok(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.message = None;
                hook::record_event("history", "copied-original");
                cx.notify();
                Ok(())
            }
            Err(message) => self.refuse(message, cx),
        }
    }

    pub(crate) fn original_of(row: &Row) -> Result<String, String> {
        if row.llm_text.is_none() {
            return Err("This transcript has no AI cleanup to look past.".to_owned());
        }
        [&row.rule_text, &row.raw_text]
            .into_iter()
            .flatten()
            .find(|text| !text.is_empty())
            .cloned()
            .ok_or_else(|| "This transcript has no original text.".to_owned())
    }

    /// Makes the rule text the final text of a row whose AI text was used. No key and no
    /// paste goes to another app: this only changes what History and Paste last hold.
    pub fn undo_cleanup(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        let row = match self.row(id) {
            Ok(row) => row,
            Err(message) => return self.refuse(message, cx),
        };
        match history::undo_cleanup(&self.storage.database, &row.id) {
            Ok(Some(text)) => {
                self.message = None;
                self.controller.update(cx, |controller, cx| {
                    controller.cleanup_undone(&row.id, text, cx)
                });
                self.reload(cx);
                Ok(())
            }
            Ok(None) => self.refuse(
                "The AI cleanup of this transcript cannot be undone.".into(),
                cx,
            ),
            Err(error) => self.refuse(format!("The cleanup could not be undone: {error}"), cx),
        }
    }

    /// Puts the text of the row into the app that has the focus, as soon as Hushpen no longer
    /// has it.
    pub fn repaste(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        let text = self.row(id).and_then(|row| Self::text_of(&row));
        match text {
            Ok(text) => {
                self.message = None;
                // The click that got here runs inside the window's own update, so the window
                // steps aside once that ends. The controller then waits for the focus to leave.
                cx.defer(crate::main_window::step_aside);
                self.controller
                    .update(cx, |controller, cx| controller.repaste(text, cx));
                cx.notify();
                Ok(())
            }
            Err(message) => self.refuse(message, cx),
        }
    }

    pub fn reprocess(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        let started = self.row(id).and_then(|row| {
            self.controller
                .update(cx, |controller, cx| controller.reprocess(&row.id, cx))
        });
        match started {
            Ok(()) => {
                self.message = None;
                cx.notify();
                Ok(())
            }
            Err(message) => self.refuse(message, cx),
        }
    }

    pub fn state_json(&self, cx: &App) -> Value {
        let reprocess = self
            .controller
            .read(cx)
            .reprocess_state()
            .map(|(id, state)| {
                let (name, message) = match state {
                    ReprocessState::Running => ("running", None),
                    ReprocessState::Failed(message) => ("failed", Some(message.as_str())),
                };
                json!({"id": id, "state": name, "message": message})
            });
        json!({
            "count": self.rows.len(),
            "total": self.total,
            "query": self.query,
            "search_ms": (self.search_ms * 100.0).round() / 100.0,
            "has_more": self.next.is_some(),
            "drawn": [self.window_rows().start, self.window_rows().end],
            "rows": self.rows.iter().map(|row| json!({
                "id": row.id,
                "created_at": row.created_at,
                "status": row.status,
                "title": panel::title(row),
                "kind": row.kind,
                "source_name": row.source_name,
                "meta": panel::meta(row),
                "insert_outcome": row.insert_outcome,
                "audio_removed": row.audio_removed_at.is_some(),
                "selected": self.is_selected(&row.id),
            })).collect::<Vec<_>>(),
            "detail": self.detail.as_ref().map(|row| json!({
                "id": row.id,
                "kind": row.kind,
                "source_name": row.source_name,
                "segments": self.detail_segments.iter().map(|segment| json!({
                    "start_ms": segment.start_ms,
                    "end_ms": segment.end_ms,
                    "text": segment.text,
                })).collect::<Vec<_>>(),
                "status": row.status,
                "error_code": row.error_code,
                "raw_text": row.raw_text,
                "rule_text": row.rule_text,
                "llm_text": row.llm_text,
                "llm_outcome": row.llm_outcome,
                "final_text": row.final_text,
                "target_app": row.target_app,
                "duration_ms": row.duration_ms,
                "model_id": row.model_id,
                "language": row.language_detected.as_ref().or(row.language_requested.as_ref()),
                "insert_outcome": row.insert_outcome,
                "audio": self.audio_available(row, cx),
                "audio_removed_at": row.audio_removed_at,
                "audio_note": panel::audio_note(row, self.audio_available(row, cx)),
            })),
            "playback": self.playback_json(),
            "last": history::newest(&self.storage.database).ok().flatten().map(|row| json!({
                "id": row.id,
                "status": row.status,
                "error_code": row.error_code,
                "insert_outcome": row.insert_outcome,
                "prompt": row.prompt,
                "raw_text": row.raw_text,
                "rule_text": row.rule_text,
                "llm_text": row.llm_text,
                "llm_outcome": row.llm_outcome,
                "final_text": row.final_text,
            })),
            "undo": self.undo.as_ref().map(|undo| json!({"id": undo.stored.row.id})),
            "confirm_clear": self.confirm_clear,
            "message": self.message,
            "notice": self.notice,
            "selecting": self.selecting,
            "selection": self.selection,
            "export_menu": self.export_menu,
            "export_busy": self.export_busy,
            "export": self.last_export.as_ref().map(export::Exported::json),
            "reprocess": reprocess,
            "read_only": self.storage.database.read_only(),
        })
    }

    /// The message that explains a failed run, for rows that show it.
    pub fn failure_message(code: &str) -> &'static str {
        failure_for(code).1
    }
}

#[cfg(test)]
mod tests;
