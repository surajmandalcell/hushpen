//! The Models view: a section for each kind of model (speech to text, text cleanup, text to
//! speech), with a row for each model. A row's details button opens its sheet
//! ([`super::sheet`]). The model step of onboarding draws the speech rows with the same builder
//! ([`speech_rows`]).

use super::{DELETE, INFO, ModelState, Models, PRIMARY, Row, SET_DEFAULT, search};
use crate::hook;
use crate::theme::{self, BODY, Palette, SECONDARY, StyledType, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Background, Entity, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Rgba, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    linear_color_stop, linear_gradient, px,
};
use hushpen_core::catalog::{Languages, ModelKind};

/// Where a row is drawn. Its hook ids are `models.<part>.<index>` in the Models view and
/// `onboarding.model.<part>.<index>` on the model step, with the index of
/// [`Models::all_rows`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Models,
    Onboarding,
}

impl Place {
    fn id(self, part: &str, index: usize) -> SharedString {
        match self {
            Place::Models => hook::indexed("models", part, index),
            Place::Onboarding => hook::indexed("onboarding", &format!("model.{part}"), index),
        }
    }
}

/// What one row shows, read out of [`Models`] before the elements are built.
struct RowView {
    index: usize,
    kind: ModelKind,
    name: String,
    detail: String,
    bytes: u64,
    recommended: bool,
    state: ModelState,
    updating: bool,
    /// The speech model a dictation uses.
    default: bool,
    /// Accuracy and speed in half dots, 1 to 10, of a speech or cleanup model with details.
    meters: Option<(u8, u8)>,
    focus: [FocusHandle; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Download,
    Cancel,
    SetDefault,
    Delete,
}

impl Action {
    fn icon(self) -> IconName {
        match self {
            Action::Download => IconName::Download,
            Action::Cancel => IconName::X,
            Action::SetDefault => IconName::Star,
            Action::Delete => IconName::Trash,
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Action::Download => "Download",
            Action::Cancel => "Cancel",
            Action::SetDefault => "Set as default",
            Action::Delete => "Delete",
        }
    }

    /// The name in the accessibility tree, which says what the button does to which model.
    fn label(self, view: &RowView) -> String {
        let name = &view.name;
        match self {
            Action::Download => format!("Download {name}"),
            Action::Cancel if view.updating => format!("Cancel the update of {name}"),
            Action::Cancel => format!("Cancel the download of {name}"),
            Action::SetDefault => format!("Set {name} as default"),
            Action::Delete => format!("Delete {name}"),
        }
    }

    fn element(self) -> &'static str {
        match self {
            Action::Download => "download",
            Action::Cancel => "cancel",
            Action::SetDefault => "select",
            Action::Delete => "delete",
        }
    }

    /// The slot of the button after Details, which is also its focus handle in [`Row::focus`].
    fn slot(self) -> usize {
        match self {
            Action::Download | Action::Cancel => PRIMARY,
            Action::SetDefault => SET_DEFAULT,
            Action::Delete => DELETE,
        }
    }
}

/// The heading and the help line of each section.
fn section_text(kind: ModelKind) -> (&'static str, &'static str) {
    match kind {
        ModelKind::Speech => (
            "Speech to Text",
            "Nemotron Unified is recommended. Parakeet is faster on long files. Use Whisper for other languages or translation.",
        ),
        ModelKind::Cleanup => (
            "Text cleanup",
            "Fixes grammar and punctuation when AI cleanup is on. Choose the model in Settings.",
        ),
        ModelKind::Voice => ("Text to Speech", "Reads text and files aloud."),
    }
}

/// A section of the Models view: Active, which holds the models a dictation uses, or a kind.
#[derive(Clone, Copy)]
enum Section {
    Active,
    Kind(ModelKind),
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Section::Active => "active",
            Section::Kind(kind) => kind.key(),
        }
    }

    fn text(self) -> (&'static str, &'static str) {
        match self {
            Section::Active => ("Active", "The models Hushpen uses when you dictate."),
            Section::Kind(kind) => section_text(kind),
        }
    }
}

