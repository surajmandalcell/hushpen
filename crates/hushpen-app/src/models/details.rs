//! The details sheet of one model: which row it shows, where focus goes when it opens and
//! closes, and its actions (the source page, the folder, the checksum).

use super::{INFO, Models, Row, files};
use crate::hook;
use gpui_kit::{App, ClipboardItem, Context, FocusHandle, Window};
use hushpen_core::error::MODEL_OPEN_FAILED;

/// The focus handles of the sheet. `trap` is the sheet itself, which keeps Tab inside it.
pub struct SheetFocus {
    pub trap: FocusHandle,
    pub source: FocusHandle,
    pub show: FocusHandle,
    pub copy: FocusHandle,
    pub done: FocusHandle,
}

impl SheetFocus {
    pub(super) fn new(cx: &mut App) -> Self {
        let stop = |cx: &mut App| cx.focus_handle().tab_stop(true);
        Self {
            trap: cx.focus_handle(),
            source: stop(cx),
            show: stop(cx),
            copy: stop(cx),
            done: stop(cx),
        }
    }
}

impl Models {
    /// The row whose details are open.
    pub fn details(&self) -> Option<&Row> {
        self.details.and_then(|index| self.rows.get(index))
    }

    /// Opens the details of row `index` of [`Models::all_rows`] and puts focus on Done.
    pub fn open_details(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        hook::record_event("models", &format!("details {}", row.entry.id));
        self.details = Some(index);
        window.focus(&self.sheet_focus.done, cx);
        cx.notify();
    }

    /// Closes the sheet and gives focus back to the details button that opened it.
    pub fn close_details(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.details.take().and_then(|index| self.rows.get(index)) else {
            return;
        };
        window.focus(&row.focus[INFO], cx);
        cx.notify();
    }

    /// Closes the sheet without moving focus, for a view change that hides it.
    pub fn dismiss_details(&mut self, cx: &mut Context<Self>) {
        if self.details.take().is_some() {
            cx.notify();
        }
    }

    /// Where the open model's files are, as `~/...` when they are under the home folder.
    pub fn location(&self, row: &Row) -> String {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let folder = files::folder(&self.storage.data, &row.entry);
        hushpen_core::paths::display(&folder, home.as_deref())
    }

    /// Opens the model's page on Hugging Face in the browser.
    pub fn open_source(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self
            .details()
            .and_then(|row| row.entry.info.as_ref())
            .map(|info| info.source_page.clone())
        else {
            return;
        };
        let opened = self.opener.open_url(&page);
        self.after_open(opened, "the source page", cx);
    }

    /// Opens the folder of the model, or the closest folder above it that exists yet.
    pub fn show_files(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = self
            .details()
            .map(|row| files::folder(&self.storage.data, &row.entry))
        else {
            return;
        };
        let Some(existing) = folder.ancestors().find(|path| path.is_dir()) else {
            return;
        };
        let opened = self.opener.open_path(existing);
        self.after_open(opened, "the model folder", cx);
    }

    fn after_open(&mut self, opened: Result<(), String>, what: &str, cx: &mut Context<Self>) {
        match opened {
            Ok(()) => hook::record_event("models", &format!("opened {what}")),
            Err(error) => {
                log::warn!("{MODEL_OPEN_FAILED} could not open {what}: {error}");
                self.set_notice(
                    MODEL_OPEN_FAILED,
                    format!("Hushpen could not open {what}: {error}"),
                );
                cx.notify();
            }
        }
    }

    /// Copies the open model's sha256: the hash alone for one file, `sha256sum` lines for a
    /// folder of files.
    pub fn copy_checksum(&self, cx: &mut App) {
        let Some(row) = self.details() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(checksum_text(row)));
        hook::record_event("models", &format!("copied checksum {}", row.entry.id));
    }
}

pub(super) fn checksum_text(row: &Row) -> String {
    match row.entry.files.as_slice() {
        [file] => file.sha256.clone(),
        files => files
            .iter()
            .map(|file| format!("{}  {}", file.sha256, file.file))
            .collect::<Vec<_>>()
            .join("\n"),
    }
}
