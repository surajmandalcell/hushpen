//! Choosing rows in the History view and writing them to one TXT, SRT, VTT, or JSON file.

use super::{History, save_dialog};
use crate::hook;
use gpui_kit::Context;
use hushpen_core::export::{self, Format, Item};
use hushpen_store::history::{self, Row};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// The file that the last export wrote.
#[derive(Debug, Clone)]
pub struct Exported {
    pub path: PathBuf,
    pub format: Format,
    pub rows: usize,
    pub bytes: usize,
}

impl Exported {
    pub fn json(&self) -> Value {
        json!({
            "path": self.path.display().to_string(),
            "format": self.format.extension(),
            "rows": self.rows,
            "bytes": self.bytes,
        })
    }
}

impl History {
    pub fn selecting(&self) -> bool {
        self.selecting
    }

    pub fn selection(&self) -> &[String] {
        &self.selection
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selection.iter().any(|selected| selected == id)
    }

    pub fn export_menu_open(&self) -> bool {
        self.export_menu
    }

    pub fn exporting(&self) -> bool {
        self.export_busy
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn last_export(&self) -> Option<&Exported> {
        self.last_export.as_ref()
    }

    /// Turns the selecting mode on or off. Leaving it forgets the selection.
    pub fn set_selecting(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.selecting == on {
            return;
        }
        self.selecting = on;
        if !on {
            self.selection.clear();
        }
        self.export_menu = false;
        self.message = None;
        hook::record_event("history", if on { "select-on" } else { "select-off" });
        cx.notify();
    }

    /// Adds the row to the selection, or takes it out when it is in.
    pub fn toggle_selected(&mut self, id: &str, cx: &mut Context<Self>) {
        self.selecting = true;
        match self.selection.iter().position(|selected| selected == id) {
            Some(at) => {
                self.selection.remove(at);
            }
            None => self.selection.push(id.to_owned()),
        }
        self.message = None;
        hook::record_event("history", &format!("selected {}", self.selection.len()));
        cx.notify();
    }

    /// Makes `ids` the whole selection.
    pub fn select_only(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.selecting = true;
        self.selection.clear();
        for id in ids {
            if !self.selection.contains(&id) {
                self.selection.push(id);
            }
        }
        self.message = None;
        hook::record_event("history", &format!("selected {}", self.selection.len()));
        cx.notify();
    }

    /// Selects every row that is loaded, or none when all of them are already selected.
    pub fn toggle_select_all(&mut self, cx: &mut Context<Self>) {
        let loaded: Vec<String> = self.rows.iter().map(|row| row.id.clone()).collect();
        let all = !loaded.is_empty() && loaded.iter().all(|id| self.is_selected(id));
        self.select_only(if all { Vec::new() } else { loaded }, cx);
    }

    pub fn toggle_export_menu(&mut self, cx: &mut Context<Self>) {
        self.export_menu = !self.export_menu;
        self.message = None;
        cx.notify();
    }

    /// Takes a row that is gone from the selection.
    pub(super) fn forget_selected(&mut self, id: &str) {
        self.selection.retain(|selected| selected != id);
    }

    /// Writes the chosen rows to one file in `format`. The rows are `ids`, or the selection,
    /// or the open transcript, in that order. The save dialog asks for the path; the file is
    /// written when it answers, and the state shows the result.
    pub fn export(
        &mut self,
        format: Format,
        ids: Option<Vec<String>>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.export_busy {
            return self.refuse("A save dialog is already open.".into(), cx);
        }
        let ids = match ids.filter(|ids| !ids.is_empty()) {
            Some(ids) => ids,
            None if !self.selection.is_empty() => self.selection.clone(),
            None => match &self.detail {
                Some(open) => vec![open.id.clone()],
                None => return self.refuse("Select transcripts to export first.".into(), cx),
            },
        };
        let items = match self.items(&ids) {
            Ok(items) => items,
            Err(message) => return self.refuse(message, cx),
        };
        if format != Format::Json && !export::has_text(&items) {
            let message = match items.len() {
                1 => "This transcript has no text to export.",
                _ => "None of the selected transcripts has text to export.",
            };
            return self.refuse(message.into(), cx);
        }
        let content = export::render(format, &items);
        let name = suggested_name(format, &items);
        let directory = self.export_dir.clone().unwrap_or_else(default_directory);
        let rows = items.len();
        self.export_busy = true;
        self.message = None;
        self.notice = None;
        hook::record_event("history", &format!("export-dialog {}", format.extension()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let chosen = save_dialog::choose(cx, directory, name).await;
            let _ = this.update(cx, |me, cx| {
                me.export_busy = false;
                me.finish_export(chosen, format, rows, &content, cx);
            });
        })
        .detach();
        Ok(())
    }

    fn finish_export(
        &mut self,
        chosen: Result<Option<PathBuf>, String>,
        format: Format,
        rows: usize,
        content: &str,
        cx: &mut Context<Self>,
    ) {
        match chosen {
            Ok(None) => hook::record_event("history", "export-cancelled"),
            Err(message) => {
                self.message = Some(message);
                hook::record_event("history", "export-failed dialog");
            }
            Ok(Some(path)) => {
                let path = with_extension(path, format);
                match fs::write(&path, content) {
                    Ok(()) => {
                        self.export_dir = path.parent().map(Path::to_path_buf);
                        self.notice = Some(format!(
                            "Exported {rows} {} to {}.",
                            if rows == 1 {
                                "transcript"
                            } else {
                                "transcripts"
                            },
                            shown_path(&path)
                        ));
                        hook::record_event(
                            "history",
                            &format!("export-written {}", path.display()),
                        );
                        self.last_export = Some(Exported {
                            path,
                            format,
                            rows,
                            bytes: content.len(),
                        });
                        self.export_menu = false;
                    }
                    Err(error) => {
                        log::warn!("the export file was not written: {error}");
                        self.message = Some(format!(
                            "The file could not be written to {}: {}",
                            shown_path(&path),
                            error
                        ));
                        hook::record_event("history", "export-failed write");
                    }
                }
            }
        }
        cx.notify();
    }

    fn items(&self, ids: &[String]) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        for id in ids {
            let Some(row) =
                history::get(&self.storage.database, id).map_err(|error| error.to_string())?
            else {
                continue;
            };
            let segments =
                history::segments(&self.storage.database, id).map_err(|error| error.to_string())?;
            items.push(item_of(&row, &segments));
        }
        if items.is_empty() {
            return Err("Those transcripts are no longer in the history.".to_owned());
        }
        Ok(items)
    }
}