pub fn render(models: &Entity<Models>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (sections, notice, query, clear_focus, any_rows) = {
        let models = models.read(cx);
        let default = models.effective();
        let in_use: Vec<String> = std::iter::once(default.clone())
            .chain(models.active_cleanup())
            .collect();
        let query = models.query().to_owned();
        let shown = |active: bool| {
            let (default, in_use, query) = (&default, &in_use, &query);
            models
                .all_rows()
                .iter()
                .enumerate()
                .filter(move |(_, row)| in_use.contains(&row.entry.id) == active)
                .filter(move |(_, row)| search::matches(row, query))
                .map(move |(index, row)| (row.entry.kind, row_view(index, row, default)))
        };
        let mut sections: Vec<(Section, Vec<RowView>)> =
            vec![(Section::Active, shown(true).map(|(_, view)| view).collect())];
        for kind in ModelKind::ALL {
            let views = shown(false)
                .filter(|(of, _)| *of == kind)
                .map(|(_, view)| view)
                .collect();
            sections.push((Section::Kind(kind), views));
        }
        sections.retain(|(_, views)| !views.is_empty());
        (
            sections,
            models.notice().map(|notice| notice.message.clone()),
            query,
            models.search.clear.clone(),
            !models.all_rows().is_empty(),
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
    if sections.is_empty() && any_rows && !query.is_empty() {
        page = page.child(no_match(palette, models, &query, clear_focus.as_ref()));
    }
    // One button area for the whole screen, so the meters line up across sections.
    let slots = sections
        .iter()
        .map(|(_, views)| widest_buttons(views))
        .max()
        .unwrap_or(1);
    for (section, views) in sections {
        let (title, help) = section.text();
        let key = section.key();
        let rows = views
            .into_iter()
            .map(|view| model_row(palette, models, view, Place::Models, slots))
            .collect();
        page = page.child(ui::titled_section(
            palette,
            hook::id("models", &format!("heading.{key}")),
            title,
            ui::group(hook::id("models", &format!("list.{key}")), palette, rows),
            Some((hook::id("models", &format!("help.{key}")), help.to_owned())),
        ));
    }
    page
}

/// The speech rows as the model step of onboarding shows them: the rows of the Models view,
/// with a progress bar in place of the description while a model downloads.
pub fn speech_rows(palette: Palette, models: &Entity<Models>, cx: &App) -> Vec<AnyElement> {
    let views: Vec<RowView> = {
        let models = models.read(cx);
        let default = models.effective();
        let (start, rows) = models.of_kind(ModelKind::Speech);
        rows.iter()
            .enumerate()
            .map(|(offset, row)| row_view(start + offset, row, &default))
            .collect()
    };
    let slots = widest_buttons(&views);
    views
        .into_iter()
        .map(|view| model_row(palette, models, view, Place::Onboarding, slots))
        .collect()
}

/// The group that says no model matches, with a button that clears the search.
fn no_match(
    palette: Palette,
    models: &Entity<Models>,
    query: &str,
    focus: Option<&FocusHandle>,
) -> AnyElement {
    let models = models.clone();
    let clear = ui::button(
        palette,
        hook::id("models", "search.clear"),
        "Clear",
        Kind::Plain,
        focus,
        true,
        move |window, cx| {
            models.update(cx, |models, cx| models.clear_search(window, cx));
        },
    );
    ui::group(
        hook::id("models", "search.empty.group"),
        palette,
        vec![ui::notice_row(
            palette,
            hook::id("models", "search.empty"),
            format!("No models match \"{query}\""),
            Notice::Info,
            Some(clear.into_any_element()),
        )],
    )
    .into_any_element()
}

/// The right end of the title row: the search field, then Benchmarks, which opens the page with
/// the numbers behind the dots. Nothing before the field is attached.
pub fn search_field(models: &Entity<Models>, cx: &App) -> Option<impl IntoElement + use<>> {
    let palette = theme::palette(cx);
    let (state, focus) = {
        let models = models.read(cx);
        (
            models.search_field_state()?.clone(),
            models.search.benchmarks.clone(),
        )
    };
    let open = {
        let models = models.clone();
        move |_: &mut Window, cx: &mut App| {
            models.update(cx, |models, cx| models.open_benchmarks(cx));
        }
    };
    Some(
        div()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .child(
                div()
                    .w(px(size::SEARCH_FIELD_WIDTH))
                    .flex_none()
                    .child(ui::search_field(
                        palette,
                        hook::id("models", "search"),
                        "Search models",
                        &state,
                    )),
            )
            .child(ui::button(
                palette,
                hook::id("models", "benchmarks"),
                "Benchmarks",
                Kind::Plain,
                focus.as_ref(),
                true,
                open,
            )),
    )
}

fn row_view(index: usize, row: &Row, default: &str) -> RowView {
    let detail = match (&row.entry.info, row.state) {
        (_, ModelState::Failed) => super::failure_text(&row.entry),
        (Some(info), _) => info.description.clone(),
        (None, _) => languages_text(row.entry.languages).to_owned(),
    };
    let speech = row.entry.kind == ModelKind::Speech;
    let rated = speech || row.entry.kind == ModelKind::Cleanup;
    RowView {
        index,
        kind: row.entry.kind,
        name: row.entry.name.clone(),
        detail,
        bytes: row.entry.bytes(),
        recommended: row.recommended,
        state: row.state,
        updating: row.updating,
        default: speech && row.entry.id == default,
        meters: row
            .entry
            .info
            .as_ref()
            .filter(|_| rated)
            .map(|info| (info.accuracy, info.speed)),
        focus: row.focus.clone(),
    }
}

/// The buttons a row offers in its state, after Details. Only a speech model can be made the
/// default here; Settings chooses the cleanup model, and Read aloud the voice. A failed row
/// offers all three, so the user can replace it, choose it (which says why it cannot be used),
/// or delete it.
fn actions(view: &RowView) -> Vec<Action> {
    let speech = view.kind == ModelKind::Speech;
    match view.state {
        ModelState::NotDownloaded => vec![Action::Download],
        ModelState::Downloading { .. } => vec![Action::Cancel],
        ModelState::Verifying => Vec::new(),
        ModelState::Ready if speech && !view.default => vec![Action::SetDefault, Action::Delete],
        ModelState::Ready => vec![Action::Delete],
        ModelState::Failed if speech => {
            vec![Action::Download, Action::SetDefault, Action::Delete]
        }
        ModelState::Failed => vec![Action::Download, Action::Delete],
    }
}

/// The buttons a row shows: Details and the actions of its state.
fn button_count(view: &RowView) -> usize {
    1 + actions(view).len()
}

/// The most buttons any of `views` shows now, which sets the width of the button column of
/// their section.
fn widest_buttons(views: &[RowView]) -> usize {
    views.iter().map(button_count).max().unwrap_or(1)
}

/// The name, Recommended, and the state word over the description; then, lightest to heaviest,
/// the meters, the size, and the icon buttons. The size and the buttons pack to the right end of
/// a box as wide as the size and `slots` buttons, so the last button ends at the row's padding,
/// a row with fewer buttons moves its size right, and the meters line up from row to row.
fn model_row(
    palette: Palette,
    models: &Entity<Models>,
    view: RowView,
    place: Place,
    slots: usize,
) -> AnyElement {
    let index = view.index;
    // In the Models view the default sits under Active, which already says it is in use.
    let default = view.default && place == Place::Onboarding;
    let word = state_word(palette, view.state, default, view.updating);
    let name_line = div()
        .flex()
        .items_baseline()
        .gap(px(space::SM))
        .child(
            ui::title(palette)
                .id(place.id("name", index))
                .test_support()
                .child(view.name.clone()),
        )
        .children(view.recommended.then(|| {
            div()
                .id(place.id("recommended", index))
                .test_support()
                .flex_none()
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .child("Recommended")
        }))
        .children(word.map(|(text, tone)| {
            div()
                .id(place.id("state", index))
                .test_support()
                .role(Role::Status)
                .aria_label(text.clone())
                .flex_none()
                .whitespace_nowrap()
                .text_token(SECONDARY)
                .text_color(tone)
                .child(text)
        }));
    let downloading = matches!(view.state, ModelState::Downloading { .. });
    let under = if place == Place::Onboarding && downloading {
        let fraction = view
            .state
            .percent()
            .map_or(0.0, |percent| percent as f32 / 100.0);
        div()
            .flex()
            .items_center()
            .w_full()
            .h(px(line_height(SECONDARY)))
            .child(
                ui::track(palette, fraction)
                    .id(place.id("progress", index))
                    .test_support()
                    .role(Role::ProgressIndicator)
                    .aria_label("Download progress"),
            )
            .into_any_element()
    } else {
        ui::secondary(palette)
            .id(place.id("detail", index))
            .test_support()
            .w_full()
            .child(view.detail.clone())
            .into_any_element()
    };
    let controls = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .children(
            view.meters
                .map(|(accuracy, speed)| meters(palette, place, index, accuracy, speed)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(space::SM))
                .w(px(size::MODEL_SIZE_WIDTH
                    + space::SM
                    + ui::icon_group_width(slots, size::MODEL_ICON_GAP)))
                .child(
                    ui::value(palette)
                        .id(place.id("size", index))
                        .test_support()
                        .w(px(size::MODEL_SIZE_WIDTH))
                        .text_right()
                        .child(format_size(view.bytes)),
                )
                .child(icon_buttons(palette, models, place, &view)),
        );

    ui::row(palette)
        .id(place.id("row", index))
        .test_support()
        .gap(px(space::MD))
        .child(ui::text_column().child(name_line).child(under))
        .child(controls)
        .into_any_element()
}

/// The height of one line of `token` text.
fn line_height(token: theme::TextToken) -> f32 {
    token.size * token.line_height
}

/// Accuracy over speed, each a label and five dots, lined up with the name and the description.
fn meters(palette: Palette, place: Place, index: usize, accuracy: u8, speed: u8) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .flex_col()
        .w(px(size::MODEL_METER_WIDTH))
        .child(meter(
            palette,
            place.id("accuracy", index),
            ("Accuracy", ACCURACY),
            accuracy,
            palette.accent,
            line_height(BODY),
        ))
        .child(meter(
            palette,
            place.id("speed", index),
            ("Speed", SPEED),
            speed,
            palette.label_secondary,
            line_height(SECONDARY),
        ))
        .into_any_element()
}

/// One meter: its label, then five dots with `halves` half dots filled in `fill`. Its name and
/// tooltip say it in words, such as `Accuracy: High (4.5 of 5)`.
fn meter(
    palette: Palette,
    id: SharedString,
    (label, words): (&'static str, [&'static str; 5]),
    halves: u8,
    fill: Rgba,
    height: f32,
) -> impl IntoElement {
    let text = format!("{label}: {}", scale(halves, words));
    // Much dimmer than a filled gray Speed dot, so 3 of 5 never reads as 5 in Dark.
    let empty = palette.separator;
    let dots = (0..5u8).map(|dot| {
        let background: Background = match halves.saturating_sub(dot * 2) {
            0 => empty.into(),
            // Half a dot: a hard stop in the middle, filled on the left.
            1 => linear_gradient(
                90.0,
                linear_color_stop(fill, 0.5),
                linear_color_stop(empty, 0.501),
            ),
            _ => fill.into(),
        };
        div()
            .flex_none()
            .size(px(size::MODEL_DOT))
            .rounded_full()
            .bg(background)
    });
    let line = div()
        .id(id)
        .test_support()
        .role(Role::Meter)
        .aria_label(text.clone())
        .flex()
        .items_center()
        .gap(px(size::MODEL_METER_GAP))
        .h(px(height))
        .child(
            div()
                .flex_none()
                .w(px(size::MODEL_METER_LABEL))
                .whitespace_nowrap()
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .child(label),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap(px(size::MODEL_DOT_GAP))
                .children(dots),
        );
    ui::tooltip(line, text)
}

