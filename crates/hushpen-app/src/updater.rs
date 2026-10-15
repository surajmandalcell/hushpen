//! The opt-in update check: when it runs, what it found, and what the views show about it.
//!
//! A check runs only while `updates.check` is on, at most once every 24 hours on its own, and
//! at once when the user presses Check now. It sends no identifier.
//!
//! What an offered version leads to depends on how this copy was installed. A macOS app or an
//! AppImage downloads the file of the signed manifest, checks its sha256, stages it beside the
//! install, and waits; [`Updater::restart_to_update`] then hands the swap to a helper and quits.
//! A deb, rpm, or tar.gz install only shows a notice with the release link. With "Install
//! updates automatically" on, [`Updater::install`] runs by itself and the swap still waits for the
//! restart.

mod install;
mod panel;

use crate::about::Opener;
use crate::hook;
use crate::net::download::Progress;
use crate::net::update::{CheckError, UpdateFeed};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui_kit::{App, Context, FocusHandle};
use hushpen_core::error::{
    UPDATE_DOWNLOAD_FAILED, UPDATE_HASH_MISMATCH, UPDATE_SIGNATURE_INVALID, UPDATE_SWAP_FAILED,
};
use hushpen_core::update::{
    Delivery, Manifest, Offer, Version, check_due, current_target, delivery,
};
use serde_json::{Value, json};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub use install::{Installer, Staged, SystemInstaller};
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

