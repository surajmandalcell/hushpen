//! Settings > Cleanup, endpoint rows: the three text fields, the API key in the secrets file,
//! and the Test connection button.

use crate::hook;
use crate::net::endpoint::{Config, Failure, Transport};
use crate::storage::Storage;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Context, Entity, Window};
use hushpen_core::endpoint::{Preset, key_is_valid};
use hushpen_core::error::ENDPOINT_FAILED;
use serde_json::{Value, json};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const URL_SETTING: &str = "cleanup.llm.endpointUrl";
const MODEL_SETTING: &str = "cleanup.llm.endpointModel";
const PRESET_SETTING: &str = "cleanup.llm.endpointPreset";
const TIMEOUT_SETTING: &str = "cleanup.llm.endpointTimeoutMs";
const DEFAULT_TIMEOUT_MS: u64 = 6_000;

/// Where the Test connection button stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Test {
    Idle,
    Running,
    Connected,
    /// The words come from [`Failure::reason`], which never holds the key or the URL.
    Failed(String),
}

impl Test {
    pub fn key(&self) -> &'static str {
        match self {
            Test::Idle => "idle",
            Test::Running => "running",
            Test::Connected => "connected",
            Test::Failed(_) => "failed",
        }
    }
}

pub struct EndpointForm {
    storage: Rc<Storage>,
    transport: Arc<dyn Transport>,
    window: AnyWindowHandle,
    pub(crate) url: Entity<InputState>,
    pub(crate) model: Entity<InputState>,
    pub(crate) key: Entity<InputState>,
    pub(crate) test: Test,
    /// Counts the tests, so the answer to an older one is ignored.
    runs: u64,
    pub(crate) notice: Option<String>,
}

