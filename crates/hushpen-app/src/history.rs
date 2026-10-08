//! The History view: a paged, searchable list of every dictation outcome, and the detail view
//! of one row with Copy, Re-paste, Reprocess, and Delete.
//!
//! The rows live in the history database. The controller writes them and redoes the engine and
//! paste work; this view reads, searches, and deletes. A delete takes the row out of the
//! database at once and keeps it, with its audio, for the undo window. When the window ends,
//! or the app quits, or the app starts again, the audio file goes too.

pub mod export;
pub mod panel;
pub mod playback;
mod save_dialog;
mod sweep;

use crate::controller::{Controller, ReprocessState, failure_for};
use crate::hook;
use crate::storage::Storage;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext as _, Bounds, ClipboardItem, Context, Entity, FocusHandle, Pixels, Point,
    ScrollHandle, Window, px,
};
use hushpen_store::history::{self, Cursor, Row, Stored};
use serde_json::{Value, json};
use std::cell::Cell;
use std::fs;
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Rows in one page.
pub const PAGE: usize = 50;
/// The height of one list row. Every row has this height, so the list can leave out the rows
/// that are far from the screen and stand in for them with space.
pub const ROW_HEIGHT: f32 = 64.0;
/// Where the first row starts below the top of the scrolled content, and the most that banners
/// can add to that.
const LIST_TOP: f32 = 72.0;
const LIST_TOP_MAX: f32 = 220.0;
/// Rows kept ready above and below the screen.
const BUFFER: usize = 3;
/// How close to the end of the list, in pixels, the next page is asked for.
const NEXT_PAGE_REACH: f32 = 160.0;
const FALLBACK_VIEWPORT: f32 = 486.0;
/// How long a deleted transcript can come back.
pub const UNDO_WINDOW: Duration = Duration::from_secs(8);

