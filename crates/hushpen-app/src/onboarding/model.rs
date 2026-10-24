//! The model step: the one speech model the step downloads, and its state.

use super::Onboarding;
use crate::models::ModelState;
use gpui_kit::{App, Context};

impl Onboarding {
    /// The id of the model the step downloads: the chosen one, else the catalog default.
    pub fn model_id(&self, cx: &App) -> String {
        self.models.read(cx).effective()
    }

    pub fn model_state(&self, cx: &App) -> Option<ModelState> {
        let models = self.models.read(cx);
        let id = models.effective();
        models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .map(|row| row.state)
    }

    /// The name and size of the model the step downloads.
    pub fn model_info(&self, cx: &App) -> Option<(String, u64)> {
        let models = self.models.read(cx);
        let id = models.effective();
        models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .map(|row| (row.entry.name.clone(), row.entry.bytes))
    }

    /// The failure the Models service last reported, such as a bad checksum.
    pub fn model_notice(&self, cx: &App) -> Option<String> {
        self.models
            .read(cx)
            .notice()
            .map(|notice| notice.message.clone())
    }

    pub fn model_ready(&self, cx: &App) -> bool {
        self.models.read(cx).usable().is_some()
    }

    pub fn download_model(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let id = self.model_id(cx);
        self.models
            .update(cx, |models, cx| models.download(&id, cx))
    }

    pub fn cancel_model(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let id = self.model_id(cx);
        self.models.update(cx, |models, cx| models.cancel(&id, cx))
    }

    /// The downloaded model becomes the chosen one, so the Models view shows it as active.
    pub(super) fn choose_default_model(&mut self, cx: &mut Context<Self>) {
        let id = self.model_id(cx);
        self.models.update(cx, |models, cx| {
            if models.active().is_empty() {
                let _ = models.select(&id, cx);
            }
        });
    }
}