/// What the banner above the content pane says and offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BannerKind {
    /// "Download and install", for a version this install replaces itself with.
    Install,
    /// The release link, for an install that a package manager or the user owns.
    Notice,
    Downloading(Option<u8>),
    Restart,
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
    pub installer: Arc<dyn Installer>,
    /// Ends the app once the helper that swaps the new version in has started.
    pub quit: Rc<dyn Fn(&mut App)>,
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
    installer: Arc<dyn Installer>,
    quit: Rc<dyn Fn(&mut App)>,
    clock: Rc<dyn Fn() -> u64>,
    current: Version,
    state: State,
    /// Counts the checks, so the answer to an older one is ignored.
    runs: u64,
    installs: u32,
    /// The new version that is downloaded, checked, and staged, once the state is `Ready`.
    staged: Option<Staged>,
    /// "Not now" on the restart banner, for this session only.
    restart_dismissed: bool,
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
            installer: parts.installer,
            quit: parts.quit,
            clock: parts.clock,
            current: parts.current,
            state: State::Idle,
            runs: 0,
            installs: 0,
            staged: None,
            restart_dismissed: false,
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
        self.checks_on()
            && !matches!(
                self.state,
                State::Checking | State::Downloading(..) | State::Ready(_)
            )
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
        if self.state.key() != state.key() {
            hook::record_event("updates", &format!("state {}", state.key()));
            match &state {
                State::Error(error) => log::info!("updates: state error {}", error.code),
                other => log::info!("updates: state {}", other.key()),
            }
        }
        self.state = state;
        cx.notify();
    }

    /// The download has begun.
    fn downloading(&mut self, percent: Option<u8>, cx: &mut Context<Self>) {
        if let State::Downloading(manifest, _) = &self.state {
            let manifest = manifest.clone();
            self.set_state(State::Downloading(manifest, percent), cx);
        }
    }

    /// The new version is downloaded, checked, and staged. It starts after a restart.
    fn ready(&mut self, cx: &mut Context<Self>) {
        if let Some(manifest) = self.state.manifest().cloned() {
            self.restart_dismissed = false;
            self.set_state(State::Ready(manifest), cx);
        }
    }

    /// The download or the swap failed. The old version keeps running.
    fn failed(&mut self, error: CheckError, cx: &mut Context<Self>) {
        log::warn!("update failed: {} {}", error.code, error.detail);
        self.staged = None;
        self.set_state(State::Error(error), cx);
    }

    /// What an offered version leads to on this install.
    fn delivery_of(&self, manifest: &Manifest) -> Delivery {
        delivery(self.installer.kind(), current_target(), manifest)
    }

    /// What the offered version leads to, if one is offered.
    fn delivery_now(&self) -> Option<Delivery> {
        self.state
            .manifest()
            .map(|manifest| self.delivery_of(manifest))
    }

    /// "Download and install", or the automatic install. Only an offered version can start, and
    /// only where the app replaces itself: a package manager or the user owns the other
    /// installs, which show the release link instead.
    pub fn install(&mut self, cx: &mut Context<Self>) {
        let State::Available(manifest) = &self.state else {
            return;
        };
        let manifest = manifest.clone();
        let artifact = match self.delivery_of(&manifest) {
            Delivery::Notice => return,
            Delivery::Unpackaged => None,
            Delivery::Artifact(artifact) => Some(artifact),
        };
        self.installs += 1;
        hook::record_event(
            "updates",
            &format!("install requested {}", manifest.version.as_str()),
        );
        let Some(artifact) = artifact else {
            log::info!(
                "updates: this build runs from a source tree, so there is nothing to replace"
            );
            cx.notify();
            return;
        };
        self.set_state(State::Downloading(manifest, None), cx);
        let installer = Arc::clone(&self.installer);
        let (sender, mut receiver) = mpsc::unbounded::<Progress>();
        cx.spawn(async move |this, cx| {
            while let Some(progress) = receiver.next().await {
                if this
                    .update(cx, |me, cx| me.stage_progress(progress, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    installer.stage(&artifact, &mut |progress| {
                        let _ = sender.unbounded_send(progress);
                    })
                })
                .await;
            let _ = this.update(cx, |me, cx| me.finish_stage(result, cx));
        })
        .detach();
    }

    fn stage_progress(&mut self, progress: Progress, cx: &mut Context<Self>) {
        let percent = match progress {
            Progress::Bytes { done, total } if total > 0 => {
                u8::try_from((done.min(total) * 100 / total).min(100)).ok()
            }
            Progress::Bytes { .. } => None,
            Progress::Verifying => Some(100),
        };
        self.downloading(percent, cx);
    }

    fn finish_stage(&mut self, result: Result<Staged, CheckError>, cx: &mut Context<Self>) {
        if !matches!(self.state, State::Downloading(..)) {
            return;
        }
        match result {
            Ok(staged) => {
                self.staged = Some(staged);
                self.ready(cx);
            }
            Err(error) => self.failed(error, cx),
        }
    }

    /// "Restart to update": a helper swaps the staged version in after this app quits, and starts
    /// it. If the helper cannot start, the app keeps running with an error.
    pub fn restart_to_update(&mut self, cx: &mut Context<Self>) {
        let (State::Ready(_), Some(staged)) = (&self.state, self.staged.clone()) else {
            return;
        };
        match self.installer.restart(&staged) {
            Ok(()) => {
                hook::record_event("updates", "restart to update");
                log::info!("updates: restarting to update");
                let app: &mut App = cx;
                (self.quit)(app);
            }
            Err(error) => self.failed(error, cx),
        }
    }

    /// "Not now" on the banner: the banner stays away for this version.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        let Some(manifest) = self.state.manifest() else {
            return;
        };
        if matches!(self.state, State::Ready(_)) {
            self.restart_dismissed = true;
            cx.notify();
            return;
        }
        let version = manifest.version.as_str().to_owned();
        let stored = json!(version);
        if let Err(error) = self.storage.settings.set_internal(DISMISSED, stored) {
            log::warn!("the dismissed update could not be saved: {error}");
        }
        hook::record_event("updates", &format!("dismissed {version}"));
        cx.notify();
    }

    /// What the banner shows, if it is on screen. An offer waits for a person who installs by
    /// hand and has not sent it away; a notice shows whatever the switch says, because nothing
    /// installs by itself there; a finished download asks for the restart.
    pub(crate) fn banner_kind(&self) -> Option<BannerKind> {
        let dismissed = |manifest: &Manifest| {
            self.storage
                .settings
                .get(DISMISSED)
                .and_then(|value| value.as_str().map(str::to_owned))
                .as_deref()
                == Some(manifest.version.as_str())
        };
        match &self.state {
            State::Available(manifest) => match self.delivery_of(manifest) {
                Delivery::Notice => (!dismissed(manifest)).then_some(BannerKind::Notice),
                _ => (!self.auto_install() && !dismissed(manifest)).then_some(BannerKind::Install),
            },
            State::Downloading(_, percent) => {
                (!self.auto_install()).then_some(BannerKind::Downloading(*percent))
            }
            State::Ready(_) => (!self.restart_dismissed).then_some(BannerKind::Restart),
            _ => None,
        }
    }

    pub fn banner_visible(&self) -> bool {
        self.banner_kind().is_some()
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
            State::Error(error) if error.code == UPDATE_HASH_MISMATCH => {
                "The downloaded update did not match its checksum".to_owned()
            }
            State::Error(error) if error.code == UPDATE_DOWNLOAD_FAILED => {
                "The update could not be downloaded".to_owned()
            }
            State::Error(error) if error.code == UPDATE_SWAP_FAILED => {
                "The update could not be installed".to_owned()
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
            "install_kind": self.installer.kind().key(),
            "delivery": self.delivery_now().map(|delivery| match delivery {
                Delivery::Artifact(_) => "artifact",
                Delivery::Notice => "notice",
                Delivery::Unpackaged => "unpackaged",
            }),
            "staged": self.staged.is_some(),
            "last_open": self.last_open,
        })
    }
}

#[cfg(test)]
mod tests;
