//! The details sheet of one model, drawn over the whole window like a macOS sheet: what the model
//! is for, its facts, where it comes from and where it lives, and its checksum. Tab stays inside
//! the sheet, Esc and Done close it, and focus goes back to the row's details button.

use super::panel::{format_size, languages_text, status_text};
use super::{Models, Row, details::checksum_text};
use crate::theme::{self, Palette, SECONDARY, StyledType, TOOLBAR_TITLE, radius, space};
use crate::ui::{self, Kind};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use hushpen_core::catalog::ModelInfo;

/// What the folder button says on this system.
const SHOW_FILES: &str = if cfg!(target_os = "macos") {
    "Show in Finder"
} else {
    "Show in Files"
};

/// The sheet of the open row, or nothing while no details are open. As it opens, the scrim fades
/// in and the sheet fades in and rises a few pixels into place.
pub fn render(models: &Entity<Models>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    models.read(cx).details()?;
    let arrival = motion::enter(
        hook::id("models", "sheet.arrival"),
        motion::BASE,
        window,
        cx,
    );
    let palette = theme::palette(cx);
    let me = models.read(cx);
    let row = me.details()?;
    let info = row.entry.info.clone();
    let focus = &me.sheet_focus;
    let (status, _) = status_text(palette, row.state, me.active() == row.entry.id);
    let facts = vec![
        fact(
            palette,
            "languages",
            "Languages",
            languages_text(row.entry.languages).to_owned(),
        ),
        fact(
            palette,
            "speed",
            "Speed",
            info.as_ref().map(|i| scale(i.speed, SPEED)),
        ),
        fact(
            palette,
            "accuracy",
            "Accuracy",
            info.as_ref().map(|i| scale(i.accuracy, ACCURACY)),
        ),
        fact(
            palette,
            "memory",
            "Memory",
            info.as_ref()
                .map(|i| format!("About {}", format_size(i.ram_bytes))),
        ),
        fact(palette, "size", "Size on disk", size_text(row)),
        fact(palette, "license", "License", row.entry.license.clone()),
        fact(palette, "status", "Status", status),
    ];
    let files = vec![
        source_row(palette, models, info.as_ref(), &focus.source),
        ui::fact_row(
            palette,
            hook::id("models", "sheet.row.location"),
            hook::id("models", "sheet.location"),
            "Location",
            Some(me.location(row)),
            Some(sheet_button(
                palette,
                models,
                "show",
                SHOW_FILES,
                &focus.show,
                |models, cx| models.show_files(cx),
            )),
        ),
        ui::fact_row(
            palette,
            hook::id("models", "sheet.row.checksum"),
            hook::id("models", "sheet.checksum"),
            "SHA-256",
            Some(checksum_text(row).replace('\n', ", ")),
            Some(sheet_button(
                palette,
                models,
                "copy",
                "Copy",
                &focus.copy,
                |models, cx| models.copy_checksum(cx),
            )),
        ),
    ];
    let close = {
        let models = models.clone();
        move |window: &mut Window, cx: &mut App| {
            models.update(cx, |models, cx| models.close_details(window, cx));
        }
    };
    let on_key = {
        let close = close.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                close(window, cx);
                cx.stop_propagation();
            }
        }
    };
    let sheet = div()
        .id(hook::id("models", "sheet"))
        .test_support()
        .role(Role::Dialog)
        .aria_label(format!("{} details", row.entry.name))
        .on_key_down(on_key)
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w(px(space::SHEET_WIDTH))
        .p(px(space::XL))
        .bg(palette.content)
        .rounded(px(radius::WINDOW))
        .border(theme::hairline())
        .border_color(palette.separator)
        .child(header(palette, row, info.as_ref()))
        .child(ui::group(hook::id("models", "sheet.facts"), palette, facts))
        .child(ui::group(hook::id("models", "sheet.files"), palette, files))
        .child(div().flex().justify_end().child(ui::button(
            palette,
            hook::id("models", "sheet.done"),
            "Done",
            Kind::Default,
            Some(&focus.done),
            true,
            close,
        )));
    // The trap takes over the id of the element it wraps, so it goes on the scrim and the sheet
    // keeps its own.
    Some(
        div()
            .focus_trap(hook::id("models", "sheet.scrim"), &focus.trap)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::faded(palette.scrim, arrival))
            .child(motion::arrive(sheet, arrival, motion::TRAVEL))
            .into_any_element(),
    )
}

