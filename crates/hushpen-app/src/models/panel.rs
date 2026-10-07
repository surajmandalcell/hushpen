//! The Models view: one grouped panel with a row for each catalog model.

use super::{ModelState, Models};
use crate::hook;
use crate::theme::{
    self, BODY_MD, BODY_SM, LABEL_MD, ROW_TITLE, StyledType, color, radius, size, space,
};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
};
use hushpen_core::catalog::Languages;

const PROGRESS_WIDTH: f32 = 72.0;

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

    let mut panel = div()
        .id(hook::id("models", "list"))
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER));
    if let Some(message) = notice {
        panel = panel.child(notice_row(message));
    }
    for (index, (row, focus)) in rows.into_iter().zip(focus).enumerate() {
        panel = panel.child(model_row(models, index, row, focus));
    }
    panel
}

fn notice_row(message: String) -> impl IntoElement {
    div()
        .id(hook::id("models", "notice"))
        .test_support()
        .role(Role::Alert)
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_b_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::STATUS_ERROR))
        .child(
            svg()
                .path(IconName::CircleAlert.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::STATUS_ERROR)),
        )
        .child(div().flex_1().min_w_0().child(message))
}

fn model_row(
    models: &Entity<Models>,
    index: usize,
    row: RowView,
    focus: [FocusHandle; 3],
) -> impl IntoElement + use<> {
    let [primary_focus, use_focus, delete_focus] = focus;
    let state = row.state;
    let mut controls = div().flex_none().flex().items_center().gap(px(space::SM));
    controls = controls.child(status(index, &row));
    match state {
        ModelState::NotDownloaded => {
            controls = controls.child(button(
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
                    models,
                    index,
                    Action::Download,
                    "Download",
                    &primary_focus,
                ));
            }
            if !row.active {
                controls = controls.child(button(models, index, Action::Use, "Use", &use_focus));
            }
            controls = controls.child(button(
                models,
                index,
                Action::Delete,
                "Delete",
                &delete_focus,
            ));
        }
    }

    let mut container = div()
        .id(hook::indexed("models", "row", index))
        .test_support()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)));
    if index > 0 {
        container = container
            .border_t_1()
            .border_color(theme::rgb_of(color::DIVIDER));
    }
    container
        .child(icon_tile(IconName::Cpu))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(space::SM))
                        .child(
                            div()
                                .id(hook::indexed("models", "name", index))
                                .test_support()
                                .text_token(ROW_TITLE)
                                .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                                .truncate()
                                .child(row.name),
                        )
                        .children(row.recommended.then(|| {
                            div()
                                .id(hook::indexed("models", "recommended", index))
                                .test_support()
                                .flex_none()
                                .text_token(BODY_SM)
                                .text_color(theme::rgb_of(color::TEXT_SUBTLE))
                                .child("Recommended")
                        })),
                )
                .child(
                    div()
                        .id(hook::indexed("models", "detail", index))
                        .test_support()
                        .w_full()
                        .min_w_0()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .truncate()
                        .child(row.detail),
                ),
        )
        .child(controls)
}

fn status(index: usize, row: &RowView) -> impl IntoElement + use<> {
    let (text, tone): (String, u32) = match row.state {
        ModelState::NotDownloaded => ("Not downloaded".into(), color::TEXT_SUBTLE),
        ModelState::Downloading { .. } => (
            row.state
                .percent()
                .map_or_else(|| "Starting".into(), |percent| format!("{percent}%")),
            color::TEXT_BODY,
        ),
        ModelState::Verifying => ("Checking".into(), color::TEXT_MUTED),
        ModelState::Ready if row.active => ("Active".into(), color::FOCUS),
        ModelState::Ready => ("Downloaded".into(), color::TEXT_MUTED),
        ModelState::Failed => ("Failed verification".into(), color::STATUS_ERROR),
    };
    div()
        .id(hook::indexed("models", "status", index))
        .test_support()
        .flex()
        .flex_none()
        .items_center()
        .justify_end()
        .min_w(px(PROGRESS_WIDTH))
        .whitespace_nowrap()
        .text_token(BODY_SM)
        .text_color(theme::rgb_of(tone))
        .child(text)
}

fn button(
    models: &Entity<Models>,
    index: usize,
    action: Action,
    label: &'static str,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let press = {
        let models = models.clone();
        move |cx: &mut App| {
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
    let on_click = {
        let press = press.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| press(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            press(cx);
            cx.stop_propagation();
        }
    };
    let element: SharedString = match action {
        Action::Download => hook::indexed("models", "download", index),
        Action::Cancel => hook::indexed("models", "cancel", index),
        Action::Use => hook::indexed("models", "select", index),
        Action::Delete => hook::indexed("models", "delete", index),
    };
    div()
        .id(element)
        .test_support()
        .track_focus(focus)
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .h(px(32.0))
        .px(px(10.0 - size::FOCUS_RING))
        .rounded(px(radius::MD))
        .border_2()
        .border_color(transparent_black())
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
        .cursor_pointer()
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(label)
}

fn icon_tile(icon: IconName) -> impl IntoElement {
    div()
        .size(px(28.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .bg(theme::rgb_of(color::SURFACE_SOFT))
        .child(
            svg()
                .path(icon.path())
                .size(px(14.0))
                .text_color(theme::rgb_of(color::TEXT_BODY)),
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