/// A deleted row that can still come back.
struct Undo {
    stored: Stored,
    /// Where the audio file was, and where it waits while the undo window is open.
    audio: Option<(PathBuf, PathBuf)>,
}

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

    fn scrolled(&self) -> f32 {
        f32::from(-self.scroll.offset().y).max(0.0)
    }

    fn viewport(&self) -> f32 {
        match f32::from(self.scroll.bounds().size.height) {
            height if height > 0.0 => height,
            _ => FALLBACK_VIEWPORT,
        }
    }

    /// The rows that are drawn: the ones on the screen and a few around them. A list of
    /// hundreds of rows would take a whole frame to lay out.
    pub fn window_rows(&self) -> Range<usize> {
        let scrolled = self.scrolled();
        let first = ((scrolled - LIST_TOP_MAX) / ROW_HEIGHT).floor().max(0.0) as usize;
        let last = ((scrolled + self.viewport()) / ROW_HEIGHT).ceil().max(0.0) as usize;
        let end = (last + BUFFER).min(self.rows.len());
        first.saturating_sub(BUFFER).min(end)..end
    }

    /// True when the end of the loaded rows is on the screen and there are more to load. The
    /// scroll range is the one of the last painted frame, so a page that was just asked for
    /// is not asked for again until the range has grown.
    pub fn page_due(&self) -> bool {
        let max = f32::from(self.scroll.max_offset().y);
        let due = self.next.is_some()
            && self.detail.is_none()
            && max > 0.0
            && self.scrolled() > 0.0
            && self.scrolled() >= max - NEXT_PAGE_REACH
            && (max - self.asked_at.get()).abs() > 1.0;
        if due {
            self.asked_at.set(max);
        }
        due
    }

    /// Scrolls so that the row shows, and gives its focus handle.
    pub fn reveal_row(&self, index: usize) -> Option<FocusHandle> {
        let handle = self.row_focus.get(index)?.clone();
        let top = LIST_TOP + index as f32 * ROW_HEIGHT;
        let bottom = top + ROW_HEIGHT;
        let (scrolled, viewport) = (self.scrolled(), self.viewport());
        let target = if top < scrolled {
            Some(top - ROW_HEIGHT / 2.0)
        } else if bottom > scrolled + viewport {
            Some(bottom - viewport + ROW_HEIGHT / 2.0)
        } else {
            None
        };
        if let Some(target) = target {
            let offset = self.scroll.offset();
            self.scroll
                .set_offset(Point::new(offset.x, px(-target.max(0.0))));
        }
        Some(handle)
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

    /// Reads the first page again, with the search that is typed.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let started = Instant::now();
        match history::page(&self.storage.database, &self.query, None, PAGE) {
            Ok(page) => {
                self.rows = page.rows;
                self.next = page.next;
            }
            Err(error) => {
                self.rows.clear();
                self.next = None;
                self.message = Some(format!("The history could not be read: {error}"));
            }
        }
        self.search_ms = started.elapsed().as_secs_f64() * 1000.0;
        self.asked_at.set(0.0);
        self.total = history::count(&self.storage.database).unwrap_or(0);
        while self.row_focus.len() < self.rows.len() {
            self.row_focus.push(cx.focus_handle().tab_stop(true));
        }
        self.row_focus.truncate(self.rows.len());
        if let Some(open) = &self.detail {
            self.detail = history::get(&self.storage.database, &open.id)
                .ok()
                .flatten();
        }
        cx.notify();
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        if query == self.query {
            return;
        }
        self.query = query;
        self.message = None;
        self.scroll.set_offset(Point::default());
        self.reload(cx);
        hook::record_event("history", "search");
    }

    /// Types `query` into the search box like a user does.
    pub fn search_for(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| {
            input.set_value(query.to_owned(), window, cx)
        });
        self.set_query(query.to_owned(), cx);
    }

    /// Adds the next page below the rows that show.
    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        let Some(cursor) = self.next.clone() else {
            return;
        };
        match history::page(&self.storage.database, &self.query, Some(&cursor), PAGE) {
            Ok(page) => {
                self.rows.extend(page.rows);
                self.next = page.next;
            }
            Err(error) => {
                self.next = None;
                self.message = Some(format!("The history could not be read: {error}"));
            }
        }
        while self.row_focus.len() < self.rows.len() {
            self.row_focus.push(cx.focus_handle().tab_stop(true));
        }
        hook::record_event("history", "more");
        cx.notify();
    }

    pub fn open(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let row = history::get(&self.storage.database, id)
            .map_err(|error| error.to_string())?
            .ok_or("That transcript is no longer in the history.")?;
        if self.detail.as_ref().is_none_or(|open| open.id != row.id) {
            self.playback = None;
        }
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

    /// Puts the text of the row into the app that has the focus, as soon as Hushpen no longer
    /// has it.
    pub fn repaste(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        let text = self.row(id).and_then(|row| Self::text_of(&row));
        match text {
            Ok(text) => {
                self.message = None;
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

    /// Takes the row out of the list and the database and starts the undo window.
    pub fn delete(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        let row = match self.row(id) {
            Ok(row) => row,
            Err(message) => return self.refuse(message, cx),
        };
        self.finish_undo();
        if self.playback.as_ref().is_some_and(|open| open.id == row.id) {
            self.playback = None;
        }
        let stored = match history::delete(&self.storage.database, &row.id) {
            Ok(Some(stored)) => stored,
            Ok(None) => return self.refuse("That transcript is already gone.".into(), cx),
            Err(error) => {
                return self.refuse(format!("The transcript could not be deleted: {error}"), cx);
            }
        };
        let audio = self.park_audio(&stored.row);
        self.forget_selected(&stored.row.id);
        self.undo = Some(Undo { stored, audio });
        self.undo_generation += 1;
        let generation = self.undo_generation;
        let window = self.undo_window;
        self.detail = None;
        self.message = None;
        self.reload(cx);
        hook::record_event("history", "deleted");
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(window).await;
            let _ = this.update(cx, |me, cx| {
                if me.undo_generation == generation && me.undo.is_some() {
                    me.finish_undo();
                    hook::record_event("history", "undo-closed");
                    cx.notify();
                }
            });
        })
        .detach();
        Ok(())
    }

    /// Brings the last deleted row back with its segments and its audio.
    pub fn undo(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(undo) = self.undo.take() else {
            return self.refuse("There is nothing to undo.".into(), cx);
        };
        if let Err(error) = history::restore(&self.storage.database, &undo.stored) {
            let message = format!("The transcript could not be restored: {error}");
            self.undo = Some(undo);
            return self.refuse(message, cx);
        }
        if let Some((original, parked)) = &undo.audio
            && let Err(error) = fs::rename(parked, original)
        {
            log::warn!("the audio of a restored transcript could not be moved back: {error}");
        }
        self.undo_generation += 1;
        self.message = None;
        self.reload(cx);
        hook::record_event("history", "restored");
        Ok(())
    }

    /// Ends the undo window now: the audio of the deleted row goes. Called when the window
    /// closes, and at quit.
    pub fn finish_undo(&mut self) {
        if let Some(undo) = self.undo.take()
            && let Some((_, parked)) = undo.audio
            && let Err(error) = fs::remove_file(&parked)
        {
            log::warn!("the audio of a deleted transcript was not removed: {error}");
        }
    }

    fn park_audio(&self, row: &Row) -> Option<(PathBuf, PathBuf)> {
        let original = self.storage.data.root().join(row.audio_path.as_deref()?);
        if !original.is_file() {
            return None;
        }
        let folder = self.storage.data.trash_dir();
        let parked = folder.join(original.file_name()?);
        match fs::create_dir_all(&folder).and_then(|()| fs::rename(&original, &parked)) {
            Ok(()) => Some((original, parked)),
            Err(error) => {
                log::warn!("the audio of a deleted transcript could not be set aside: {error}");
                // The row is gone from the database, so the file is only an orphan.
                let _ = fs::remove_file(&original);
                None
            }
        }
    }

    pub fn ask_clear(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.total == 0 {
            return self.refuse("There are no transcripts to clear.".into(), cx);
        }
        self.confirm_clear = true;
        self.message = None;
        cx.notify();
        Ok(())
    }

    pub fn cancel_clear(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.confirm_clear) {
            cx.notify();
        }
    }

    /// Removes every row and every audio file. Only after the confirmation.
    pub fn confirm_clear(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if !self.confirm_clear {
            return self.refuse("Choose Clear all first.".into(), cx);
        }
        self.confirm_clear = false;
        self.playback = None;
        self.finish_undo();
        self.undo_generation += 1;
        match history::clear(&self.storage.database) {
            Ok(paths) => {
                for path in paths {
                    let file = self.storage.data.root().join(path);
                    if let Err(error) = fs::remove_file(&file)
                        && file.exists()
                    {
                        log::warn!("an audio file of the history was not removed: {error}");
                    }
                }
                sweep_trash(&self.storage);
            }
            Err(error) => {
                return self.refuse(format!("The history could not be cleared: {error}"), cx);
            }
        }
        self.detail = None;
        self.message = None;
        self.selection.clear();
        self.reload(cx);
        hook::record_event("history", "cleared");
        Ok(())
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
                "insert_outcome": row.insert_outcome,
                "audio_removed": row.audio_removed_at.is_some(),
                "selected": self.is_selected(&row.id),
            })).collect::<Vec<_>>(),
            "detail": self.detail.as_ref().map(|row| json!({
                "id": row.id,
                "status": row.status,
                "error_code": row.error_code,
                "raw_text": row.raw_text,
                "rule_text": row.rule_text,
                "llm_text": row.llm_text,
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

/// Removes audio that a delete set aside in an earlier run of the app.
fn sweep_trash(storage: &Storage) {
    let Ok(entries) = fs::read_dir(storage.data.trash_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        if let Err(error) = fs::remove_file(entry.path()) {
            log::warn!("a set-aside audio file was not removed: {error}");
        }
    }
}

#[cfg(test)]
mod tests;
