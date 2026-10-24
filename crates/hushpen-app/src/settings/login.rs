//! Launch at login: the login item trait, the cached system answer, and the threads that read and write it.

use super::Settings;
use crate::hook;
use gpui_kit::Context;
use serde_json::json;
use std::sync::Arc;

/// Launch at login, behind a trait so tests never touch the login items of
/// the machine. The platform impl is [`hushpen_platform::login_item`].
///
/// `is_enabled` can take over a second on macOS (an XPC call to the background task
/// manager), so only a reading thread calls it. The views read [`Settings::launch_at_login`],
/// which answers from a cache.
pub trait LoginToggle: Send + Sync {
    fn is_enabled(&self) -> bool;
    fn set(&self, enabled: bool) -> Result<(), String>;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl LoginToggle for hushpen_platform::login_item::LoginItem {
    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }

    fn set(&self, enabled: bool) -> Result<(), String> {
        self.set(enabled).map_err(|error| error.to_string())
    }
}

/// How often the view looks for the answer of a running status read.
const LOGIN_POLL: std::time::Duration = std::time::Duration::from_millis(20);

/// What the system last said about launch at login. A read starts on a thread and its answer
/// comes back as a `LoginRead`.
#[derive(Default)]
pub(super) struct LoginCache {
    /// `None` until the first read lands.
    value: Option<bool>,
    /// Counts the toggles, so a read that started before one never overwrites its result.
    generation: u64,
    pub(super) reading: bool,
    /// A refresh was asked for while a read ran; one more read follows it.
    again: bool,
    pub(super) writing: bool,
    /// The newest click while a write ran; it is written after that write.
    queued: Option<bool>,
}

/// The answer of one reading thread, and the generation it started in.
struct LoginRead {
    generation: u64,
    enabled: bool,
}

impl Settings {
    /// Launch at login as the system last said it, or the stored intent until the first answer
    /// and on a platform with no login items. It never asks the system.
    pub fn launch_at_login(&self) -> bool {
        self.login_cache
            .value
            .unwrap_or_else(|| self.stored_bool("startup.launchAtLogin"))
    }

    /// Starts a read of the system's answer on a thread. A read already running gets one more
    /// after it, so the newest request is never lost. During a write the system still has the
    /// old value, so the read waits for the write, which refreshes when it lands.
    pub(super) fn refresh_login(&mut self, cx: &mut Context<Self>) {
        let Some(login) = &self.login else { return };
        if self.login_cache.writing {
            return;
        }
        if self.login_cache.reading {
            self.login_cache.again = true;
            return;
        }
        let login = Arc::clone(login);
        let (answer, answered) = std::sync::mpsc::channel();
        let generation = self.login_cache.generation;
        let spawned = std::thread::Builder::new()
            .name("hushpen-login-status".into())
            .spawn(move || {
                let enabled = login.is_enabled();
                let _ = answer.send(LoginRead {
                    generation,
                    enabled,
                });
            });
        if spawned.is_err() {
            return;
        }
        self.login_cache.reading = true;
        Self::await_answer(answered, cx, |me, read, cx| match read {
            Some(read) => me.login_read(read, cx),
            None => me.login_cache.reading = false,
        });
    }

    /// Hands the one answer of a worker thread to `land`, or `None` when the thread ended
    /// without one. The thread cannot wake this task, so the task looks for the answer on a
    /// short timer. A task woken from another thread would also break the test scheduler.
    fn await_answer<T: Send + 'static>(
        answered: std::sync::mpsc::Receiver<T>,
        cx: &mut Context<Self>,
        land: impl FnOnce(&mut Self, Option<T>, &mut Context<Self>) + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            let answer = loop {
                match answered.try_recv() {
                    Ok(answer) => break Some(answer),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor().timer(LOGIN_POLL).await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break None,
                }
            };
            let _ = this.update(cx, |me, cx| land(me, answer, cx));
        })
        .detach();
    }

    fn login_read(&mut self, read: LoginRead, cx: &mut Context<Self>) {
        self.login_cache.reading = false;
        if read.generation == self.login_cache.generation {
            self.login_cache.value = Some(read.enabled);
            // A login item removed by hand in System Settings shows as off, and stays off.
            if read.enabled != self.stored_bool("startup.launchAtLogin") {
                let _ = self
                    .storage
                    .settings
                    .set("startup.launchAtLogin", json!(read.enabled));
            }
            cx.notify();
        }
        if std::mem::take(&mut self.login_cache.again) {
            self.refresh_login(cx);
        }
    }

    /// The row follows the click at once, and the login item is written on a thread. A refused
    /// write puts the row back and says why; the system's own answer replaces it after that.
    pub fn toggle_launch_at_login(&mut self, cx: &mut Context<Self>) {
        let on = !self.launch_at_login();
        self.set("startup.launchAtLogin", json!(on), cx);
        if self.login.is_none() {
            hook::record_event("settings", &format!("launch-at-login {on}"));
            return;
        }
        self.login_cache.generation += 1;
        self.login_cache.value = Some(on);
        self.write_login(on, cx);
    }

    /// Writes launch at login on a thread: registering a login item is an XPC call that can
    /// take over a second. One write runs at a time, and a click during it is written next.
    fn write_login(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some(login) = &self.login else { return };
        if self.login_cache.writing {
            self.login_cache.queued = Some(on);
            return;
        }
        let login = Arc::clone(login);
        let (answer, answered) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("hushpen-login-write".into())
            .spawn(move || {
                let _ = answer.send(login.set(on));
            });
        if let Err(error) = spawned {
            self.login_written(on, Err(error.to_string()), cx);
            return;
        }
        self.login_cache.writing = true;
        Self::await_answer(answered, cx, move |me, result, cx| {
            me.login_cache.writing = false;
            let result = result.unwrap_or_else(|| Err("the login item did not answer".into()));
            me.login_written(on, result, cx);
        });
    }

    fn login_written(&mut self, on: bool, result: Result<(), String>, cx: &mut Context<Self>) {
        let queued = self.login_cache.queued.take();
        let written = result.is_ok();
        match result {
            Ok(()) => hook::record_event("settings", &format!("launch-at-login {on}")),
            Err(error) => {
                if queued.is_none() {
                    self.login_cache.generation += 1;
                    self.login_cache.value = Some(!on);
                    let _ = self
                        .storage
                        .settings
                        .set("startup.launchAtLogin", json!(!on));
                }
                self.notice = Some(format!("Launch at login could not be changed: {error}"));
                cx.notify();
            }
        }
        match queued {
            Some(next) if next != on || !written => self.write_login(next, cx),
            _ => self.refresh_login(cx),
        }
    }
}
