//! The History view: one group with the search box and a row for each transcript, or the detail
//! of one transcript.

use super::playback::{self, KEY_STEP_MS, PlaybackInfo};
use super::{History, ROW_HEIGHT};
use crate::controller::ReprocessState;
use crate::hook;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, App, Bounds, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Role, StatefulInteractiveElement, Styled, Window, div, px,
    transparent_black,
};
use hushpen_core::export::Format;
use hushpen_core::{catalog, language};
use hushpen_store::history::Row;
use std::cell::Cell;
use std::rc::Rc;

const EMPTY_TEXT: &str = "No transcripts yet. Dictate something and it shows here.";
const TITLE_CHARS: usize = 160;

/// What the list shows for a row.
pub fn title(row: &Row) -> String {
    if row.status == "cancelled" {
        return "Not transcribed".to_owned();
    }
    let text = [&row.final_text, &row.rule_text, &row.raw_text]
        .into_iter()
        .flatten()
        .find(|text| !text.trim().is_empty());
    if let Some(text) = text {
        let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
        return match line.char_indices().nth(TITLE_CHARS) {
            Some((end, _)) => format!("{}\u{2026}", &line[..end]),
            None => line,
        };
    }
    match (row.status.as_str(), row.error_code.as_deref()) {
        ("failed", Some(code)) => {
            let message = History::failure_message(code);
            let first = message.split(". ").next().unwrap_or(message);
            first.trim_end_matches('.').to_owned()
        }
        ("failed", None) => "Transcription failed".to_owned(),
        _ => "No text".to_owned(),
    }
}

/// How the run ended, in words. The error code stays in the row, the log, and the export.
pub fn outcome(row: &Row) -> String {
    match row.status.as_str() {
        "cancelled" => "Canceled".to_owned(),
        "failed" => "Failed".to_owned(),
        _ => match row.insert_outcome.as_deref() {
            Some("pasted") => "Pasted".to_owned(),
            Some("copied_only") => "Copied only".to_owned(),
            _ => "Saved".to_owned(),
        },
    }
}

/// `2025-06-01 09:15` in the time zone the system is set to.
pub fn when(created_at: i64) -> String {
    let offset = hushpen_platform::local_time::utc_offset_seconds(created_at.div_euclid(1000));
    when_at(created_at, offset)
}

/// The same for a given offset from UTC, in seconds.
pub fn when_at(created_at: i64, offset_seconds: i64) -> String {
    let local = u64::try_from(created_at.saturating_add(offset_seconds * 1000)).unwrap_or(0);
    let stamp = hushpen_store::time::iso_seconds(local);
    stamp[..16].replace('T', " ")
}

/// The name the Models view gives a speech model, or the id when the catalog lacks it.
fn model_name(id: &str) -> String {
    catalog::embedded()
        .whisper(id)
        .map_or_else(|| id.to_owned(), |entry| entry.name.clone())
}

/// The name of a language code, as Home shows it.
fn language_name(code: &str) -> String {
    match language::name(code) {
        Some(name) => name.to_owned(),
        None if code == language::AUTO => language::label(code).to_owned(),
        None => code.to_owned(),
    }
}

pub fn seconds(duration_ms: i64) -> String {
    format!("{:.1} s", duration_ms as f64 / 1000.0)
}

pub fn meta(row: &Row) -> String {
    let mut parts = vec![when(row.created_at)];
    if is_import(row) {
        parts.push("Import".to_owned());
        parts.extend(row.source_name.clone());
    } else if let Some(app) = &row.target_app {
        parts.push(app.clone());
    }
    if row.duration_ms > 0 {
        parts.push(seconds(row.duration_ms));
    }
    parts.push(outcome(row));
    if row.audio_removed_at.is_some() {
        parts.push(AUDIO_REMOVED.to_owned());
    }
    parts.join(" \u{b7} ")
}

pub const AUDIO_REMOVED: &str = "Audio removed";

fn is_import(row: &Row) -> bool {
    row.kind == "import"
}

/// `m:ss`, or `h:mm:ss` past an hour, for the start and end of a segment.
fn clock(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    match total / 3600 {
        0 => format!("{}:{:02}", total / 60, total % 60),
        hours => format!("{hours}:{:02}:{:02}", total / 60 % 60, total % 60),
    }
}