fn item_of(row: &Row, segments: &[history::Segment]) -> Item {
    Item {
        id: row.id.clone(),
        created_at_ms: row.created_at,
        kind: row.kind.clone(),
        status: row.status.clone(),
        error_code: row.error_code.clone(),
        duration_ms: row.duration_ms,
        model_id: row.model_id.clone(),
        language_requested: row.language_requested.clone(),
        language_detected: row.language_detected.clone(),
        target_app: row.target_app.clone(),
        raw_text: row.raw_text.clone(),
        final_text: row.final_text.clone(),
        segments: segments
            .iter()
            .map(|segment| export::Segment {
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                text: segment.text.clone(),
            })
            .collect(),
    }
}

/// `hushpen-2026-10-08-1405.srt` for one transcript, `hushpen-3-transcripts.srt` for more.
fn suggested_name(format: Format, items: &[Item]) -> String {
    let stem = match items {
        [only] => {
            let iso = export::iso_utc(only.created_at_ms);
            format!("hushpen-{}-{}{}", &iso[..10], &iso[11..13], &iso[14..16])
        }
        many => format!("hushpen-{}-transcripts", many.len()),
    };
    format!("{stem}.{}", format.extension())
}

/// The dialog may return a name with no extension; the format decides it then.
fn with_extension(path: PathBuf, format: Format) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension(format.extension())
    }
}

fn default_directory() -> PathBuf {
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// `~/...` for a path under the home folder.
fn shown_path(path: &Path) -> String {
    std::env::home_dir()
        .and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf))
        .map_or_else(
            || path.display().to_string(),
            |rest| format!("~/{}", rest.display()),
        )
}
