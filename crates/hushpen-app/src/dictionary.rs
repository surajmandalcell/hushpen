//! The Dictionary view: words for the whisper prompt, and "heard as" to "write as"
//! replacements for the cleaned text.
//!
//! The entries live in the history database. The dictation controller reads them from there at
//! the start of each dictation, so every change here applies to the next one.

pub mod panel;

use crate::hook;
use crate::storage::Storage;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext as _, Context, Entity, FocusHandle, Window};
use hushpen_core::dictionary::Entry;
use hushpen_store::dictionary as store;
use serde_json::{Value, json};
use std::rc::Rc;

pub struct Dictionary {
    storage: Rc<Storage>,
    entries: Vec<Entry>,
    /// "Write as": the word or phrase.
    phrase: Entity<InputState>,
    /// "Heard as": optional spoken text that this phrase replaces.
    heard: Entity<InputState>,
    /// The entry the form is changing, or `None` while it adds a new one.
    editing: Option<i64>,
    message: Option<String>,
    pub(crate) submit_focus: FocusHandle,
    pub(crate) cancel_focus: FocusHandle,
    /// Edit and Delete of each row.
    pub(crate) row_focus: Vec<[FocusHandle; 2]>,
}

impl Dictionary {
    pub fn new(storage: Rc<Storage>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let phrase =
            cx.new(|cx| InputState::new(window, cx).placeholder("Word or phrase to write"));
        let heard = cx.new(|cx| InputState::new(window, cx).placeholder("Heard as (optional)"));
        for input in [&phrase, &heard] {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    // A refusal shows in the message row.
                    let _ = this.submit(window, cx);
                }
            })
            .detach();
        }
        let mut dictionary = Self {
            storage,
            entries: Vec::new(),
            phrase,
            heard,
            editing: None,
            message: None,
            submit_focus: cx.focus_handle().tab_stop(true),
            cancel_focus: cx.focus_handle().tab_stop(true),
            row_focus: Vec::new(),
        };
        dictionary.reload(cx);
        dictionary
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn editing(&self) -> Option<i64> {
        self.editing
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn phrase_input(&self) -> &Entity<InputState> {
        &self.phrase
    }

    pub fn heard_input(&self) -> &Entity<InputState> {
        &self.heard
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        match store::list(&self.storage.database) {
            Ok(entries) => self.entries = entries,
            Err(error) => self.message = Some(error.to_string()),
        }
        while self.row_focus.len() < self.entries.len() {
            self.row_focus.push([
                cx.focus_handle().tab_stop(true),
                cx.focus_handle().tab_stop(true),
            ]);
        }
        self.row_focus.truncate(self.entries.len());
        if self
            .editing
            .is_some_and(|id| !self.entries.iter().any(|entry| entry.id == id))
        {
            self.editing = None;
        }
        cx.notify();
    }

    fn settle(
        &mut self,
        result: Result<(), store::DictionaryError>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match result {
            Ok(()) => {
                self.message = None;
                self.reload(cx);
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.message = Some(message.clone());
                hook::record_event("dictionary", "refused");
                cx.notify();
                Err(message)
            }
        }
    }

    /// Saves a new entry. A blank `heard_as` makes a word for the prompt only.
    pub fn add(
        &mut self,
        phrase: &str,
        heard_as: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let result = store::add(&self.storage.database, phrase, Some(heard_as)).map(|_| ());
        let saved = self.settle(result, cx);
        if saved.is_ok() {
            hook::record_event("dictionary", "added");
        }
        saved
    }

    pub fn update(
        &mut self,
        id: i64,
        phrase: &str,
        heard_as: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let result = store::update(&self.storage.database, id, phrase, Some(heard_as)).map(|_| ());
        let saved = self.settle(result, cx);
        if saved.is_ok() {
            hook::record_event("dictionary", "edited");
        }
        saved
    }

    pub fn remove(&mut self, id: i64, cx: &mut Context<Self>) -> Result<(), String> {
        let result = store::delete(&self.storage.database, id);
        let removed = self.settle(result, cx);
        if removed.is_ok() {
            hook::record_event("dictionary", "deleted");
        }
        removed
    }

    /// What the Add or Save button does: saves the form and, when it worked, empties it.
    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<(), String> {
        let phrase = self.phrase.read(cx).value().to_string();
        let heard = self.heard.read(cx).value().to_string();
        match self.editing {
            Some(id) => self.update(id, &phrase, &heard, cx)?,
            None => self.add(&phrase, &heard, cx)?,
        }
        self.editing = None;
        self.fill("", "", window, cx);
        self.phrase.update(cx, |input, cx| input.focus(window, cx));
        Ok(())
    }

    /// What an Edit button does: puts the entry into the form.
    pub fn start_edit(
        &mut self,
        id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(entry) = self.entries.iter().find(|entry| entry.id == id).cloned() else {
            return Err("that entry is not in the dictionary".into());
        };
        self.editing = Some(id);
        self.message = None;
        self.fill(
            &entry.phrase,
            entry.heard_as.as_deref().unwrap_or(""),
            window,
            cx,
        );
        self.phrase.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
        Ok(())
    }

    pub fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.take().is_some() {
            self.message = None;
            self.fill("", "", window, cx);
            cx.notify();
        }
    }

    fn fill(&self, phrase: &str, heard: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.phrase.update(cx, |input, cx| {
            input.set_value(phrase.to_owned(), window, cx)
        });
        self.heard.update(cx, |input, cx| {
            input.set_value(heard.to_owned(), window, cx)
        });
    }

    pub fn state_json(&self, cx: &App) -> Value {
        json!({
            "count": self.entries.len(),
            "entries": self.entries.iter().map(|entry| json!({
                "id": entry.id,
                "phrase": entry.phrase,
                "heard_as": entry.heard_as,
            })).collect::<Vec<_>>(),
            "editing": self.editing,
            "message": self.message,
            "form": {
                "phrase": self.phrase.read(cx).value().to_string(),
                "heard_as": self.heard.read(cx).value().to_string(),
            },
            "read_only": self.storage.database.read_only(),
        })
    }
}

#[cfg(test)]
mod tests;