/// The name, the one-line description, and what the model suits.
fn header(palette: Palette, row: &Row, info: Option<&ModelInfo>) -> impl IntoElement {
    let line = |id: &str, text: String| {
        div()
            .id(hook::id("models", id))
            .test_support()
            .aria_label(text.clone())
            .text_token(SECONDARY)
            .text_color(palette.label_secondary)
            .child(text)
    };
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(hook::id("models", "sheet.title"))
                .test_support()
                .role(Role::Heading)
                .aria_label(row.entry.name.clone())
                .text_token(TOOLBAR_TITLE)
                .text_color(palette.label)
                .truncate()
                .child(row.entry.name.clone()),
        )
        .children(info.map(|info| line("sheet.description", info.description.clone())))
        .children(info.map(|info| {
            line(
                "sheet.recommendedFor",
                format!("Good for: {}", info.recommended_for),
            )
        }))
}

fn fact(
    palette: Palette,
    key: &str,
    title: &'static str,
    value: impl Into<Option<String>>,
) -> AnyElement {
    ui::fact_row(
        palette,
        hook::id("models", &format!("sheet.row.{key}")),
        hook::id("models", &format!("sheet.{key}")),
        title,
        value.into(),
        None,
    )
}

/// The words for 1 to 5 on the speed and the accuracy scales.
const SPEED: [&str; 5] = ["Slowest", "Slow", "Moderate", "Fast", "Fastest"];
const ACCURACY: [&str; 5] = ["Basic", "Fair", "Good", "High", "Highest"];

/// `Fast (4 of 5)`.
fn scale(value: u8, words: [&str; 5]) -> String {
    let word = words
        .get(usize::from(value.saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    format!("{word} ({value} of 5)")
}

fn size_text(row: &Row) -> String {
    let size = format_size(row.entry.bytes());
    match row.entry.files.len() {
        1 => size,
        count => format!("{size} in {count} files"),
    }
}

/// The source page as a link row: the host and the repository, and the external-link glyph.
fn source_row(
    palette: Palette,
    models: &Entity<Models>,
    info: Option<&ModelInfo>,
    focus: &gpui_kit::FocusHandle,
) -> AnyElement {
    let page = info.map(|info| info.source_page.trim_start_matches("https://").to_owned());
    let open = {
        let models = models.clone();
        move |_: &mut Window, cx: &mut App| models.update(cx, |models, cx| models.open_source(cx))
    };
    ui::press_row(
        palette,
        hook::id("models", "sheet.source"),
        "Open the source page",
        focus,
        page.is_some(),
        open,
    )
    .gap(px(space::LG))
    .child(div().flex_none().child("Source"))
    .child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_right()
            .text_color(palette.label_secondary)
            .child(page.unwrap_or_else(|| "Not available".to_owned())),
    )
    .child(ui::glyph(palette, IconName::ExternalLink))
    .into_any_element()
}

fn sheet_button(
    palette: Palette,
    models: &Entity<Models>,
    key: &str,
    label: &'static str,
    focus: &gpui_kit::FocusHandle,
    act: fn(&mut Models, &mut gpui_kit::Context<Models>),
) -> AnyElement {
    let models = models.clone();
    ui::button(
        palette,
        hook::id("models", &format!("sheet.{key}")),
        label,
        Kind::Plain,
        Some(focus),
        true,
        move |_: &mut Window, cx: &mut App| models.update(cx, act),
    )
    .into_any_element()
}
