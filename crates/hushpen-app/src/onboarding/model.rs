//! The model step: the speech models the user downloads, and the default among them.

use super::Onboarding;
use crate::models::{ModelState, Models};
use gpui_kit::{App, Context, Entity};
use hushpen_core::catalog::ModelKind;

impl Onboarding {
    /// The library the rows of the step act on.
    pub fn models(&self) -> &Entity<Models> {
        &self.models
    }

    /// The id of the default speech model: the chosen one, else the catalog default.
    pub fn model_id(&self, cx: &App) -> String {
        self.models.read(cx).effective()
    }

    /// The state of the default speech model.
    pub fn model_state(&self, cx: &App) -> Option<ModelState> {
        let models = self.models.read(cx);
        let id = models.effective();
        models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .map(|row| row.state)
    }

    /// The ids of the speech models that are downloaded and checked.
    pub fn downloaded_models(&self, cx: &App) -> Vec<String> {
        self.models
            .read(cx)
            .rows()
            .iter()
            .filter(|row| row.entry.kind == ModelKind::Speech && row.state == ModelState::Ready)
            .map(|row| row.entry.id.clone())
            .collect()
    }

    /// The failure the Models service last reported, such as a bad checksum.
    pub fn model_notice(&self, cx: &App) -> Option<String> {
        self.models
            .read(cx)
            .notice()
            .map(|notice| notice.message.clone())
    }

    /// The step passes once one speech model is downloaded.
    pub fn model_ready(&self, cx: &App) -> bool {
        !self.downloaded_models(cx).is_empty()
    }

    /// The default becomes the chosen model, so the Models view shows it as the default. When
    /// the default is not downloaded, the first downloaded speech model becomes the default.
    pub(super) fn choose_default_model(&mut self, cx: &mut Context<Self>) {
        let id = match self.models.read(cx).usable() {
            Some(row) => row.entry.id.clone(),
            None => match self.downloaded_models(cx).into_iter().next() {
                Some(id) => id,
                None => return,
            },
        };
        self.models.update(cx, |models, cx| {
            if models.active() != id {
                let _ = models.select(&id, cx);
            }
        });
    }
}
