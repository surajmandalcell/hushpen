//! The Models view: a section for each kind of model (speech recognition, text cleanup, voices),
//! with a row for each model. A row's info button opens its details sheet ([`super::sheet`]).

use super::{DELETE, INFO, ModelState, Models, PRIMARY, Row, USE};
use crate::hook;
use crate::theme::{self, Palette, SECONDARY, StyledType, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Rgba,
    Styled, Window, div, px,
};
use hushpen_core::catalog::{Languages, ModelKind};

/// What one row shows, read out of [`Models`] before the elements are built.
struct RowView {
    index: usize,
    kind: ModelKind,
    name: String,
    detail: String,
    bytes: u64,
    recommended: bool,
    state: ModelState,
    active: bool,
    focus: [FocusHandle; 4],
}

#[derive(Clone, Copy)]
enum Action {
    Download,
    Cancel,
    Use,
    Delete,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::Download => "Download",
            Action::Cancel => "Cancel",
            Action::Use => "Use",
            Action::Delete => "Delete",
        }
    }

    fn element(self) -> &'static str {
        match self {
            Action::Download => "download",
            Action::Cancel => "cancel",
            Action::Use => "select",
            Action::Delete => "delete",
        }
    }

    fn slot(self) -> usize {
        match self {
            Action::Download | Action::Cancel => PRIMARY,
            Action::Use => USE,
            Action::Delete => DELETE,
        }
    }
}

/// The heading and the help line of each section.
fn section_text(kind: ModelKind) -> (&'static str, &'static str) {
    match kind {
        ModelKind::Speech => (
            "Speech recognition",
            "Turns speech into text for dictation and files. Larger models are more accurate and slower.",
        ),
        ModelKind::Cleanup => (
            "Text cleanup",
            "Fixes grammar and punctuation when AI cleanup is on. Choose the model in Settings.",
        ),
        ModelKind::Voice => ("Voices", "Reads text and files aloud."),
    }
}

pub fn render(models: &Entity<Models>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (sections, notice) = {
        let models = models.read(cx);
        let active = models.active();
        let sections: Vec<(ModelKind, Vec<RowView>)> = ModelKind::ALL
            .into_iter()
            .map(|kind| {
                let (start, rows) = models.of_kind(kind);
                let views = rows
                    .iter()
                    .enumerate()
                    .map(|(offset, row)| row_view(start + offset, row, &active))
                    .collect();
                (kind, views)
            })
            .filter(|(_, views): &(_, Vec<RowView>)| !views.is_empty())
            .collect();
        (
            sections,
            models.notice().map(|notice| notice.message.clone()),
        )
    };

    let mut page = div().flex().flex_col().gap(px(space::XXL)).w_full();
    if let Some(message) = notice {
        page = page.child(ui::group(
            hook::id("models", "notices"),
            palette,
            vec![ui::notice_row(
                palette,
                hook::id("models", "notice"),
                message,
                Notice::Alert,
                None,
            )],
        ));
    }
    for (kind, views) in sections {
        let (title, help) = section_text(kind);
        let rows = views
            .into_iter()
            .map(|view| model_row(palette, models, view))
            .collect();
        page = page.child(ui::titled_section(
            palette,
            hook::id("models", &format!("heading.{}", kind.key())),
            title,
            ui::group(
                hook::id("models", &format!("list.{}", kind.key())),
                palette,
                rows,
            ),
            Some((
                hook::id("models", &format!("help.{}", kind.key())),
                help.to_owned(),
            )),
        ));
    }
    page
}

fn row_view(index: usize, row: &Row, active: &str) -> RowView {
    let detail = match (&row.entry.info, row.state) {
        (_, ModelState::Failed) => super::failure_text(&row.entry),
        (Some(info), _) => info.description.clone(),
        (None, _) => languages_text(row.entry.languages).to_owned(),
    };
    RowView {
        index,
        kind: row.entry.kind,
        name: row.entry.name.clone(),
        detail,
        bytes: row.entry.bytes(),
        recommended: row.recommended,
        state: row.state,
        active: row.entry.kind == ModelKind::Speech && row.entry.id == active,
        focus: row.focus.clone(),
    }
}

