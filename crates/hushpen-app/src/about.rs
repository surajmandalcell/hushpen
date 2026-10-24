//! The About view: the app header with the build stamp and Check for Updates, the facts of this
//! install, and the links. Every open goes through the platform and is recorded for the test
//! hook.

mod panel;

use crate::hook;
use crate::models::Models;
use crate::storage::Storage;
use crate::updater::{State, Updater};
use gpui_kit::{App, Context, Entity, FocusHandle};
use hushpen_core::links;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub(crate) use panel::render;

/// The line under the name.
pub const TAGLINE: &str = if cfg!(target_os = "macos") {
    "Private dictation that stays on this Mac"
} else {
    "Private dictation that stays on this computer"
};

/// The footer line.
pub const COPYRIGHT: &str = "Copyright (c) 2026 Suraj Mandal. Licensed under AGPL-3.0-or-later.";

/// The speech engine the Engine fact names.
const ENGINE: &str = "whisper.cpp";

/// What this binary is built from: the version, and the short commit and its date when the build
/// knew them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Build {
    pub version: &'static str,
    pub commit: Option<&'static str>,
    pub date: Option<&'static str>,
}

impl Build {
    /// This binary: the build version, and the commit and the date `build.rs` stamped in.
    pub fn current() -> Self {
        let stamp = |value: &'static str| Some(value).filter(|value| !value.is_empty());
        Self {
            version: hushpen_core::BUILD_VERSION,
            commit: stamp(env!("HUSHPEN_BUILD_COMMIT")),
            date: stamp(env!("HUSHPEN_BUILD_DATE")),
        }
    }

    /// "Version 2.0.0 (1a2b3c4, 2026-05-15)", without the parts the build did not know.
    pub fn line(&self) -> String {
        let stamp: Vec<&str> = [self.commit, self.date].into_iter().flatten().collect();
        if stamp.is_empty() {
            format!("Version {}", self.version)
        } else {
            format!("Version {} ({})", self.version, stamp.join(", "))
        }
    }
}

/// What a control of About opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    DataFolder,
    Website,
    Github,
    Issues,
    Licenses,
    LogFolder,
}

impl Target {
    pub fn key(self) -> &'static str {
        match self {
            Target::DataFolder => "data-folder",
            Target::Website => "website",
            Target::Github => "github",
            Target::Issues => "issues",
            Target::Licenses => "licenses",
            Target::LogFolder => "log-folder",
        }
    }

    /// Its control in [`About::focus`], in screen order after Check for Updates.
    fn slot(self) -> usize {
        match self {
            Target::DataFolder => 1,
            Target::Website => 2,
            Target::Github => 3,
            Target::Issues => 4,
            Target::Licenses => 5,
            Target::LogFolder => 6,
        }
    }
}

/// The slot of Check for Updates in [`About::focus`].
const CHECK_SLOT: usize = 0;
const SLOTS: usize = 7;

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
    build: Build,
    models: Option<Entity<Models>>,
    updater: Option<Entity<Updater>>,
    pub(crate) notice: Option<String>,
    /// Every open request, for `hookctl state` section `about`.
    requests: Rc<RefCell<Vec<Value>>>,
    /// Check for Updates, the Show button of the data folder, and the links, keyboard reachable
    /// in that order.
    pub(crate) focus: Vec<FocusHandle>,
    notices: Option<PathBuf>,
}

impl About {
    pub fn new(
        storage: Rc<Storage>,
        opener: Rc<dyn Opener>,
        build: Build,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            storage,
            opener,
            build,
            models: None,
            updater: None,
            notice: None,
            requests: Rc::new(RefCell::new(Vec::new())),
            focus: (0..SLOTS)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            notices: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.canonicalize().ok())
                .and_then(|exe| locate_notices(exe.parent()?)),
        }
    }

    /// The models, for the one in use in the Engine fact.
    pub fn attach_models(&mut self, models: Entity<Models>, cx: &mut Context<Self>) {
        cx.observe(&models, |_, _, cx| cx.notify()).detach();
        self.models = Some(models);
        cx.notify();
    }

    /// The update check behind Check for Updates. Without it the button stays off.
    pub fn attach_updater(&mut self, updater: Entity<Updater>, cx: &mut Context<Self>) {
        cx.observe(&updater, |_, _, cx| cx.notify()).detach();
        self.updater = Some(updater);
        cx.notify();
    }

    #[cfg(test)]
    fn set_notices(&mut self, path: Option<PathBuf>) {
        self.notices = path;
    }

    pub fn version(&self) -> &'static str {
        self.build.version
    }

    pub fn build(&self) -> Build {
        self.build
    }

    /// The data folder as the user reads it: `~/...` under home.
    pub fn data_folder_display(&self) -> String {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        hushpen_core::paths::display(self.storage.data.root(), home.as_deref())
    }

    /// The Engine fact: the speech engine and the model a dictation uses.
    pub fn engine(&self, cx: &App) -> String {
        let model = self.models.as_ref().and_then(|models| {
            let models = models.read(cx);
            let id = models.effective();
            models
                .rows()
                .iter()
                .find(|row| row.entry.id == id)
                .map(|row| row.entry.name.clone())
        });
        match model {
            Some(name) => format!("{ENGINE}, {name}"),
            None => ENGINE.to_owned(),
        }
    }

    /// Check for Updates: one check now, even while the automatic check is off.
    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        let Some(updater) = self.updater.clone() else {
            return;
        };
        hook::record_event("about", "check for updates");
        updater.update(cx, |updater, cx| updater.check_once(cx));
    }

    pub fn can_check_for_updates(&self, cx: &App) -> bool {
        self.updater
            .as_ref()
            .is_some_and(|updater| updater.read(cx).can_check_once())
    }

    /// What the last check found, and whether it failed. Nothing before the first check.
    pub fn update_status(&self, cx: &App) -> Option<(String, bool)> {
        let updater = self.updater.as_ref()?.read(cx);
        match updater.state() {
            State::Idle => None,
            state => Some((updater.status_text(), matches!(state, State::Error(_)))),
        }
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
            Target::Website => links::SITE_URL.to_owned(),
            Target::Github => links::REPO_URL.to_owned(),
            Target::Issues => links::ISSUES_URL.to_owned(),
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
            Target::Website | Target::Github | Target::Issues => self.opener.open_url(&url),
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
            Target::Website | Target::Github | Target::Issues => {
                format!("The browser could not be opened: {error}")
            }
            Target::Licenses => format!("The licenses could not be opened: {error}"),
            Target::DataFolder | Target::LogFolder => {
                format!("The folder could not be opened: {error}")
            }
        });
        cx.notify();
    }

    /// `hookctl state` section `about`: the build, the engine, the folders, and the open
    /// requests so a test sees what a click opened. The hook module wires it once the hook is
    /// attached.
    pub fn hook_state_json(&self, cx: &App) -> Value {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        json!({
            "version": self.version(),
            "commit": self.build.commit,
            "date": self.build.date,
            "engine": self.engine(cx),
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
