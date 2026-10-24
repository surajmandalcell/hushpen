//! The list of the History view: the page reads, the search, and the rows that are drawn.

use super::{History, PAGE, ROW_HEIGHT};
use crate::hook;
use gpui_kit::{Context, FocusHandle, Point, Window, px};
use hushpen_store::history::{self, Row};
use std::ops::Range;
use std::time::Instant;

/// Where the first row starts below the top of the scrolled content, and the most that banners
/// can add to that.
const LIST_TOP: f32 = 49.0;
const LIST_TOP_MAX: f32 = 220.0;
/// Rows kept ready above and below the screen.
const BUFFER: usize = 3;
/// How close to the end of the list, in pixels, the next page is asked for.
const NEXT_PAGE_REACH: f32 = 160.0;
const FALLBACK_VIEWPORT: f32 = 486.0;

impl History {
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

    /// Reads the first page again, with the search that is typed.
    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
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
            let row = history::get(&self.storage.database, &open.id)
                .ok()
                .flatten();
            self.detail_segments = self.segments_of(row.as_ref());
            self.detail = row;
        }
        cx.notify();
    }

    /// The timed pieces of an import. A dictation has no use for them in the detail view.
    pub(super) fn segments_of(&self, row: Option<&Row>) -> Vec<history::Segment> {
        let Some(row) = row.filter(|row| row.kind == "import") else {
            return Vec::new();
        };
        history::segments(&self.storage.database, &row.id).unwrap_or_default()
    }

    pub fn detail_segments(&self) -> &[history::Segment] {
        &self.detail_segments
    }

    pub(super) fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
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
}
