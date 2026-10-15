//! The opt-in update check: when it runs, what it found, and what the views show about it.
//!
//! A check runs only while `updates.check` is on, at most once every 24 hours on its own, and
//! at once when the user presses Check now. It sends no identifier. The download and the swap
//! of a new version start from [`Updater::install`].

mod panel;

use crate::about::Opener;
use crate::hook;
use crate::net::update::{CheckError, UpdateFeed};
use crate::storage::Storage;
use gpui_kit::{Context, FocusHandle};
use hushpen_core::error::UPDATE_SIGNATURE_INVALID;
use hushpen_core::update::{Manifest, Offer, Version, check_due};
use serde_json::{Value, json};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub(crate) use panel::{banner, rows};

const CHECK: &str = "updates.check";
const AUTO_INSTALL: &str = "updates.autoInstall";
const LAST_CHECK: &str = "updates.lastCheckAt";
const DISMISSED: &str = "updates.dismissedVersion";

/// How often a running app looks at the clock for the next automatic check.
const RECHECK_EVERY: Duration = Duration::from_secs(60 * 60);

/// The two things the banner can press, as indexes into [`Updater::focus`].
pub(crate) const BANNER_INSTALL: usize = 0;
pub(crate) const BANNER_DISMISS: usize = 1;

/// Asks the feed. The real one talks to GitHub; tests pass a fake.
pub trait Checker: Send + Sync {
    fn check(&self, current: &Version) -> Result<Offer, CheckError>;
}

pub struct FeedChecker;

impl Checker for FeedChecker {
    fn check(&self, current: &Version) -> Result<Offer, CheckError> {
        let feed = UpdateFeed::current()?;
        feed.check(&feed.fetcher(), current)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// No check ran in this session.
    Idle,
    Checking,
    UpToDate,
    Available(Manifest),
    Downloading(Manifest, Option<u8>),
    Ready(Manifest),
    Error(CheckError),
}

impl State {
    pub fn key(&self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Checking => "checking",
            State::UpToDate => "up_to_date",
            State::Available(_) => "available",
            State::Downloading(..) => "downloading",
            State::Ready(_) => "ready",
            State::Error(_) => "error",
        }
    }

    /// The newer version this state is about, if any.
    pub fn manifest(&self) -> Option<&Manifest> {
        match self {
            State::Available(manifest)
            | State::Downloading(manifest, _)
            | State::Ready(manifest) => Some(manifest),
            _ => None,
        }
    }
}

pub struct Parts {
    pub storage: Rc<Storage>,
    pub checker: Arc<dyn Checker>,
    pub opener: Rc<dyn Opener>,
    /// Milliseconds since 1970.
    pub clock: Rc<dyn Fn() -> u64>,
    /// The running version.
    pub current: Version,
}

pub fn wall_clock() -> Rc<dyn Fn() -> u64> {
    Rc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64)
    })
}

pub struct Updater {
    storage: Rc<Storage>,
    checker: Arc<dyn Checker>,
    opener: Rc<dyn Opener>,
    clock: Rc<dyn Fn() -> u64>,
    current: Version,
    state: State,
    /// Counts the checks, so the answer to an older one is ignored.
    runs: u64,
    installs: u32,
    /// The last release notes open, for `hookctl state`.
    last_open: Option<Value>,
    /// The banner buttons, keyboard reachable.
    pub(crate) focus: Vec<FocusHandle>,
}

impl Updater {
    pub fn new(parts: Parts, cx: &mut Context<Self>) -> Self {
        Self {
            storage: parts.storage,
            checker: parts.checker,
            opener: parts.opener,
            clock: parts.clock,
            current: parts.current,
            state: State::Idle,
            runs: 0,
            installs: 0,
            last_open: None,
            focus: (0..2).map(|_| cx.focus_handle().tab_stop(true)).collect(),
        }
    }

    /// The app started: run the check if it is on and due, and look at the clock every hour
    /// after that.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.check_if_due(cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(RECHECK_EVERY).await;
                if this.update(cx, |me, cx| me.check_if_due(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn current(&self) -> &Version {
        &self.current
    }

    pub fn checks_on(&self) -> bool {
        self.stored_bool(CHECK)
    }

    pub fn auto_install(&self) -> bool {
        self.stored_bool(AUTO_INSTALL)
    }

    fn stored_bool(&self, key: &str) -> bool {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    fn last_check(&self) -> u64 {
        self.storage
            .settings
            .get(LAST_CHECK)
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
    }

    /// Settings changed `updates.check` or `updates.autoInstall`.
    pub fn settings_changed(&mut self, cx: &mut Context<Self>) {
        if self.checks_on() {
            self.check_if_due(cx);
        } else if self.state == State::Checking {
            // The answer to this check is ignored, and the person said no more requests.
            self.runs += 1;
            self.set_state(State::Idle, cx);
        }
        if self.auto_install() {
            self.install(cx);
        }
        cx.notify();
    }

    fn check_if_due(&mut self, cx: &mut Context<Self>) {
        if self.checks_on() && check_due(self.last_check(), (self.clock)()) {
            self.begin_check(cx);
        }
    }

    /// The Check now button. It ignores the 24 hour rule and does nothing while the check is off.
    pub fn check_now(&mut self, cx: &mut Context<Self>) {
        if self.checks_on() {
            self.begin_check(cx);
        }
    }

    pub fn can_check_now(&self) -> bool {
        self.checks_on() && !matches!(self.state, State::Checking | State::Downloading(..))
    }

    fn begin_check(&mut self, cx: &mut Context<Self>) {
        if !self.can_check_now() {
            return;
        }
        self.runs += 1;
        let run = self.runs;
        // Stored when the request starts, so a failing network is not asked again on every start.
        let now = json!((self.clock)());
        if let Err(error) = self.storage.settings.set_internal(LAST_CHECK, now) {
            log::warn!("the time of the update check could not be saved: {error}");
        }
        self.set_state(State::Checking, cx);
        let (checker, current) = (Arc::clone(&self.checker), self.current.clone());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { checker.check(&current) })
                .await;
            let _ = this.update(cx, |me, cx| me.finish_check(run, result, cx));
        })
        .detach();
    }