/// What the detail view says in place of the player, or `None` when the audio plays.
pub fn audio_note(row: &Row, available: bool) -> Option<String> {
    if available {
        return None;
    }
    Some(if is_import(row) {
        "The audio of an imported file is not kept, so it cannot be played or reprocessed."
            .to_owned()
    } else if row.audio_removed_at.is_some() {
        format!("{AUDIO_REMOVED}. This transcript cannot be played or reprocessed.")
    } else {
        "The audio of this transcript is not available, so it cannot be played or reprocessed."
            .to_owned()
    })
}

pub fn render(history: &Entity<History>, cx: &mut App) -> impl IntoElement + use<> {
    let detail = history.read(cx).detail().cloned();
    match detail {
        Some(row) => detail_view(history, row, cx).into_any_element(),
        None => list_view(history, cx).into_any_element(),
    }
}

struct Snapshot {
    /// The drawn rows, with the index of the first, and how many rows lie above and below.
    rows: Vec<Row>,
    first: usize,
    below: usize,
    row_focus: Vec<FocusHandle>,
    query: String,
    total: i64,
    has_more: bool,
    can_undo: bool,
    confirming: bool,
    message: Option<String>,
    notice: Option<String>,
    search: Entity<InputState>,
    selecting: bool,
    /// For each drawn row, whether it is selected.
    selected: Vec<bool>,
    selection_len: usize,
    select_focus: FocusHandle,
    select_all_focus: FocusHandle,
    format_focus: [FocusHandle; 4],
}

/// Stacks the rows of the list into a group. A transcript row draws its own line above it, so
/// every row is the same height, and the spacers that stand in for rows outside the screen take
/// no line: both pass `false`.
fn list_group(palette: Palette, rows: Vec<(AnyElement, bool)>) -> AnyElement {
    let mut frame = ui::group_frame(hook::id("history", "list"), palette);
    for (index, (row, lined)) in rows.into_iter().enumerate() {
        if index > 0 && lined {
            frame = frame.child(ui::line(palette));
        }
        frame = frame.child(row);
    }
    frame.into_any_element()
}

fn list_view(history: &Entity<History>, cx: &mut App) -> impl IntoElement + use<> {
    if history.read(cx).page_due() {
        let history = history.clone();
        cx.defer(move |cx| history.update(cx, |history, cx| history.load_more(cx)));
    }
    let palette = theme::palette(cx);
    let (snap, focus) = {
        let view = history.read(cx);
        let drawn = view.window_rows();
        (
            Snapshot {
                rows: view.rows()[drawn.clone()].to_vec(),
                first: drawn.start,
                below: view.rows().len() - drawn.end,
                row_focus: view.row_focus[drawn].to_vec(),
                query: view.query().to_owned(),
                total: view.total(),
                has_more: view.has_more(),
                can_undo: view.can_undo(),
                confirming: view.confirming_clear(),
                message: view.message().map(str::to_owned),
                notice: view.notice().map(str::to_owned),
                search: view.search_input().clone(),
                selecting: view.selecting(),
                selected: view.rows()[view.window_rows()]
                    .iter()
                    .map(|row| view.is_selected(&row.id))
                    .collect(),
                selection_len: view.selection().len(),
                select_focus: view.focus.select.clone(),
                select_all_focus: view.focus.select_all.clone(),
                format_focus: view.focus.export_format.clone(),
            },
            [
                view.focus.clear.clone(),
                view.focus.clear_confirm.clone(),
                view.focus.clear_cancel.clone(),
                view.focus.undo.clone(),
                view.focus.more.clone(),
            ],
        )
    };
    let [
        clear_focus,
        confirm_focus,
        cancel_focus,
        undo_focus,
        more_focus,
    ] = focus;
    let mut rows: Vec<(AnyElement, bool)> =
        vec![(toolbar(palette, history, &snap, &clear_focus), true)];
    if snap.selecting {
        rows.push((select_bar(palette, history, &snap), true));
    }
    if let Some(notice) = snap.notice.clone() {
        rows.push((notice_row(palette, "notice", notice, Notice::Info), true));
    }
    if snap.confirming {
        rows.push((
            confirm_row(palette, history, snap.total, &confirm_focus, &cancel_focus),
            true,
        ));
    }
    if snap.can_undo {
        rows.push((undo_row(palette, history, &undo_focus), true));
    }
    if let Some(message) = snap.message.clone() {
        rows.push((notice_row(palette, "message", message, Notice::Alert), true));
    }
    if snap.rows.is_empty() {
        let searching = !snap.query.is_empty();
        rows.push((
            notice_row(
                palette,
                if searching { "no-results" } else { "empty" },
                if searching {
                    format!("No transcripts match \u{201c}{}\u{201d}.", snap.query)
                } else {
                    EMPTY_TEXT.to_owned()
                },
                Notice::Info,
            ),
            true,
        ));
    }
    if snap.first > 0 {
        rows.push((
            div()
                .flex_none()
                .h(px(snap.first as f32 * ROW_HEIGHT))
                .into_any_element(),
            false,
        ));
    }
    for (offset, ((row, focus), selected)) in snap
        .rows
        .into_iter()
        .zip(snap.row_focus)
        .zip(snap.selected)
        .enumerate()
    {
        let mode = snap.selecting.then_some(selected);
        rows.push((
            entry_row(palette, history, snap.first + offset, row, focus, mode),
            false,
        ));
    }
    if snap.below > 0 {
        rows.push((
            div()
                .flex_none()
                .h(px(snap.below as f32 * ROW_HEIGHT))
                .into_any_element(),
            false,
        ));
    }
    if snap.has_more {
        let more = {
            let history = history.clone();
            move |_: &mut Window, cx: &mut App| {
                history.update(cx, |history, cx| history.load_more(cx));
            }
        };
        rows.push((
            ui::row(palette)
                .justify_center()
                .child(ui::button(
                    palette,
                    hook::id("history", "more"),
                    "Show more",
                    Kind::Plain,
                    Some(&more_focus),
                    true,
                    more,
                ))
                .into_any_element(),
            true,
        ));
    }
    list_group(palette, rows)
}

