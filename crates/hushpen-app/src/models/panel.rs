//! The Models view: one group with a row for each catalog model.

use super::{ModelState, Models};
use crate::hook;
use crate::theme::{self, Palette, SECONDARY, StyledType};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Styled,
    Window, div, px,
};
use hushpen_core::catalog::Languages;

const STATUS_WIDTH: f32 = 92.0;

struct RowView {
    name: String,
    detail: String,
    recommended: bool,
    state: ModelState,
    active: bool,
}

#[derive(Clone, Copy)]
enum Action {
    Download,
    Cancel,
    Use,
    Delete,
}

pub fn render(models: &Entity<Models>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (rows, notice, focus) = {
        let models = models.read(cx);
        let active = models.active();
        let rows: Vec<RowView> = models
            .rows()
            .iter()
            .map(|row| RowView {
                name: row.entry.name.clone(),
                detail: if row.state == ModelState::Failed {
                    super::failure_text(&row.entry)
                } else {
                    format!(
                        "{} · {}",
                        format_size(row.entry.bytes),
                        match row.entry.languages {
                            Languages::Multilingual => "Multilingual",
                            Languages::English => "English only",
                        },
                    )
                },
                recommended: row.entry.default,
                state: row.state,
                active: row.entry.id == active,
            })
            .collect();
        let focus: Vec<[FocusHandle; 3]> =
            models.rows().iter().map(|row| row.focus.clone()).collect();
        (
            rows,
            models.notice().map(|notice| notice.message.clone()),
            focus,
        )
    };

    let mut group_rows: Vec<AnyElement> = Vec::new();
    if let Some(message) = notice {
        group_rows.push(ui::note_row(
            palette,
            hook::id("models", "notice"),
            message,
            true,
        ));
    }
    for (index, (row, focus)) in rows.into_iter().zip(focus).enumerate() {
        group_rows.push(model_row(palette, models, index, row, focus));
    }
    ui::plain_group(hook::id("models", "list"), palette, group_rows)
}

fn model_row(
    palette: Palette,
    models: &Entity<Models>,
    index: usize,
    row: RowView,
    focus: [FocusHandle; 3],
) -> AnyElement {
    let [primary_focus, use_focus, delete_focus] = focus;
    let state = row.state;
    let mut controls = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(theme::space::SM))
        .child(status(palette, index, &row));
    match state {
        ModelState::NotDownloaded => {
            controls = controls.child(button(
                palette,
                models,
                index,
                Action::Download,
                "Download",
                &primary_focus,
            ));
        }
        ModelState::Downloading { .. } | ModelState::Verifying => {
            if matches!(state, ModelState::Downloading { .. }) {
                controls = controls.child(button(
                    palette,
                    models,
                    index,
                    Action::Cancel,
                    "Cancel",
                    &primary_focus,
                ));
            }
        }
        ModelState::Ready | ModelState::Failed => {
            if state == ModelState::Failed {
                controls = controls.child(button(
                    palette,
                    models,
                    index,
                    Action::Download,
                    "Download",
                    &primary_focus,
                ));
            }
            if !row.active {
                controls = controls.child(button(
                    palette,
                    models,
                    index,
                    Action::Use,
                    "Use",
                    &use_focus,
                ));
            }
            controls = controls.child(button(
                palette,
                models,
                index,
                Action::Delete,
                "Delete",
                &delete_focus,
            ));
        }
    }

    ui::row(palette)
        .id(hook::indexed("models", "row", index))
        .test_support()
        .child(
            ui::text_column()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(theme::space::SM))
                        .child(
                            ui::title(palette)
                                .id(hook::indexed("models", "name", index))
                                .test_support()
                                .child(row.name),
                        )
                        .children(row.recommended.then(|| {
                            div()
                                .id(hook::indexed("models", "recommended", index))
                                .test_support()
                                .flex_none()
                                .text_token(SECONDARY)
                                .text_color(palette.label_secondary)
                                .child("Recommended")
                        })),
                )
                .child(
                    ui::secondary(palette)
                        .id(hook::indexed("models", "detail", index))
                        .test_support()
                        .w_full()
                        .child(row.detail),
                ),
        )
        .child(controls)
        .into_any_element()
}

/// The state of a model in words. Only the words that name a result take a status color.
fn status(palette: Palette, index: usize, row: &RowView) -> impl IntoElement + use<> {
    let (text, tone) = match row.state {
        ModelState::NotDownloaded => ("Not downloaded".to_owned(), palette.label_secondary),
        ModelState::Downloading { .. } => (
            row.state
                .percent()
                .map_or_else(|| "Starting".into(), |percent| format!("{percent}%")),
            palette.label,
        ),
        ModelState::Verifying => ("Checking".to_owned(), palette.label_secondary),
        ModelState::Ready if row.active => ("Active".to_owned(), palette.success),
        ModelState::Ready => ("Downloaded".to_owned(), palette.label_secondary),
        ModelState::Failed => ("Failed verification".to_owned(), palette.error),
    };
    ui::value(palette)
        .id(hook::indexed("models", "status", index))
        .test_support()
        .flex()
        .items_center()
        .justify_end()
        .min_w(px(STATUS_WIDTH))
        .text_color(tone)
        .child(text)
}

fn button(
    palette: Palette,
    models: &Entity<Models>,
    index: usize,
    action: Action,
    label: &'static str,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let press = {
        let models = models.clone();
        move |_: &mut Window, cx: &mut App| {
            models.update(cx, |models, cx| {
                let Some(id) = models.rows().get(index).map(|row| row.entry.id.clone()) else {
                    return;
                };
                // A refused action shows its reason in the notice row.
                let _ = match action {
                    Action::Download => models.download(&id, cx),
                    Action::Cancel => models.cancel(&id, cx),
                    Action::Use => models.select(&id, cx),
                    Action::Delete => models.delete(&id, cx),
                };
            });
        }
    };
    let element = match action {
        Action::Download => hook::indexed("models", "download", index),
        Action::Cancel => hook::indexed("models", "cancel", index),
        Action::Use => hook::indexed("models", "select", index),
        Action::Delete => hook::indexed("models", "delete", index),
    };
    ui::button(
        palette,
        element,
        label,
        Kind::Plain,
        Some(focus),
        true,
        press,
    )
}

/// Sizes in the units whisper.cpp uses on its model page: MB below 1 GB, then GB.
pub fn format_size(bytes: u64) -> String {
    const MB: f64 = 1_000_000.0;
    let value = bytes as f64;
    if value < 1_000.0 * MB {
        format!("{:.0} MB", value / MB)
    } else {
        format!("{:.1} GB", value / (1_000.0 * MB))
    }
}

#[cfg(test)]
mod tests {
    use super::format_size;

    #[test]
    fn sizes_use_decimal_megabytes_then_gigabytes() {
        assert_eq!(format_size(77_704_715), "78 MB");
        assert_eq!(format_size(1_624_555_275), "1.6 GB");
        assert_eq!(format_size(999_000_000), "999 MB");
    }
}