    fn finish_check(
        &mut self,
        run: u64,
        result: Result<Offer, CheckError>,
        cx: &mut Context<Self>,
    ) {
        if run != self.runs {
            return;
        }
        let state = match result {
            Ok(Offer::UpToDate) => State::UpToDate,
            Ok(Offer::Available(manifest)) => State::Available(manifest),
            Err(error) => {
                log::warn!("{} {}", error.code, error.detail);
                State::Error(error)
            }
        };
        let available = matches!(state, State::Available(_));
        self.set_state(state, cx);
        if available && self.auto_install() {
            self.install(cx);
        }
    }

    fn set_state(&mut self, state: State, cx: &mut Context<Self>) {
        if self.state != state {
            hook::record_event("updates", &format!("state {}", state.key()));
        }
        self.state = state;
        cx.notify();
    }

    /// The download has begun. Called by the code that downloads the new version.
    pub fn downloading(&mut self, percent: Option<u8>, cx: &mut Context<Self>) {
        if let Some(manifest) = self.state.manifest().cloned() {
            self.set_state(State::Downloading(manifest, percent), cx);
        }
    }

    /// The new version is downloaded, checked, and staged. It starts after a restart.
    pub fn ready(&mut self, cx: &mut Context<Self>) {
        if let Some(manifest) = self.state.manifest().cloned() {
            self.set_state(State::Ready(manifest), cx);
        }
    }

    /// The download or the swap failed. The old version keeps running.
    pub fn failed(&mut self, error: CheckError, cx: &mut Context<Self>) {
        self.set_state(State::Error(error), cx);
    }

    /// "Download and install", or the automatic install. Only an offered version can start.
    pub fn install(&mut self, cx: &mut Context<Self>) {
        let State::Available(manifest) = &self.state else {
            return;
        };
        self.installs += 1;
        hook::record_event(
            "updates",
            &format!("install requested {}", manifest.version.as_str()),
        );
        cx.notify();
    }

    /// "Not now" on the banner: the banner stays away for this version.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        let Some(manifest) = self.state.manifest() else {
            return;
        };
        let version = manifest.version.as_str().to_owned();
        let stored = json!(version);
        if let Err(error) = self.storage.settings.set_internal(DISMISSED, stored) {
            log::warn!("the dismissed update could not be saved: {error}");
        }
        hook::record_event("updates", &format!("dismissed {version}"));
        cx.notify();
    }

    /// Whether the banner is on screen: a version is offered, the person chose to install by
    /// hand, and has not sent it away.
    pub fn banner_visible(&self) -> bool {
        let State::Available(manifest) = &self.state else {
            return false;
        };
        let dismissed = self
            .storage
            .settings
            .get(DISMISSED)
            .and_then(|value| value.as_str().map(str::to_owned));
        !self.auto_install() && dismissed.as_deref() != Some(manifest.version.as_str())
    }

    /// Opens the release notes of the offered version in the browser.
    pub fn open_notes(&mut self, cx: &mut Context<Self>) {
        let Some(manifest) = self.state.manifest() else {
            return;
        };
        let url = manifest.notes_url.clone();
        let result = self.opener.open_url(&url);
        hook::record_event("updates", "open release notes");
        self.last_open = Some(json!({"url": url, "ok": result.is_ok()}));
        if let Err(error) = result {
            log::warn!("the release notes could not be opened: {error}");
        }
        cx.notify();
    }

    /// The words under the status row of Settings > Updates.
    pub fn status_text(&self) -> String {
        match &self.state {
            State::Idle if self.checks_on() => "Not checked yet".to_owned(),
            State::Idle => "Update checks are off".to_owned(),
            State::Checking => "Checking for updates".to_owned(),
            State::UpToDate => "Hushpen is up to date".to_owned(),
            State::Available(manifest) => {
                format!("Version {} is available", manifest.version.as_str())
            }
            State::Downloading(manifest, Some(percent)) => {
                format!(
                    "Downloading version {} ({percent}%)",
                    manifest.version.as_str()
                )
            }
            State::Downloading(manifest, None) => {
                format!("Downloading version {}", manifest.version.as_str())
            }
            State::Ready(manifest) => {
                format!("Version {} is ready", manifest.version.as_str())
            }
            State::Error(error) if error.code == UPDATE_SIGNATURE_INVALID => {
                "The update could not be verified".to_owned()
            }
            State::Error(_) => "The update check failed".to_owned(),
        }
    }

    /// `hookctl state` section `updates`. Codes and detail live here, and in the log, but not in
    /// the view.
    pub fn hook_state_json(&self) -> Value {
        json!({
            "state": self.state.key(),
            "current": self.current.as_str(),
            "version": self.state.manifest().map(|manifest| manifest.version.as_str().to_owned()),
            "notes_url": self.state.manifest().map(|manifest| manifest.notes_url.clone()),
            "error": match &self.state {
                State::Error(error) => json!({"code": error.code, "detail": error.detail}),
                _ => Value::Null,
            },
            "check": self.checks_on(),
            "auto_install": self.auto_install(),
            "last_check_at": self.last_check(),
            "banner": self.banner_visible(),
            "install_requests": self.installs,
            "last_open": self.last_open,
        })
    }
}

#[cfg(test)]
mod tests;