/// Details, then Download or Cancel, Set as default, and Delete, as many as the row's state
/// offers, in a group exactly as wide as its buttons, so a missing button leaves no hole.
fn icon_buttons(
    palette: Palette,
    models: &Entity<Models>,
    place: Place,
    view: &RowView,
) -> AnyElement {
    let index = view.index;
    let details = {
        let models = models.clone();
        ui::quiet_icon_button(
            palette,
            place.id("info", index),
            IconName::Info,
            format!("Details of {}", view.name),
            "Details",
            &view.focus[INFO],
            move |window: &mut Window, cx: &mut App| {
                models.update(cx, |models, cx| models.open_details(index, window, cx));
            },
        )
        .into_any_element()
    };
    ui::icon_group(button_count(view), size::MODEL_ICON_GAP)
        .child(details)
        .children(
            actions(view)
                .into_iter()
                .map(|action| button(palette, models, place, view, action)),
        )
        .into_any_element()
}

/// The word after the name of a row, with its color. A model that is downloaded, or not, has
/// none; only a failure takes a status color.
pub(super) fn state_word(
    palette: Palette,
    state: ModelState,
    default: bool,
    updating: bool,
) -> Option<(String, Rgba)> {
    match state {
        ModelState::NotDownloaded => None,
        ModelState::Downloading { .. } if updating => Some((
            state.percent().map_or_else(
                || "Updating".into(),
                |percent| format!("Updating {percent}%"),
            ),
            palette.label,
        )),
        ModelState::Downloading { .. } => Some((
            state
                .percent()
                .map_or_else(|| "Starting".into(), |percent| format!("{percent}%")),
            palette.label,
        )),
        ModelState::Verifying => Some(("Checking".to_owned(), palette.label_secondary)),
        ModelState::Ready if default => Some(("Default".to_owned(), palette.label_secondary)),
        ModelState::Ready => None,
        ModelState::Failed => Some(("Failed verification".to_owned(), palette.error)),
    }
}

