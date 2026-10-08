//! Home dictation: the record button, the transcript, and the language picker.
//!
//! Home owns no pipeline. The record button sends [`AppEvent::HomeToggle`] to the
//! [`Controller`], the same pipeline that the hold key, the tray, and the flow bar feed, and
//! Home shows what the controller reports.

pub mod panel;

use crate::controller::{Blocker, Controller, Notice, Phase};
use crate::hook;
use crate::models::Models;
use crate::storage::Storage;
use gpui_kit::{ClipboardItem, Context, Entity, FocusHandle};
use hushpen_core::catalog::Languages;
use hushpen_core::dictation::AppEvent;
use hushpen_core::language;
use serde_json::{Value, json};
use std::path::Path;
use std::rc::Rc;

const RECENT_SETTING: &str = "dictation.recentLanguages";

pub struct Dictation {
    storage: Rc<Storage>,
    models: Entity<Models>,
    controller: Entity<Controller>,
    picker_open: bool,
    pub(crate) record_focus: FocusHandle,
    pub(crate) copy_focus: FocusHandle,
    pub(crate) models_focus: FocusHandle,
    pub(crate) language_focus: FocusHandle,
    pub(crate) option_focus: Vec<FocusHandle>,
}

impl Dictation {
    pub fn new(
        storage: Rc<Storage>,
        models: Entity<Models>,
        controller: Entity<Controller>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&controller, |_, _, cx| cx.notify()).detach();
        cx.observe(&models, |_, _, cx| cx.notify()).detach();
        // One handle for Auto and one for each language.
        let option_focus = (0..=language::LANGUAGES.len())
            .map(|_| cx.focus_handle().tab_stop(true))
            .collect();
        Self {
            storage,
            models,
            controller,
            picker_open: false,
            record_focus: cx.focus_handle().tab_stop(true),
            copy_focus: cx.focus_handle().tab_stop(true),
            models_focus: cx.focus_handle().tab_stop(true),
            language_focus: cx.focus_handle().tab_stop(true),
            option_focus,
        }
    }

    pub fn controller(&self) -> &Entity<Controller> {
        &self.controller
    }

    pub fn phase(&self, cx: &gpui_kit::App) -> Phase {
        self.controller.read(cx).phase()
    }

    pub fn transcript<'a>(&self, cx: &'a gpui_kit::App) -> &'a str {
        self.controller.read(cx).transcript()
    }

    pub fn detected<'a>(&self, cx: &'a gpui_kit::App) -> Option<&'a str> {
        self.controller.read(cx).detected()
    }

    pub fn notice<'a>(&self, cx: &'a gpui_kit::App) -> Option<&'a Notice> {
        self.controller.read(cx).notice()
    }

    pub fn keys_notice<'a>(&self, cx: &'a gpui_kit::App) -> Option<&'a str> {
        self.controller.read(cx).keys_notice()
    }

    pub fn picker_open(&self) -> bool {
        self.picker_open
    }

    pub fn session<'a>(&self, cx: &'a gpui_kit::App) -> Option<&'a Path> {
        self.controller.read(cx).session()
    }

    /// The `dictation.language` setting: `auto` or a whisper code.
    pub fn language(&self, cx: &gpui_kit::App) -> String {
        self.controller.read(cx).language()
    }

    fn recent(&self) -> Vec<String> {
        self.storage
            .settings
            .get(RECENT_SETTING)
            .and_then(|value| {
                value.as_array().map(|list| {
                    list.iter()
                        .filter_map(|code| code.as_str().map(str::to_owned))
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// `auto`, then the recent languages, then English and the rest by name.
    pub fn picker_codes(&self) -> Vec<&'static str> {
        let mut codes = vec![language::AUTO];
        codes.extend(language::picker_order(&self.recent()));
        codes
    }

    /// Why the picker is off, or `None` while it works.
    pub fn picker_off_reason(&self, cx: &gpui_kit::App) -> Option<&'static str> {
        let models = self.models.read(cx);
        let id = models.effective();
        let english_only = models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .is_some_and(|row| row.entry.languages == Languages::English);
        english_only.then_some("This model understands English only.")
    }

    /// Why Home cannot start a recording, or `None` while it can.
    pub fn blocker(&self, cx: &gpui_kit::App) -> Option<Blocker> {
        self.controller.read(cx).blocker(cx)
    }

    pub fn can_record(&self, cx: &gpui_kit::App) -> bool {
        self.controller.read(cx).can_record(cx)
    }

    /// What the record button does: stop while listening, otherwise start.
    pub fn toggle(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.phase(cx) == Phase::Transcribing {
            return Err("a transcription is still running".into());
        }
        self.controller.update(cx, |controller, cx| {
            controller.dispatch(AppEvent::HomeToggle, cx)
        })
    }

    pub fn toggle_picker(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(reason) = self.picker_off_reason(cx) {
            self.picker_open = false;
            return Err(reason.to_owned());
        }
        self.picker_open = !self.picker_open;
        cx.notify();
        Ok(())
    }

    pub fn close_picker(&mut self, cx: &mut Context<Self>) {
        if self.picker_open {
            self.picker_open = false;
            cx.notify();
        }
    }

    /// Saves the language of the next dictation: `auto` or a whisper code.
    pub fn set_language(&mut self, code: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if !language::is_setting_value(code) {
            return Err(format!("'{code}' is not a language whisper knows"));
        }
        if let Some(reason) = self.picker_off_reason(cx) {
            return Err(reason.to_owned());
        }
        self.storage
            .settings
            .set(crate::controller::LANGUAGE_SETTING, json!(code))
            .map_err(|error| error.to_string())?;
        let recent = language::push_recent(&self.recent(), code);
        self.storage
            .settings
            .set(RECENT_SETTING, json!(recent))
            .map_err(|error| error.to_string())?;
        self.picker_open = false;
        hook::record_event("dictation", &format!("language {code}"));
        cx.notify();
        Ok(())
    }

    /// Copies the shown transcript again.
    pub fn copy(&self, cx: &mut Context<Self>) -> Result<(), String> {
        let text = self.transcript(cx).to_owned();
        if text.is_empty() {
            return Err("there is no transcript to copy".into());
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        hook::record_event("dictation", "copied");
        Ok(())
    }

    pub fn state_json(&self, cx: &gpui_kit::App) -> Value {
        let models = self.models.read(cx);
        let blocker = self.blocker(cx);
        let off = self.picker_off_reason(cx);
        let language = self.language(cx);
        let detected = self.detected(cx);
        json!({
            "state": self.phase(cx).key(),
            "transcript": self.transcript(cx),
            "language": detected,
            "language_name": detected.and_then(language::name),
            "notice": self.notice(cx).map(|notice| json!({
                "code": notice.code,
                "message": notice.message,
            })),
            "last_result": self.controller.read(cx).last_result(),
            "blocker": blocker.as_ref().map(|blocker| json!({
                "code": blocker.code,
                "message": blocker.message,
                "models_link": blocker.models_link,
            })),
            "can_record": self.can_record(cx),
            "model": models.effective(),
            "picker": {
                "selected": language,
                "label": language::label(&language),
                "open": self.picker_open,
                "enabled": off.is_none(),
                "reason": off,
                "options": self.picker_codes().len(),
            },
            "session": self.session(cx).map(|path| path.to_string_lossy().into_owned()),
        })
    }
}

#[cfg(test)]
mod tests;
