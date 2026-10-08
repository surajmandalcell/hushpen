//! The About view: version, data folder, and the rows that open a folder or
//! a page of the repository. Every open goes through the platform and is
//! recorded for the test hook.

mod panel;

use crate::hook;
use crate::storage::Storage;
use gpui_kit::Context;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub(crate) use panel::render;

/// What an About row opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    DataFolder,
    LogFolder,
    Github,
    Issues,
}

impl Target {
    pub fn key(self) -> &'static str {
        match self {
            Target::DataFolder => "data-folder",
            Target::LogFolder => "log-folder",
            Target::Github => "github",
            Target::Issues => "issues",
        }
    }
}

/// Opening a folder or a URL, behind a trait so tests watch the calls.
pub trait Opener {
    fn open_url(&self, url: &str) -> Result<(), String>;
    fn open_path(&self, path: &std::path::Path) -> Result<(), String>;
}

pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open_url(&self, url: &str) -> Result<(), String> {
        hushpen_platform::permissions::open_url(url).map_err(|error| error.to_string())
    }

    fn open_path(&self, path: &std::path::Path) -> Result<(), String> {
        hushpen_platform::permissions::open_path(path).map_err(|error| error.to_string())
    }
}

pub struct About {
    pub(crate) storage: Rc<Storage>,
    opener: Rc<dyn Opener>,
    pub(crate) notice: Option<String>,
    /// Every open request, for `hookctl state` section `about`.
    requests: Rc<RefCell<Vec<Value>>>,
    /// The four row controls, keyboard reachable.
    pub(crate) focus: Vec<gpui_kit::FocusHandle>,
}

impl About {
    pub fn new(storage: Rc<Storage>, opener: Rc<dyn Opener>, cx: &mut Context<Self>) -> Self {
        Self {
            storage,
            opener,
            notice: None,
            requests: Rc::new(RefCell::new(Vec::new())),
            focus: (0..4).map(|_| cx.focus_handle().tab_stop(true)).collect(),
        }
    }

    pub fn version(&self) -> &'static str {
        hushpen_core::BUILD_VERSION
    }

    /// The data folder as the user reads it: `~/...` under home.
    pub fn data_folder_display(&self) -> String {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        hushpen_core::paths::display(self.storage.data.root(), home.as_deref())
    }

    pub fn open(&mut self, target: Target, cx: &mut Context<Self>) {
        let result = match target {
            Target::DataFolder => self.opener.open_path(self.storage.data.root()),
            Target::LogFolder => self.opener.open_path(&self.storage.data.logs_dir()),
            Target::Github => self.opener.open_url(hushpen_core::links::REPO_URL),
            Target::Issues => self.opener.open_url(hushpen_core::links::ISSUES_URL),
        };
        self.requests.borrow_mut().push(json!({
            "target": target.key(),
            "ok": result.is_ok(),
        }));
        hook::record_event("about", &format!("open {}", target.key()));
        self.notice = result.err().map(|error| match target {
            Target::Github | Target::Issues => {
                format!("The browser could not be opened: {error}")
            }
            _ => format!("The folder could not be opened: {error}"),
        });
        cx.notify();
    }

    /// `hookctl state` section `about`: version, folders, and the open
    /// requests so a test sees what a click opened. The hook module wires it
    /// once the hook is attached.
    pub fn hook_state_json(&self) -> Value {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        json!({
            "version": self.version(),
            "data_folder": hushpen_core::paths::display(
                self.storage.data.root(),
                home.as_deref()
            ),
            "log_folder": hushpen_core::paths::display(
                &self.storage.data.logs_dir(),
                home.as_deref()
            ),
            "open-requests": self.requests.borrow().clone(),
        })
    }
}

#[cfg(test)]
mod tests;
