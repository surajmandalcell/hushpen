//! AI cleanup in Settings: the model library link, the model status, and the provider and model choice.

use super::Settings;
use crate::models::{ModelState, Models};
use gpui_kit::{App, Context, Entity};
use hushpen_core::catalog::{Catalog, ModelEntry};
use hushpen_core::endpoint::Preset;
use serde_json::json;
use std::rc::Rc;

/// What Settings > Cleanup needs to offer, download, and report the cleanup model.
pub struct AiParts {
    pub models: Entity<Models>,
    pub catalog: &'static Catalog,
    /// Whether the model worker is loading the model right now.
    pub loading: Rc<dyn Fn() -> bool>,
}

/// What the cleanup model is doing, for the words under the AI model row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiStatus {
    NotDownloaded,
    Downloading(Option<u64>),
    Verifying,
    Failed,
    Preparing,
    Ready,
}

impl AiStatus {
    pub(crate) fn key(self) -> &'static str {
        match self {
            AiStatus::NotDownloaded => "not_downloaded",
            AiStatus::Downloading(_) => "downloading",
            AiStatus::Verifying => "verifying",
            AiStatus::Failed => "failed",
            AiStatus::Preparing => "preparing",
            AiStatus::Ready => "ready",
        }
    }
}

const AI_LOADING_POLL: std::time::Duration = std::time::Duration::from_millis(500);

impl Settings {
    /// Where AI cleanup runs: `local` or `endpoint`.
    pub(crate) fn ai_provider(&self) -> &'static str {
        match self.stored_str("cleanup.llm.provider").as_str() {
            "endpoint" => "endpoint",
            _ => "local",
        }
    }

    pub fn set_ai_provider(&mut self, provider: &str, cx: &mut Context<Self>) {
        if matches!(provider, "local" | "endpoint") {
            self.set("cleanup.llm.provider", json!(provider), cx);
            if provider == "local" && self.stored_bool("cleanup.llm.enabled") {
                self.download_ai_model(cx);
            }
        }
        self.picker = None;
        cx.notify();
    }

    pub fn set_endpoint_preset(&mut self, preset: Preset, cx: &mut Context<Self>) {
        if let Some(form) = self.endpoint.clone() {
            form.update(cx, |form, cx| form.set_preset(preset, cx));
        }
        self.picker = None;
        cx.notify();
    }

    /// Connects Cleanup to the model library and the cleanup worker. Without it the AI rows
    /// show nothing to choose.
    pub fn attach_ai(&mut self, ai: AiParts, cx: &mut Context<Self>) {
        cx.observe(&ai.models, |_, _, cx| cx.notify()).detach();
        self.ai = Some(ai);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(AI_LOADING_POLL).await;
                if this.update(cx, |me, cx| me.poll_ai_loading(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn poll_ai_loading(&mut self, cx: &mut Context<Self>) {
        let loading = self.ai.as_ref().is_some_and(|ai| (ai.loading)());
        if loading != self.ai_loading {
            self.ai_loading = loading;
            cx.notify();
        }
    }

    /// The cleanup models, best first.
    pub(crate) fn ai_offered(&self) -> Vec<ModelEntry> {
        self.ai.as_ref().map_or_else(Vec::new, |ai| {
            ai.catalog.offered_llm().into_iter().cloned().collect()
        })
    }

    /// The model AI cleanup uses: the stored choice while it is offered, otherwise the first
    /// one offered.
    pub(crate) fn ai_model(&self) -> Option<ModelEntry> {
        let ai = self.ai.as_ref()?;
        ai.catalog
            .chosen_llm(&self.stored_str("cleanup.llm.modelId"))
            .cloned()
    }

    pub(crate) fn ai_status(&self, cx: &App) -> Option<AiStatus> {
        let model = self.ai_model()?;
        let ai = self.ai.as_ref()?;
        let state = ai.models.read(cx).llm_state(&model.id)?;
        Some(match state {
            ModelState::NotDownloaded => AiStatus::NotDownloaded,
            ModelState::Downloading { .. } => AiStatus::Downloading(state.percent()),
            ModelState::Verifying => AiStatus::Verifying,
            ModelState::Failed => AiStatus::Failed,
            ModelState::Ready if self.ai_loading && self.stored_bool("cleanup.llm.enabled") => {
                AiStatus::Preparing
            }
            ModelState::Ready => AiStatus::Ready,
        })
    }

    /// Turns AI cleanup on or off. Turning it on picks the model, saves it, and starts its
    /// download when the file is not there yet; dictations before it is ready use the rules.
    pub fn toggle_ai_cleanup(&mut self, cx: &mut Context<Self>) {
        if self.stored_bool("cleanup.llm.enabled") {
            self.set("cleanup.llm.enabled", json!(false), cx);
            return;
        }
        if self.ai_provider() == "endpoint" {
            self.set("cleanup.llm.enabled", json!(true), cx);
            return;
        }
        let Some(model) = self.ai_model() else {
            self.notice = Some("No AI model is available for this computer.".into());
            cx.notify();
            return;
        };
        self.set("cleanup.llm.modelId", json!(model.id), cx);
        self.set("cleanup.llm.enabled", json!(true), cx);
        self.download_ai_model(cx);
    }

    pub fn set_ai_model(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.ai_offered().iter().any(|entry| entry.id == id) {
            self.set("cleanup.llm.modelId", json!(id), cx);
            if self.stored_bool("cleanup.llm.enabled") {
                self.download_ai_model(cx);
            }
        }
        self.picker = None;
        cx.notify();
    }

    /// Starts the download of the chosen cleanup model unless it is there or on its way.
    pub fn download_ai_model(&mut self, cx: &mut Context<Self>) {
        let (Some(model), Some(ai)) = (self.ai_model(), self.ai.as_ref()) else {
            return;
        };
        let models = ai.models.clone();
        let started = models.update(cx, |models, cx| match models.llm_state(&model.id) {
            Some(ModelState::NotDownloaded | ModelState::Failed) => models.download(&model.id, cx),
            _ => Ok(()),
        });
        if let Err(reason) = started {
            self.notice = Some(format!("The AI model could not be downloaded: {reason}"));
            cx.notify();
        }
    }
}
