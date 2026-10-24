//! The data folder move: the state of the move, the folder pick, the copy, and the restart.

use super::Settings;
use crate::hook;
use gpui_kit::{App, Context};
use hushpen_core::error::DATA_FOLDER_MOVE_FAILED;
use std::path::PathBuf;
use std::rc::Rc;

/// Where a data folder move stands. The failure carries the code the UI and
/// the hook show, never the payload of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveState {
    Idle,
    Choosing,
    Failed(&'static str),
    /// The move finished; the app restarts into the new folder.
    Restarting,
}

impl MoveState {
    pub fn key(&self) -> &'static str {
        match self {
            MoveState::Idle => "idle",
            MoveState::Choosing => "choosing",
            MoveState::Failed(_) => "failed",
            MoveState::Restarting => "restarting",
        }
    }
}

impl Settings {
    /// The data folder row: the pick starts here. A running dictation keeps
    /// the button off so the move never copies a half-written recording.
    pub fn change_data_folder(&mut self, cx: &mut Context<Self>) {
        if self.move_state != MoveState::Idle {
            return;
        }
        if self.mic.read(cx).state() == crate::mic::CaptureState::Listening {
            self.notice = Some("The microphone is recording. Stop the dictation first.".into());
            cx.notify();
            return;
        }
        if self.in_flight.as_ref().is_some_and(|busy| busy(cx)) {
            self.notice = Some(
                "A transcription is running. Wait for it to finish, then move the folder.".into(),
            );
            cx.notify();
            return;
        }
        self.move_state = MoveState::Choosing;
        self.notice = None;
        cx.notify();
        cx.spawn(async move |me, cx| {
            let picked = pick_folder(cx).await;
            let _ = me.update(cx, |me, cx| me.finish_move(picked, cx));
        })
        .detach();
    }

    pub(super) fn finish_move(&mut self, picked: Option<PathBuf>, cx: &mut Context<Self>) {
        let Some(target) = picked else {
            self.move_state = MoveState::Idle;
            cx.notify();
            return;
        };
        let storage = Rc::clone(&self.storage);
        let current = storage.data.root().to_path_buf();
        let default = self.default_data_dir.clone();
        // The WAL must fold into the database file before the copy reads it.
        if let Err(error) = storage.database.checkpoint() {
            self.move_failed(&format!("the database could not settle: {error}"), cx);
            return;
        }
        match hushpen_store::data_move::move_data(&current, &target, &default) {
            Ok(report) => {
                hook::record_event(
                    "settings",
                    &format!(
                        "data-folder moved to {} ({} files, {} bytes)",
                        target.display(),
                        report.files,
                        report.bytes
                    ),
                );
                self.move_state = MoveState::Restarting;
                cx.notify();
                let restart = Rc::clone(&self.restart);
                restart(cx);
            }
            Err(error) => {
                log::warn!("the data folder move failed: {error}");
                hook::record_event(
                    "settings",
                    &format!("data-folder failed {DATA_FOLDER_MOVE_FAILED}"),
                );
                self.move_failed(&error.to_string(), cx);
            }
        }
    }

    fn move_failed(&mut self, reason: &str, cx: &mut Context<Self>) {
        self.move_state = MoveState::Failed(DATA_FOLDER_MOVE_FAILED);
        self.notice = Some(format!(
            "The data folder could not be moved ({DATA_FOLDER_MOVE_FAILED}). {reason}"
        ));
        cx.notify();
    }

    pub fn move_state(&self) -> &MoveState {
        &self.move_state
    }
}

/// The folder for the move: the path the test hook queued, or the system's
/// folder picker.
async fn pick_folder(cx: &mut gpui_kit::AsyncApp) -> Option<PathBuf> {
    #[cfg(feature = "test-automation")]
    if let Some(paths) = hushpen_testhook::dialogs::take_answer() {
        return paths.into_iter().next();
    }
    let asked = cx.update(|cx| {
        cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        })
    });
    match asked.await {
        Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.pop(),
        _ => None,
    }
}

/// Hands the process to a fresh copy of itself, so the store, the database,
/// and the engine reopen on the new data folder. The helper waits for this
/// process to end before the new one binds the single-instance socket.
pub(super) fn restart_into(cx: &mut App) {
    if let Ok(exe) = std::env::current_exe() {
        let pid = std::process::id();
        let _ = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "while kill -0 {pid} 2>/dev/null; do sleep 0.1; done; exec \"$0\"",
            ))
            .arg(exe)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    cx.quit();
}