/// The buttons a row offers in its state. Only a speech model can be put to use here; Settings
/// chooses the cleanup model, and Read aloud the voice.
fn actions(view: &RowView) -> Vec<Action> {
    let mut actions = match view.state {
        ModelState::NotDownloaded | ModelState::Failed => vec![Action::Download],
        ModelState::Downloading { .. } => vec![Action::Cancel],
        ModelState::Verifying | ModelState::Ready => Vec::new(),
    };
    if matches!(view.state, ModelState::Ready | ModelState::Failed) {
        if view.kind == ModelKind::Speech && !view.active {
            actions.push(Action::Use);
        }
        actions.push(Action::Delete);
    }
    actions
}

/// Lightest to heaviest: the size, the status, the details button, then the buttons.
fn model_row(palette: Palette, models: &Entity<Models>, view: RowView) -> AnyElement {
    let index = view.index;
    let info = {
        let models = models.clone();
        ui::icon_button(
            palette,
            hook::indexed("models", "info", index),
            IconName::Info,
            format!("Details of {}", view.name),
            "Details",
            &view.focus[INFO],
            move |window: &mut Window, cx: &mut App| {
                models.update(cx, |models, cx| models.open_details(index, window, cx));
            },
        )
    };
    let buttons: Vec<AnyElement> = actions(&view)
        .into_iter()
        .map(|action| button(palette, models, index, action, &view.focus[action.slot()]))
        .collect();
    let (status, tone) = status_text(palette, view.state, view.active);
    let controls = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(
            ui::value(palette)
                .id(hook::indexed("models", "size", index))
                .test_support()
                .min_w(px(size::MODEL_SIZE_WIDTH))
                .text_right()
                .child(format_size(view.bytes)),
        )
        .child(
            ui::value(palette)
                .id(hook::indexed("models", "status", index))
                .test_support()
                .min_w(px(size::MODEL_STATUS_WIDTH))
                .text_right()
                .text_color(tone)
                .child(status),
        )
        .child(info)
        .child(
            div()
                .min_w(px(size::MODEL_ACTIONS_WIDTH))
                .flex()
                .justify_end()
                .gap(px(space::SM))
                .children(buttons),
        );

    ui::row(palette)
        .id(hook::indexed("models", "row", index))
        .test_support()
        .child(
            ui::text_column()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(space::SM))
                        .child(
                            ui::title(palette)
                                .id(hook::indexed("models", "name", index))
                                .test_support()
                                .child(view.name),
                        )
                        .children(view.recommended.then(|| {
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
                        .child(view.detail),
                ),
        )
        .child(controls)
        .into_any_element()
}

/// The state of a model in words, with its color. Only the words that name a result take a
/// status color.
pub(super) fn status_text(palette: Palette, state: ModelState, active: bool) -> (String, Rgba) {
    match state {
        ModelState::NotDownloaded => ("Not downloaded".to_owned(), palette.label_secondary),
        ModelState::Downloading { .. } => (
            state
                .percent()
                .map_or_else(|| "Starting".into(), |percent| format!("{percent}%")),
            palette.label,
        ),
        ModelState::Verifying => ("Checking".to_owned(), palette.label_secondary),
        ModelState::Ready if active => ("Active".to_owned(), palette.success),
        ModelState::Ready => ("Downloaded".to_owned(), palette.label_secondary),
        ModelState::Failed => ("Failed verification".to_owned(), palette.error),
    }
}

pub(super) fn languages_text(languages: Languages) -> &'static str {
    match languages {
        Languages::Multilingual => "Multilingual",
        Languages::English => "English only",
    }
}

fn button(
    palette: Palette,
    models: &Entity<Models>,
    index: usize,
    action: Action,
    focus: &FocusHandle,
) -> AnyElement {
    let press = {
        let models = models.clone();
        move |_: &mut Window, cx: &mut App| {
            models.update(cx, |models, cx| {
                let Some(id) = models.all_rows().get(index).map(|row| row.entry.id.clone()) else {
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
    ui::button(
        palette,
        hook::indexed("models", action.element(), index),
        action.label(),
        Kind::Plain,
        Some(focus),
        true,
        press,
    )
    .into_any_element()
}

/// Sizes in the units whisper.cpp uses on its model page: KB below 1 MB, MB below 1 GB, then
/// GB.
pub fn format_size(bytes: u64) -> String {
    const MB: f64 = 1_000_000.0;
    let value = bytes as f64;
    if value < MB {
        format!("{:.0} KB", value / 1_000.0)
    } else if value < 1_000.0 * MB {
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
        assert_eq!(format_size(48_200), "48 KB");
    }
}