impl EndpointForm {
    pub fn new(
        storage: Rc<Storage>,
        transport: Arc<dyn Transport>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let stored = |key: &str| {
            storage
                .settings
                .get(key)
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let (url_text, model_text) = (stored(URL_SETTING), stored(MODEL_SETTING));
        let url = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Address ending in /v1");
            state.set_value(url_text, window, cx);
            state
        });
        let model = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Model name");
            state.set_value(model_text, window, cx);
            state
        });
        let key = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("API key")
                .masked(true)
        });
        cx.subscribe_in(&url, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().to_string();
                this.url_typed(&value, cx);
            }
        })
        .detach();
        cx.subscribe_in(&model, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().to_string();
                this.save_text(MODEL_SETTING, &value, cx);
            }
        })
        .detach();
        cx.subscribe_in(&key, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                this.save_key(cx);
            }
        })
        .detach();
        Self {
            storage,
            transport,
            window: window.window_handle(),
            url,
            model,
            key,
            test: Test::Idle,
            runs: 0,
            notice: None,
        }
    }

    pub fn preset(&self) -> Preset {
        let stored = self.storage.settings.get(PRESET_SETTING);
        Preset::parse(stored.as_ref().and_then(Value::as_str).unwrap_or_default())
    }

    pub fn key_is_set(&self) -> bool {
        self.storage.secrets.endpoint_api_key_set()
    }

    /// Fills the URL for a preset. `Custom` keeps what is typed.
    pub fn set_preset(&mut self, preset: Preset, cx: &mut Context<Self>) {
        self.save_text(PRESET_SETTING, preset.key(), cx);
        if let Some(url) = preset.url() {
            self.save_text(URL_SETTING, url, cx);
            self.show_in(&self.url.clone(), url.to_owned(), cx);
        }
        self.test = Test::Idle;
        cx.notify();
    }

    fn url_typed(&mut self, value: &str, cx: &mut Context<Self>) {
        let value = value.trim();
        self.save_text(URL_SETTING, value, cx);
        if let Some(url) = self.preset().url()
            && url != value
        {
            self.save_text(PRESET_SETTING, Preset::Custom.key(), cx);
        }
        self.test = Test::Idle;
        cx.notify();
    }

    fn save_text(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        if self
            .storage
            .settings
            .get(key)
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
            == Some(value)
        {
            return;
        }
        match self.storage.settings.set(key, json!(value)) {
            Ok(()) => {
                self.notice = None;
                hook::record_event("settings", &format!("set {key}"));
            }
            Err(error) => self.notice = Some(format!("The setting could not be saved: {error}")),
        }
        cx.notify();
    }

    /// Puts text into a field from outside an update of the window.
    fn show_in(&self, input: &Entity<InputState>, text: String, cx: &mut Context<Self>) {
        let (window, input) = (self.window, input.clone());
        cx.defer(move |cx: &mut App| {
            let _ = window.update(cx, |_, window, cx| {
                input.update(cx, |state, cx| state.set_value(text, window, cx));
            });
        });
    }

    /// Writes the typed key to the secrets file and empties the field, so the key is never
    /// shown again.
    pub fn save_key(&mut self, cx: &mut Context<Self>) {
        let typed = self.key.read(cx).value().to_string();
        let typed = typed.trim();
        if typed.is_empty() {
            return;
        }
        if !key_is_valid(typed) {
            self.notice = Some("The API key must be one line without spaces.".into());
            cx.notify();
            return;
        }
        match self.storage.secrets.set_endpoint_api_key(Some(typed)) {
            Ok(()) => {
                self.notice = None;
                hook::record_event("settings", "endpoint key saved");
                self.show_in(&self.key.clone(), String::new(), cx);
            }
            Err(error) => {
                log::warn!("the endpoint key could not be saved: {error}");
                self.notice = Some("The API key could not be saved.".into());
            }
        }
        self.test = Test::Idle;
        cx.notify();
    }

    pub fn clear_key(&mut self, cx: &mut Context<Self>) {
        match self.storage.secrets.set_endpoint_api_key(None) {
            Ok(()) => {
                self.notice = None;
                hook::record_event("settings", "endpoint key cleared");
            }
            Err(error) => {
                log::warn!("the endpoint key could not be removed: {error}");
                self.notice = Some("The API key could not be removed.".into());
            }
        }
        self.show_in(&self.key.clone(), String::new(), cx);
        self.test = Test::Idle;
        cx.notify();
    }

    fn config(&self) -> Config {
        let stored = |key: &str| {
            self.storage
                .settings
                .get(key)
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        let ms = self
            .storage
            .settings
            .get(TIMEOUT_SETTING)
            .and_then(|value| value.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        Config {
            url: stored(URL_SETTING),
            model: stored(MODEL_SETTING),
            key: self.storage.secrets.endpoint_api_key(),
            timeout: Duration::from_millis(ms),
        }
    }

    /// Asks the endpoint for its model list. The request runs off the main thread.
    pub fn test_connection(&mut self, cx: &mut Context<Self>) {
        if self.test == Test::Running {
            return;
        }
        self.save_key(cx);
        self.runs += 1;
        let run = self.runs;
        self.test = Test::Running;
        hook::record_event("settings", "endpoint test started");
        cx.notify();
        let (transport, config) = (Arc::clone(&self.transport), self.config());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { transport.test_connection(&config) })
                .await;
            let _ = this.update(cx, |me, cx| me.finish_test(run, result, cx));
        })
        .detach();
    }

    fn finish_test(&mut self, run: u64, result: Result<(), Failure>, cx: &mut Context<Self>) {
        if run != self.runs {
            return;
        }
        self.test = match result {
            Ok(()) => {
                hook::record_event("settings", "endpoint test connected");
                Test::Connected
            }
            Err(failure) => {
                hook::record_event(
                    "settings",
                    &format!("endpoint test failed {ENDPOINT_FAILED}"),
                );
                Test::Failed(failure.reason())
            }
        };
        cx.notify();
    }

    /// The words next to the Test connection button.
    pub fn test_text(&self) -> String {
        match &self.test {
            Test::Idle => "Checks that the endpoint answers.".to_owned(),
            Test::Running => "Testing the connection".to_owned(),
            Test::Connected => "Connected".to_owned(),
            Test::Failed(reason) => format!("{ENDPOINT_FAILED}: {reason}"),
        }
    }

    /// `hookctl state` `settings_view.endpoint`. The key is never in it, only whether one is
    /// saved.
    pub fn hook_state_json(&self, cx: &App) -> Value {
        let url = self.url.read(cx).value().to_string();
        json!({
            "preset": self.preset().key(),
            "url": if url.contains('@') { "(hidden)".to_owned() } else { url },
            "model": self.model.read(cx).value().to_string(),
            "api_key_set": self.key_is_set(),
            "test": match &self.test {
                Test::Failed(reason) => json!({"state": "failed", "code": ENDPOINT_FAILED, "reason": reason}),
                other => json!({"state": other.key()}),
            },
        })
    }
}