/// A notice row of the History view, under the id `history.<name>`.
fn notice_row(palette: Palette, name: &str, text: String, notice: Notice) -> AnyElement {
    ui::notice_row(palette, hook::id("history", name), text, notice, None)
}

fn toolbar(
    palette: Palette,
    history: &Entity<History>,
    snap: &Snapshot,
    clear_focus: &FocusHandle,
) -> AnyElement {
    let clear = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal shows in the message row.
            let _ = history.update(cx, |history, cx| history.ask_clear(cx));
        }
    };
    let select = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            history.update(cx, |history, cx| {
                let on = !history.selecting();
                history.set_selecting(on, cx);
            });
        }
    };
    let can_select = snap.total > 0 || snap.selecting;
    let can_clear = snap.total > 0;
    ui::row(palette)
        .id(hook::id("history", "toolbar"))
        .test_support()
        .child(div().flex_1().min_w_0().child(ui::field(
            palette,
            hook::id("history", "search"),
            "Search transcripts",
            &snap.search,
        )))
        .child(ui::button(
            palette,
            hook::id("history", "select"),
            if snap.selecting { "Done" } else { "Select" },
            Kind::Plain,
            can_select.then_some(&snap.select_focus),
            can_select,
            select,
        ))
        .child(ui::button(
            palette,
            hook::id("history", "clear"),
            "Clear all",
            Kind::Plain,
            can_clear.then_some(clear_focus),
            can_clear,
            clear,
        ))
        .into_any_element()
}

/// The line above the list while rows are being chosen: how many, and the four formats.
fn select_bar(palette: Palette, history: &Entity<History>, snap: &Snapshot) -> AnyElement {
    let all = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            history.update(cx, |history, cx| history.toggle_select_all(cx));
        }
    };
    let count = snap.selection_len;
    let text = match count {
        0 => "Choose transcripts to export.".to_owned(),
        count => format!("{count} selected"),
    };
    let can_all = !snap.rows.is_empty();
    banner(
        palette,
        "select-bar",
        text,
        div()
            .flex_none()
            .flex()
            .gap(px(space::SM))
            .child(ui::button(
                palette,
                hook::id("history", "select-all"),
                "All",
                Kind::Plain,
                can_all.then_some(&snap.select_all_focus),
                can_all,
                all,
            ))
            .child(format_buttons(
                palette,
                history,
                &snap.format_focus,
                count > 0,
            )),
    )
}

