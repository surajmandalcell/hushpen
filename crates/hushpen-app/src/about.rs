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
    Licenses,
}

impl Target {
    pub fn key(self) -> &'static str {
        match self {
            Target::DataFolder => "data-folder",
            Target::LogFolder => "log-folder",
            Target::Github => "github",
            Target::Issues => "issues",
            Target::Licenses => "licenses",
        }
    }
}

const NOTICES_FILE: &str = "THIRD-PARTY-NOTICES.html";

/// The notices file of this install, given the folder of the running binary. Packages put it
/// beside the binary (tar.gz, deb, rpm), in `Contents/Resources` (.app), or in `usr/lib/hushpen`
/// (AppImage, whose binary is in `usr/bin`).
fn locate_notices(exe_dir: &std::path::Path) -> Option<PathBuf> {
    [
        exe_dir.join(NOTICES_FILE),
        exe_dir.join("../Resources").join(NOTICES_FILE),
        exe_dir.join("../lib/hushpen").join(NOTICES_FILE),
    ]
    .into_iter()
    .find(|path| path.is_file())
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
    /// The five row controls, keyboard reachable.
    pub(crate) focus: Vec<gpui_kit::FocusHandle>,
    notices: Option<PathBuf>,
}

impl About {
    pub fn new(storage: Rc<Storage>, opener: Rc<dyn Opener>, cx: &mut Context<Self>) -> Self {
        Self {
            storage,
            opener,
            notice: None,
            requests: Rc::new(RefCell::new(Vec::new())),
            focus: (0..5).map(|_| cx.focus_handle().tab_stop(true)).collect(),
            notices: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.canonicalize().ok())
                .and_then(|exe| locate_notices(exe.parent()?)),
        }
    }

    #[cfg(test)]
    fn set_notices(&mut self, path: Option<PathBuf>) {
        self.notices = path;
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
        let url = match target {
            Target::DataFolder => self.storage.data.root().display().to_string(),
            Target::LogFolder => {
                // A deleted logs folder comes back before the file manager opens it.
                let logs = self.storage.data.logs_dir();
                if let Err(error) = std::fs::create_dir_all(&logs) {
                    log::warn!("the log folder could not be created: {error}");
                }
                logs.display().to_string()
            }
            Target::Github => hushpen_core::links::REPO_URL.to_owned(),
            Target::Issues => hushpen_core::links::ISSUES_URL.to_owned(),
            Target::Licenses => self
                .notices
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
        };
        let result = match target {
            Target::DataFolder | Target::LogFolder => {
                self.opener.open_path(std::path::Path::new(&url))
            }
            Target::Github | Target::Issues => self.opener.open_url(&url),
            Target::Licenses if url.is_empty() => {
                Err("the notices file is not part of this install".to_owned())
            }
            Target::Licenses => self.opener.open_path(std::path::Path::new(&url)),
        };
        self.requests.borrow_mut().push(json!({
            "target": target.key(),
            "url": url,
            "ok": result.is_ok(),
        }));
        hook::record_event("about", &format!("open {}", target.key()));
        self.notice = result.err().map(|error| match target {
            Target::Github | Target::Issues => {
                format!("The browser could not be opened: {error}")
            }
            Target::Licenses => format!("The licenses could not be opened: {error}"),
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
