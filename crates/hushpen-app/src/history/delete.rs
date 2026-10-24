//! Deleting from the History view: the delete with its undo window, and clearing every row.

use super::History;
use crate::hook;
use crate::storage::Storage;
use gpui_kit::Context;
use hushpen_store::history::{self, Row, Stored};
use std::fs;
use std::path::PathBuf;

/// A deleted row that can still come back.
pub(super) struct Undo {
    pub(super) stored: Stored,
    /// Where the audio file was, and where it waits while the undo window is open.
    pub(super) audio: Option<(PathBuf, PathBuf)>,
}

impl History {
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
}

/// Removes audio that a delete set aside in an earlier run of the app.
pub(super) fn sweep_trash(storage: &Storage) {
    let Ok(entries) = fs::read_dir(storage.data.trash_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        if let Err(error) = fs::remove_file(entry.path()) {
            log::warn!("a set-aside audio file was not removed: {error}");
        }
    }
}