/// One button for each export format. They write the rows that are selected, or the open one.
fn format_buttons(
    palette: Palette,
    history: &Entity<History>,
    focus: &[FocusHandle; 4],
    enabled: bool,
) -> impl IntoElement + use<> {
    let mut group = div().flex_none().flex().gap(px(space::SM));
    for (format, focus) in Format::ALL.into_iter().zip(focus) {
        let press = {
            let history = history.clone();
            move |_: &mut Window, cx: &mut App| {
                // A refusal shows in the message row.
                let _ = history.update(cx, |history, cx| history.export(format, None, cx));
            }
        };
        group = group.child(ui::button(
            palette,
            hook::id("history", &format!("export-{}", format.extension())),
            format.label(),
            Kind::Plain,
            enabled.then_some(focus),
            enabled,
            press,
        ));
    }
    group
}

fn confirm_row(
    palette: Palette,
    history: &Entity<History>,
    total: i64,
    confirm_focus: &FocusHandle,
    cancel_focus: &FocusHandle,
) -> AnyElement {
    let confirm = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            let _ = history.update(cx, |history, cx| history.confirm_clear(cx));
        }
    };
    let cancel = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            history.update(cx, |history, cx| history.cancel_clear(cx));
        }
    };
    let noun = if total == 1 {
        "transcript"
    } else {
        "transcripts"
    };
    banner(
        palette,
        "confirm",
        format!("Delete all {total} {noun} and their audio? This cannot be undone."),
        div()
            .flex_none()
            .flex()
            .gap(px(space::SM))
            .child(ui::button(
                palette,
                hook::id("history", "clear-cancel"),
                "Keep",
                Kind::Plain,
                Some(cancel_focus),
                true,
                cancel,
            ))
            .child(ui::button(
                palette,
                hook::id("history", "clear-confirm"),
                "Delete all",
                Kind::Plain,
                Some(confirm_focus),
                true,
                confirm,
            )),
    )
}

fn undo_row(palette: Palette, history: &Entity<History>, focus: &FocusHandle) -> AnyElement {
    let undo = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            let _ = history.update(cx, |history, cx| history.undo(cx));
        }
    };
    banner(
        palette,
        "undo-notice",
        "Transcript deleted.".to_owned(),
        ui::button(
            palette,
            hook::id("history", "undo"),
            "Undo",
            Kind::Plain,
            Some(focus),
            true,
            undo,
        ),
    )
}

/// A row with a sentence and the controls that answer it.
fn banner(
    palette: Palette,
    name: &'static str,
    text: String,
    controls: impl IntoElement,
) -> AnyElement {
    ui::row(palette)
        .id(hook::id("history", name))
        .test_support()
        .aria_label(text.clone())
        .child(ui::title(palette).flex_1().min_w_0().child(text))
        .child(controls)
        .into_any_element()
}

fn entry_row(
    palette: Palette,
    history: &Entity<History>,
    index: usize,
    row: Row,
    focus: FocusHandle,
    selection: Option<bool>,
) -> AnyElement {
    let id = row.id.clone();
    let selecting = selection.is_some();
    // While rows are being chosen, a press chooses the row instead of opening it.
    let open = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            let id = id.clone();
            history.update(cx, |history, cx| {
                if selecting {
                    history.toggle_selected(&id, cx);
                } else {
                    let _ = history.open(&id, cx);
                }
            });
        }
    };
    let on_click = {
        let open = open.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| open(window, cx)
    };
    let neighbours = history.clone();
    let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
        let target = match event.keystroke.key.as_str() {
            "enter" | "space" => {
                open(window, cx);
                cx.stop_propagation();
                return;
            }
            "down" => index + 1,
            "up" if index > 0 => index - 1,
            _ => return,
        };
        // The list draws only the rows near the screen, so the neighbour may not be drawn yet.
        // The handle exists anyway: scroll to the row, and the next frame draws it focused.
        if let Some(handle) = neighbours.read(cx).reveal_row(target) {
            window.focus(&handle, cx);
            window.refresh();
            cx.stop_propagation();
        }
    };
    let headline = title(&row);
    let detail = meta(&row);
    let label = match selection {
        Some(true) => format!("{headline}, selected"),
        _ => headline.clone(),
    };
    let glyph = match (selection, row.status.as_str()) {
        (Some(true), _) => IconName::SquareCheck,
        (Some(false), _) => IconName::Square,
        (None, "failed") => IconName::CircleAlert,
        (None, "cancelled") => IconName::X,
        (None, _) => IconName::Clock,
    };
    let mut element = div()
        .id(hook::indexed("history", "row", index))
        .test_support()
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(label)
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .items_center()
        .gap(px(ui::ROW_GAP))
        .px(px(size::ROW_PADDING - size::FOCUS_RING))
        .border_2()
        .border_color(transparent_black())
        .cursor_pointer()
        .hover(move |style| style.bg(palette.hover()))
        .focus_visible(move |style| style.border_color(palette.accent))
        .on_click(on_click)
        .on_key_down(on_key);
    if selection == Some(true) {
        element = element.bg(palette.selection);
    }
    let text = ui::named_lines(
        palette,
        hook::indexed("history", "name", index),
        headline,
        Some((hook::indexed("history", "meta", index), detail)),
    );
    // The line sits outside the focus ring, so it spans the group like every other line.
    div()
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .flex_col()
        .child(ui::line(palette))
        .child(element.child(ui::icon(palette, glyph)).child(text))
        .into_any_element()
}

