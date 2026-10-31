//! The search of the Models view: the field at the right end of the title row and the rows it
//! keeps. The query is never saved.

use super::panel::{format_size, languages_text};
use super::{Models, Row};
use crate::hook;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AppContext as _, Context, Entity, FocusHandle, Point, ScrollHandle, Window};

/// What the search field holds and the page it scrolls.
#[derive(Default)]
pub(super) struct Search {
    pub field: Option<Entity<InputState>>,
    pub query: String,
    pub scroll: Option<ScrollHandle>,
    /// The Clear button of the "No models match" row.
    pub clear: Option<FocusHandle>,
}

/// Whether `row` matches every word of `query`, in its name, description, languages, size, or
/// the word Recommended. Case does not matter; an empty query matches every row.
pub fn matches(row: &Row, query: &str) -> bool {
    let mut haystack = format!(
        "{} {} {} {}",
        row.entry.name,
        row.entry.id,
        languages_text(row.entry.languages),
        format_size(row.entry.bytes()),
    );
    if let Some(info) = &row.entry.info {
        haystack.push(' ');
        haystack.push_str(&info.description);
    }
    if row.recommended {
        haystack.push_str(" recommended");
    }
    let haystack = haystack.to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

impl Models {
    /// Builds the search field of the title row. It needs a window, so the caller attaches it.
    pub fn attach_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        cx.subscribe_in(&field, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().to_string();
                this.set_query(value, cx);
            }
        })
        .detach();
        self.search.field = Some(field);
        self.search.clear = Some(cx.focus_handle().tab_stop(true));
    }

    /// Shares the scroll position of the content pane, so a new query starts at the top.
    pub fn use_scroll(&mut self, scroll: ScrollHandle) {
        self.search.scroll = Some(scroll);
    }

    /// What the search field holds.
    pub fn query(&self) -> &str {
        &self.search.query
    }

    /// Shows only the rows that match `query`; an empty one shows every row.
    pub fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.search.query == query {
            return;
        }
        self.search.query = query;
        if let Some(scroll) = &self.search.scroll {
            scroll.set_offset(Point::default());
        }
        hook::record_event("models", "search");
        cx.notify();
    }

    /// Types `query` into the search field like a user does.
    pub fn search_for(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(field) = &self.search.field {
            field.update(cx, |input, cx| {
                input.set_value(query.to_owned(), window, cx)
            });
        }
        self.set_query(query.to_owned(), cx);
    }

    /// Esc in the search field: clears a query and says so. With nothing typed it returns false,
    /// and the caller moves the focus away.
    pub fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.search.query.is_empty() {
            return false;
        }
        self.search_for("", window, cx);
        true
    }

    /// The search field, once attached.
    pub fn search_field_state(&self) -> Option<&Entity<InputState>> {
        self.search.field.as_ref()
    }
}
