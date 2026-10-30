//! The Manual view (code name Files): one place for audio files and for text. A segmented
//! control at the top picks Transcribe (the import queue) or Read aloud, and the choice is kept
//! in settings. Files dropped anywhere on the window land here.

pub mod panel;

use crate::import::Import;
use crate::read_aloud::ReadAloud;
use crate::storage::Storage;
use gpui_kit::{App, Context, Entity, FocusHandle};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const MODE_SETTING: &str = "files.mode";
/// The group name of the window: the drop zone lights up while files are dragged over any part
/// of it.
pub const DROP_GROUP: &str = "window";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Transcribe,
    ReadAloud,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Transcribe, Mode::ReadAloud];

    pub fn key(self) -> &'static str {
        match self {
            Mode::Transcribe => "transcribe",
            Mode::ReadAloud => "readAloud",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Transcribe => "Transcribe",
            Mode::ReadAloud => "Read aloud",
        }
    }
}

pub struct Files {
    storage: Rc<Storage>,
    import: Entity<Import>,
    read_aloud: Option<Entity<ReadAloud>>,
    pub(crate) mode_focus: FocusHandle,
}

impl Files {
    /// `read_aloud` is `None` where the window could not give it a text area.
    pub fn new(
        storage: Rc<Storage>,
        import: Entity<Import>,
        read_aloud: Option<Entity<ReadAloud>>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&import, |_, _, cx| cx.notify()).detach();
        if let Some(read) = &read_aloud {
            cx.observe(read, |_, _, cx| cx.notify()).detach();
        }
        Self {
            storage,
            import,
            read_aloud,
            mode_focus: cx.focus_handle().tab_stop(true),
        }
    }

    pub fn import(&self) -> &Entity<Import> {
        &self.import
    }

    pub fn read_aloud(&self) -> Option<&Entity<ReadAloud>> {
        self.read_aloud.as_ref()
    }

    /// The mode the user last picked.
    pub fn mode(&self) -> Mode {
        let saved = self
            .storage
            .settings
            .get(MODE_SETTING)
            .and_then(|value| value.as_str().and_then(Mode::from_key));
        match saved {
            Some(Mode::ReadAloud) if self.read_aloud.is_some() => Mode::ReadAloud,
            _ => Mode::Transcribe,
        }
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) -> Result<(), String> {
        self.storage
            .settings
            .set(MODE_SETTING, json!(mode.key()))
            .map_err(|error| error.to_string())?;
        cx.notify();
        Ok(())
    }

    /// Files dropped on the window. Text files open in Read aloud; anything else goes in the
    /// queue, which refuses what is not an audio file.
    pub fn drop_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        let text = paths.iter().all(|path| is_text(path));
        let mode = match &self.read_aloud {
            Some(read) if text => {
                let first = paths.into_iter().next();
                read.update(cx, |read, cx| {
                    if let Some(path) = first {
                        read.open_text_file(path, cx);
                    }
                });
                Mode::ReadAloud
            }
            _ => {
                self.import
                    .update(cx, |import, cx| import.add_paths(paths, cx));
                Mode::Transcribe
            }
        };
        // The mode shows even when the setting cannot be written.
        if let Err(error) = self.set_mode(mode, cx) {
            log::warn!("the Files mode was not saved: {error}");
        }
    }

    pub fn state_json(&self, _cx: &App) -> Value {
        json!({ "mode": self.mode().key() })
    }
}

/// `.txt`, `.md`, and `.markdown`: what Read aloud opens.
fn is_text(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        ["txt", "md", "markdown"]
            .iter()
            .any(|text| extension.eq_ignore_ascii_case(text))
    })
}

#[cfg(test)]
mod tests;
