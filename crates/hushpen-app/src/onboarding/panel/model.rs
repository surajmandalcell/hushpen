//! The model step: the download of the speech model and its progress.

use crate::models::ModelState;
use crate::models::panel::format_size;
use crate::onboarding::Onboarding;

use crate::hook;
use crate::theme::{Palette, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Rgba, Role,
    StatefulInteractiveElement, Styled, Window, div, px,
};

pub(super) fn view(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (info, state, focus, notice) = {
        let me = onboarding.read(cx);
        (
            me.model_info(cx),
            me.model_state(cx),
            me.download_focus.clone(),
            me.model_notice(cx),
        )
    };
    let (name, bytes) = info.unwrap_or_else(|| ("Speech model".to_owned(), 0));
    let state = state.unwrap_or(ModelState::NotDownloaded);
    let (status, tone): (String, Rgba) = match state {
        ModelState::NotDownloaded => ("Not downloaded".into(), palette.label_secondary),
        ModelState::Downloading { .. } => (
            state
                .percent()
                .map_or_else(|| "Starting".into(), |percent| format!("{percent}%")),
            palette.label,
        ),
        ModelState::Verifying => ("Checking".into(), palette.label_secondary),
        ModelState::Ready => ("Ready".into(), palette.success),
        ModelState::Failed => ("Failed verification".into(), palette.error),
    };
    let fraction = state
        .percent()
        .map_or(0.0, |percent| percent as f32 / 100.0);
    let downloading = matches!(state, ModelState::Downloading { .. });
    let press = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                // A refused action shows its reason in the notice row.
                let _ = if downloading {
                    me.cancel_model(cx)
                } else {
                    me.download_model(cx)
                };
            });
        }
    };
    let action = match state {
        ModelState::Ready | ModelState::Verifying => None,
        _ if downloading => Some(("Cancel", "cancel")),
        _ => Some(("Download", "download")),
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
    let size_line = ui::secondary(palette).child(format!("{} · Recommended", format_size(bytes)));
    rows.push(
        ui::icon_row(
            palette,
            IconName::Cpu,
            ui::title(palette)
                .id(hook::id("onboarding", "model.name"))
                .test_support()
                .child(name),
            Some(size_line.into_any_element()),
        )
        .child(
            ui::value(palette)
                .id(hook::id("onboarding", "model.status"))
                .test_support()
                .role(Role::Status)
                .aria_label(status.clone())
                .text_color(tone)
                .child(status),
        )
        .children(action.map(|(label, id)| {
            ui::button(
                palette,
                hook::id("onboarding", &format!("model.{id}")),
                label,
                Kind::Plain,
                Some(&focus),
                true,
                press,
            )
        }))
        .into_any_element(),
    );
    if downloading {
        rows.push(
            div()
                .w_full()
                .flex_none()
                .px(px(size::ROW_PADDING))
                .py(px(space::MD))
                .child(
                    ui::track(palette, fraction)
                        .id(hook::id("onboarding", "model.progress"))
                        .test_support()
                        .role(Role::ProgressIndicator),
                )
                .into_any_element(),
        );
    }
    ui::group(hook::id("onboarding", "model"), palette, rows)
}