/// The state of a model in words for the Status fact of its details sheet.
pub(super) fn status_text(
    palette: Palette,
    state: ModelState,
    default: bool,
    updating: bool,
) -> String {
    state_word(palette, state, default, updating).map_or_else(
        || match state {
            ModelState::Ready => "Downloaded".to_owned(),
            _ => "Not downloaded".to_owned(),
        },
        |(word, _)| word,
    )
}

pub(super) fn languages_text(languages: Languages) -> &'static str {
    match languages {
        Languages::Multilingual => "Multilingual",
        Languages::English => "English only",
        Languages::European => "25 European languages",
    }
}

/// The words for 1 to 5 on the speed and the accuracy scales.
pub(super) const SPEED: [&str; 5] = ["Slowest", "Slow", "Moderate", "Fast", "Fastest"];
pub(super) const ACCURACY: [&str; 5] = ["Basic", "Fair", "Good", "High", "Highest"];

/// `Fast (4 of 5)` or `High (4.5 of 5)` for a value in half dots. A half takes the word below.
pub(super) fn scale(halves: u8, words: [&str; 5]) -> String {
    let word = words
        .get(usize::from((halves / 2).saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    let dots = if halves.is_multiple_of(2) {
        (halves / 2).to_string()
    } else {
        format!("{}.5", halves / 2)
    };
    format!("{word} ({dots} of 5)")
}

fn button(
    palette: Palette,
    models: &Entity<Models>,
    place: Place,
    view: &RowView,
    action: Action,
) -> AnyElement {
    let index = view.index;
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
                    Action::SetDefault => models.select(&id, cx),
                    Action::Delete => models.delete(&id, cx),
                };
            });
        }
    };
    ui::icon_button(
        palette,
        place.id(action.element(), index),
        action.icon(),
        action.label(view),
        action.tooltip(),
        &view.focus[action.slot()],
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

/// The names of the section a row is listed in, for the search: its heading, and the name it
/// had before.
pub(super) fn section_names(kind: ModelKind) -> &'static [&'static str] {
    match kind {
        ModelKind::Speech => &["speech to text", "speech recognition"],
        ModelKind::Cleanup => &["text cleanup"],
        ModelKind::Voice => &["text to speech", "voices"],
    }
}

#[cfg(test)]
mod tests {
    use super::{ACCURACY, SPEED, format_size, scale};

    #[test]
    fn sizes_use_decimal_megabytes_then_gigabytes() {
        assert_eq!(format_size(77_704_715), "78 MB");
        assert_eq!(format_size(1_624_555_275), "1.6 GB");
        assert_eq!(format_size(999_000_000), "999 MB");
        assert_eq!(format_size(48_200), "48 KB");
    }

    #[test]
    fn a_scale_names_the_word_and_the_value_of_five() {
        assert_eq!(scale(8, ACCURACY), "High (4 of 5)");
        assert_eq!(scale(9, ACCURACY), "High (4.5 of 5)");
        assert_eq!(scale(1, SPEED), "Slowest (0.5 of 5)");
        assert_eq!(scale(2, SPEED), "Slowest (1 of 5)");
        assert_eq!(scale(10, SPEED), "Fastest (5 of 5)");
    }
}