type Press = Box<dyn Fn(&mut Window, &mut App)>;

fn detail_view(history: &Entity<History>, row: Row, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (focus, audio, reprocess, message, playback, export, segments) = {
        let view = history.read(cx);
        (
            [
                view.focus.back.clone(),
                view.focus.copy.clone(),
                view.focus.repaste.clone(),
                view.focus.undo_cleanup.clone(),
                view.focus.copy_original.clone(),
                view.focus.reprocess.clone(),
                view.focus.delete.clone(),
            ],
            view.audio_available(&row, cx),
            view.reprocess_state(&row.id, cx),
            view.message().map(str::to_owned),
            Player {
                info: view.playback_info(&row),
                play_focus: view.focus.play.clone(),
                seek_focus: view.focus.seek.clone(),
                seek_bounds: Rc::clone(&view.seek_bounds),
            },
            DetailExport {
                open: view.export_menu_open(),
                notice: view.notice().map(str::to_owned),
                button_focus: view.focus.export.clone(),
                format_focus: view.focus.export_format.clone(),
            },
            view.detail_segments().to_vec(),
        )
    };
    let [
        back_focus,
        copy_focus,
        repaste_focus,
        undo_cleanup_focus,
        copy_original_focus,
        reprocess_focus,
        delete_focus,
    ] = focus;
    let has_text = [&row.final_text, &row.rule_text, &row.raw_text]
        .into_iter()
        .flatten()
        .any(|text| !text.is_empty());
    let running = matches!(reprocess, Some(ReprocessState::Running));
    let id = row.id.clone();
    let act = |run: fn(&mut History, &str, &mut gpui_kit::Context<History>)| {
        let history = history.clone();
        let id = id.clone();
        move |_: &mut Window, cx: &mut App| {
            let id = id.clone();
            history.update(cx, |history, cx| run(history, &id, cx));
        }
    };
    let back = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| history.update(cx, |history, cx| history.close(cx))
    };
    let ai_used = row.llm_outcome.as_deref() == Some("used");
    let has_ai_text = row.llm_text.is_some();
    let button =
        |id: &str, label: &'static str, focus: &FocusHandle, enabled: bool, press: Press| {
            ui::button(
                palette,
                hook::id("history", id),
                label,
                Kind::Plain,
                enabled.then_some(focus),
                enabled,
                move |window, cx| press(window, cx),
            )
        };
    let text_controls = div()
        .flex_none()
        .flex()
        .gap(px(space::SM))
        .child(button(
            "copy",
            "Copy",
            &copy_focus,
            has_text,
            Box::new(act(|history, id, cx| {
                let _ = history.copy(Some(id), cx);
            })),
        ))
        .child(button(
            "repaste",
            "Re-paste",
            &repaste_focus,
            has_text,
            Box::new(act(|history, id, cx| {
                let _ = history.repaste(Some(id), cx);
            })),
        ))
        .children(ai_used.then(|| {
            button(
                "undo-cleanup",
                "Undo cleanup",
                &undo_cleanup_focus,
                true,
                Box::new(act(|history, id, cx| {
                    let _ = history.undo_cleanup(Some(id), cx);
                })),
            )
        }))
        .children(has_ai_text.then(|| {
            button(
                "copy-original",
                "Copy original",
                &copy_original_focus,
                true,
                Box::new(act(|history, id, cx| {
                    let _ = history.copy_original(Some(id), cx);
                })),
            )
        }));
    let file_controls = div()
        .flex_none()
        .flex()
        .gap(px(space::SM))
        .child(button(
            "reprocess",
            "Reprocess",
            &reprocess_focus,
            audio && !running,
            Box::new(act(|history, id, cx| {
                let _ = history.reprocess(Some(id), cx);
            })),
        ))
        .child(button(
            "export",
            "Export",
            &export.button_focus,
            true,
            Box::new({
                let history = history.clone();
                move |_: &mut Window, cx: &mut App| {
                    history.update(cx, |history, cx| history.toggle_export_menu(cx));
                }
            }),
        ))
        .child(button(
            "delete",
            "Delete",
            &delete_focus,
            true,
            Box::new(act(|history, id, cx| {
                let _ = history.delete(Some(id), cx);
            })),
        ));

    let mut rows: Vec<AnyElement> = vec![
        ui::row(palette)
            .id(hook::id("history", "detail-header"))
            .test_support()
            .child(ui::button(
                palette,
                hook::id("history", "back"),
                "Back",
                Kind::Plain,
                Some(&back_focus),
                true,
                back,
            ))
            .child(ui::secondary(palette).flex_1().min_w_0().child(meta(&row)))
            .into_any_element(),
    ];
    for (name, title, controls) in [
        ("actions.text", "Text", text_controls.into_any_element()),
        (
            "actions.file",
            "Recording",
            file_controls.into_any_element(),
        ),
    ] {
        rows.push(
            ui::row(palette)
                .id(hook::id("history", name))
                .test_support()
                .child(ui::title(palette).flex_1().min_w_0().child(title))
                .child(controls)
                .into_any_element(),
        );
    }
    if export.open {
        rows.push(banner(
            palette,
            "export-bar",
            "Export this transcript as".to_owned(),
            format_buttons(palette, history, &export.format_focus, has_text),
        ));
    }
    if let Some(notice) = export.notice {
        rows.push(notice_row(palette, "notice", notice, Notice::Info));
    }
    if audio {
        rows.push(player_row(palette, history, &row.id, playback));
    }
    if let Some(message) = message {
        rows.push(notice_row(palette, "message", message, Notice::Alert));
    }
    match &reprocess {
        Some(ReprocessState::Running) => rows.push(notice_row(
            palette,
            "reprocess-status",
            "Reprocessing\u{2026}".into(),
            Notice::Info,
        )),
        Some(ReprocessState::Failed(why)) => rows.push(notice_row(
            palette,
            "message",
            format!("Reprocess failed. {why}"),
            Notice::Alert,
        )),
        None => {}
    }
    if let Some(note) = audio_note(&row, audio) {
        rows.push(notice_row(palette, "audio-missing", note, Notice::Info));
    }
    let language = row
        .language_detected
        .as_deref()
        .or(row.language_requested.as_deref())
        .map(language_name);
    let status = match (&row.status[..], &row.error_code) {
        ("failed", Some(code)) => format!("Failed: {}", History::failure_message(code)),
        _ => outcome(&row),
    };
    let fields: [(&'static str, &'static str, Option<String>); 10] = [
        ("raw", "Raw text", row.raw_text.clone()),
        ("rule", "After the rules", row.rule_text.clone()),
        ("llm", "After AI cleanup", row.llm_text.clone()),
        ("final", "Final text", row.final_text.clone()),
        ("app", "App", row.target_app.clone()),
        ("duration", "Duration", Some(seconds(row.duration_ms))),
        ("model", "Model", row.model_id.as_deref().map(model_name)),
        ("language", "Language", language),
        ("status", "Result", Some(status)),
        ("when", "When", Some(when(row.created_at))),
    ];
    for (key, label, value) in fields {
        // The AI row appears only when there is AI text.
        if key == "llm" && value.is_none() {
            continue;
        }
        // An import has no target app and no rule pass to show.
        if is_import(&row) && matches!(key, "app" | "rule") && value.is_none() {
            continue;
        }
        rows.push(field(palette, key, label, value));
    }
    if is_import(&row) {
        rows.push(field(palette, "file", "File", row.source_name.clone()));
        for (index, segment) in segments.iter().enumerate() {
            let label = format!(
                "{} \u{2013} {}",
                clock(segment.start_ms),
                clock(segment.end_ms)
            );
            rows.push(field(
                palette,
                &format!("segment.{index}"),
                &label,
                Some(segment.text.clone()),
            ));
        }
    }
    ui::group(hook::id("history", "detail"), palette, rows).into_any_element()
}

