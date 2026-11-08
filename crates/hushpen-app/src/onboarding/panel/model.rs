//! The model step: the speech models as the Models view shows them, and the line that says one
//! has to be downloaded.

use crate::hook;
use crate::models::panel::speech_rows;
use crate::onboarding::Onboarding;
use crate::theme::Palette;
use crate::ui::{self, Notice};
use gpui_kit::{AnyElement, App, Entity, IntoElement};

const HELP: &str =
    "Download at least one model to continue. The first one you download becomes the default.";

pub(super) fn view(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (models, notice) = {
        let me = onboarding.read(cx);
        (me.models().clone(), me.model_notice(cx))
    };
    let mut rows: Vec<AnyElement> = Vec::new();
    if let Some(message) = notice {
        rows.push(ui::notice_row(
            palette,
            hook::id("onboarding", "model.notice"),
            message,
            Notice::Alert,
            None,
        ));
    }
    rows.extend(speech_rows(palette, &models, cx));
    ui::section(
        palette,
        ui::group(hook::id("onboarding", "model"), palette, rows),
        Some((hook::id("onboarding", "model.help"), HELP.to_owned())),
    )
}
