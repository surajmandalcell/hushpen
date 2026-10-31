//! The settings registry rows: the toggles and setters that write one value, and the stored reads.

use super::Settings;
use crate::enhance::Mode;
use crate::hook;
use gpui_kit::{Context, FocusHandle, Window};
use serde_json::{Value, json};

impl Settings {
    pub fn toggle_start_hidden(&mut self, cx: &mut Context<Self>) {
        self.flip("startup.startHidden", cx);
    }

    pub fn toggle_sounds(&mut self, cx: &mut Context<Self>) {
        self.flip("audio.cueSounds", cx);
    }

    pub fn toggle_output_mute(&mut self, cx: &mut Context<Self>) {
        self.flip("audio.muteWhileDictating", cx);
    }

    pub fn set_idle_policy(&mut self, value: &str, cx: &mut Context<Self>) {
        self.set("engine.idlePolicy", json!(value), cx);
        self.picker = None;
        cx.notify();
    }

    pub fn set_idle_minutes(&mut self, value: i64, cx: &mut Context<Self>) {
        self.set("engine.idleMinutes", json!(value), cx);
        self.picker = None;
        cx.notify();
    }

    /// Sets the cue volume, from 0 to 1, in steps of 5%, so a drag writes the file only when
    /// the step changes.
    pub fn set_cue_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        let stepped = (f64::from(volume.clamp(0.0, 1.0)) * 20.0).round() / 20.0;
        let stored = self.stored_f64("audio.cueVolume");
        if stored.is_none_or(|stored| (stored - stepped).abs() > f64::EPSILON) {
            self.set("audio.cueVolume", json!(stepped), cx);
        }
    }

    /// Sets the microphone boost, from 100% to 200% in steps of 5%, and gives it to a session
    /// that runs, so the level meter and the dictation follow the slider at once.
    pub fn set_input_gain(&mut self, gain: f32, cx: &mut Context<Self>) {
        let stepped = (f64::from(gain.clamp(1.0, 2.0)) * 20.0).round() / 20.0;
        let stored = self.stored_f64(crate::mic::GAIN_SETTING);
        if stored.is_none_or(|stored| (stored - stepped).abs() > f64::EPSILON) {
            self.set(crate::mic::GAIN_SETTING, json!(stepped), cx);
            self.mic.read(cx).apply_gain();
        }
    }

    /// Saves the audio enhance mode and gives it to a running microphone. Picking Custom opens
    /// the tuning sheet, as the Adjust button does.
    pub fn set_enhance_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        let before = crate::enhance::mode(&self.storage);
        self.set(crate::enhance::MODE, json!(mode.key()), cx);
        self.mic.read(cx).apply_enhance();
        if mode == Mode::Custom && before != Mode::Custom {
            let back = self.control_focus[super::slots::ENHANCE].clone();
            self.open_tuning(back, window, cx);
        }
    }

    /// Opens the audio enhance sheet on its tuning page; `back` has the focus once it closes.
    pub fn open_tuning(&mut self, back: FocusHandle, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tuning) = self.tuning.clone() {
            tuning.update(cx, |tuning, cx| {
                tuning.open(crate::enhance::Page::Tune, Some(back), window, cx)
            });
        }
    }

    pub fn set_max_minutes(&mut self, minutes: i64, cx: &mut Context<Self>) {
        self.set("dictation.maxMinutes", json!(minutes), cx);
        self.picker = None;
        cx.notify();
    }

    pub fn toggle_rules(&mut self, cx: &mut Context<Self>) {
        self.flip("cleanup.rules", cx);
    }

    pub fn toggle_spoken_punctuation(&mut self, cx: &mut Context<Self>) {
        self.flip("cleanup.spokenPunctuation", cx);
    }

    pub fn toggle_gpu(&mut self, cx: &mut Context<Self>) {
        self.flip("engine.useGpu", cx);
        self.engine_changed(cx);
    }

    /// `None` is Automatic: the engine picks the count from the CPUs.
    pub fn set_threads(&mut self, threads: Option<i64>, cx: &mut Context<Self>) {
        let value = threads.map_or_else(|| json!("auto"), |count| json!(count));
        self.set("engine.threads", value, cx);
        self.engine_changed(cx);
        self.picker = None;
        cx.notify();
    }

    /// The speech engine holds the GPU choice and the thread count of its last load; a changed
    /// one loads the model again.
    fn engine_changed(&self, cx: &mut Context<Self>) {
        if let Some(ai) = &self.ai {
            ai.models.read(cx).load_active();
        }
    }

    pub fn toggle_flow_bar(&mut self, cx: &mut Context<Self>) {
        self.flip("overlay.enabled", cx);
    }

    pub fn toggle_flow_bar_idle(&mut self, cx: &mut Context<Self>) {
        self.flip("overlay.idleVisible", cx);
    }

    pub(super) fn flip(&mut self, key: &str, cx: &mut Context<Self>) {
        let on = !self.stored_bool(key);
        self.set(key, json!(on), cx);
    }

    /// The Flow bar position goes through the store, which clears a dragged
    /// custom position in the same write.
    pub fn set_position(&mut self, position: &str, cx: &mut Context<Self>) {
        self.set("overlay.position", json!(position), cx);
        self.picker = None;
        cx.notify();
    }

    pub fn set_retention(&mut self, retention: &str, cx: &mut Context<Self>) {
        self.set("history.audioRetention", json!(retention), cx);
        self.picker = None;
        cx.notify();
    }

    pub fn stored_bool(&self, key: &str) -> bool {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    pub(crate) fn stored_f64(&self, key: &str) -> Option<f64> {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_f64())
    }

    pub(crate) fn stored_str(&self, key: &str) -> String {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    pub(super) fn set(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        match self.storage.settings.set(key, value) {
            Ok(()) => {
                self.notice = None;
                hook::record_event("settings", &format!("set {key}"));
            }
            Err(error) => {
                self.notice = Some(format!("The setting could not be saved: {error}"));
            }
        }
        cx.notify();
    }
}