/// What the export controls of the detail view need from the view.
struct DetailExport {
    open: bool,
    notice: Option<String>,
    button_focus: FocusHandle,
    format_focus: [FocusHandle; 4],
}

/// What the player row of the detail view needs from the view.
struct Player {
    info: PlaybackInfo,
    play_focus: FocusHandle,
    seek_focus: FocusHandle,
    seek_bounds: Rc<Cell<Bounds<Pixels>>>,
}

fn player_row(palette: Palette, history: &Entity<History>, id: &str, player: Player) -> AnyElement {
    let Player {
        info,
        play_focus,
        seek_focus,
        seek_bounds,
    } = player;
    let fraction = match info.duration_ms {
        0 => 0.0,
        total => (info.position_ms as f32 / total as f32).clamp(0.0, 1.0),
    };
    let toggle = {
        let history = history.clone();
        let id = id.to_owned();
        move |_: &mut Window, cx: &mut App| {
            history.update(cx, |history, cx| {
                let _ = history.toggle_playback(Some(&id), cx);
            });
        }
    };
    let seek_to = {
        let history = history.clone();
        let id = id.to_owned();
        let bounds = Rc::clone(&seek_bounds);
        move |event: &ClickEvent, _: &mut Window, cx: &mut App| {
            // A key press clicks too, and it has no place along the bar.
            if event.mouse_position().is_none() {
                return;
            }
            let track = bounds.get();
            let width = f32::from(track.size.width);
            if width <= 0.0 {
                return;
            }
            let along = f32::from(event.position().x - track.origin.x) / width;
            history.update(cx, |history, cx| {
                let _ = history.seek_fraction(Some(&id), along, cx);
            });
        }
    };
    let seek_by_key = {
        let history = history.clone();
        let id = id.to_owned();
        move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let step = match event.keystroke.key.as_str() {
                "left" => -KEY_STEP_MS,
                "right" => KEY_STEP_MS,
                _ => return,
            };
            history.update(cx, |history, cx| {
                let _ = history.seek_by(Some(&id), step, cx);
            });
            cx.stop_propagation();
        }
    };
    let time = format!(
        "{} / {}",
        playback::clock(info.position_ms),
        playback::clock(info.duration_ms)
    );
    let track = ui::track(palette, fraction);
    let mut row = ui::row(palette)
        .id(hook::id("history", "player"))
        .test_support()
        .child(ui::button(
            palette,
            hook::id("history", "play"),
            if info.playing { "Pause" } else { "Play" },
            Kind::Plain,
            Some(&play_focus),
            true,
            toggle,
        ))
        .child(
            div()
                .on_children_prepainted(move |children, _, _| {
                    if let Some(bar) = children.first() {
                        seek_bounds.set(*bar);
                    }
                })
                .id(hook::id("history", "seek"))
                .test_support()
                .role(Role::Slider)
                .aria_label("Seek")
                .track_focus(&seek_focus)
                .flex_1()
                .min_w_0()
                .h(px(size::BUTTON_HEIGHT))
                .px(px(size::FOCUS_RING))
                .flex()
                .items_center()
                .rounded(px(radius::BUTTON))
                .border_2()
                .border_color(transparent_black())
                .cursor_pointer()
                .focus_visible(move |style| style.border_color(palette.accent))
                .on_click(seek_to)
                .on_key_down(seek_by_key)
                .child(track),
        )
        .child(
            ui::value(palette)
                .id(hook::id("history", "time"))
                .test_support()
                .aria_label(time.clone())
                .text_token(BUTTON)
                .child(time),
        );
    if let Some(error) = info.error {
        row = row.child(
            ui::title(palette)
                .id(hook::id("history", "player-error"))
                .test_support()
                .role(Role::Alert)
                .aria_label(error.clone())
                .text_color(palette.label_secondary)
                .child(error),
        );
    }
    row.into_any_element()
}

/// The fact row of one field of the transcript, under `history.detail.<key>`.
fn field(palette: Palette, key: &str, label: &str, value: Option<String>) -> AnyElement {
    ui::fact_row(
        palette,
        hook::id("history", &format!("detail.{key}.row")),
        hook::id("history", &format!("detail.{key}")),
        label,
        value,
        None,
    )
}
