//! The History view: one grouped panel with the search box and a row for each transcript, or
//! the detail of one transcript.

use super::playback::{self, KEY_STEP_MS, PlaybackInfo};
use super::{History, ROW_HEIGHT};
use crate::controller::ReprocessState;
use crate::hook;
use crate::theme::{
    self, BODY_MD, BODY_SM, LABEL_CAPS, LABEL_MD, ROW_TITLE, StyledType, color, radius, size, space,
};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    App, Bounds, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
    relative, svg, transparent_black,
};
use hushpen_core::export::Format;
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
            format!("Failed: {}", first.trim_end_matches('.'))
        }
        ("failed", None) => "Failed".to_owned(),
        _ => "No text".to_owned(),
    }
}

/// How the run ended, in words.
pub fn outcome(row: &Row) -> String {
    match row.status.as_str() {
        "cancelled" => "Cancelled".to_owned(),
        "failed" => match &row.error_code {
            Some(code) => format!("Failed ({code})"),
            None => "Failed".to_owned(),
        },
        _ => match row.insert_outcome.as_deref() {
            Some("pasted") => "Pasted".to_owned(),
            Some("copied_only") => "Copied only".to_owned(),
            _ => "Saved".to_owned(),
        },
    }
}

pub fn when(created_at: i64) -> String {
    let stamp = hushpen_store::time::iso_seconds(u64::try_from(created_at).unwrap_or(0));
    format!("{} UTC", stamp[..16].replace('T', " "))
}

pub fn seconds(duration_ms: i64) -> String {
    format!("{:.1} s", duration_ms as f64 / 1000.0)
}

fn meta(row: &Row) -> String {
    let mut parts = vec![when(row.created_at)];
    if let Some(app) = &row.target_app {
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

/// What the detail view says in place of the player, or `None` when the audio plays.
pub fn audio_note(row: &Row, available: bool) -> Option<String> {
    if available {
        return None;
    }
    Some(if row.audio_removed_at.is_some() {
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

fn list_view(history: &Entity<History>, cx: &mut App) -> impl IntoElement + use<> {
    if history.read(cx).page_due() {
        let history = history.clone();
        cx.defer(move |cx| history.update(cx, |history, cx| history.load_more(cx)));
    }
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
    let mut panel = panel_shell("list").child(toolbar(history, &snap, &clear_focus));
    if snap.selecting {
        panel = panel.child(select_bar(history, &snap));
    }
    if let Some(notice) = snap.notice.clone() {
        panel = panel.child(note_row("notice", notice));
    }
    if snap.confirming {
        panel = panel.child(confirm_row(
            history,
            snap.total,
            &confirm_focus,
            &cancel_focus,
        ));
    }
    if snap.can_undo {
        panel = panel.child(undo_row(history, &undo_focus));
    }
    if let Some(message) = snap.message.clone() {
        panel = panel.child(message_row(message));
    }
    if snap.rows.is_empty() {
        let searching = !snap.query.is_empty();
        panel = panel.child(note_row(
            if searching { "no-results" } else { "empty" },
            if searching {
                format!("No transcripts match \u{201c}{}\u{201d}.", snap.query)
            } else {
                EMPTY_TEXT.to_owned()
            },
        ));
    }
    if snap.first > 0 {
        panel = panel.child(div().flex_none().h(px(snap.first as f32 * ROW_HEIGHT)));
    }
    for (offset, ((row, focus), selected)) in snap
        .rows
        .into_iter()
        .zip(snap.row_focus)
        .zip(snap.selected)
        .enumerate()
    {
        let mode = snap.selecting.then_some(selected);
        panel = panel.child(entry_row(history, snap.first + offset, row, focus, mode));
    }
    if snap.below > 0 {
        panel = panel.child(div().flex_none().h(px(snap.below as f32 * ROW_HEIGHT)));
    }
    if snap.has_more {
        let more = {
            let history = history.clone();
            move |_: &mut Window, cx: &mut App| {
                history.update(cx, |history, cx| history.load_more(cx));
            }
        };
        panel = panel.child(
            div()
                .flex()
                .justify_center()
                .py(px(space::MD))
                .border_t_1()
                .border_color(theme::rgb_of(color::DIVIDER))
                .child(button(
                    hook::id("history", "more"),
                    "Show more",
                    &more_focus,
                    true,
                    more,
                )),
        );
    }
    panel
}

fn panel_shell(name: &'static str) -> impl ParentElement + Styled + IntoElement {
    div()
        .id(hook::id("history", name))
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
}

fn toolbar(
    history: &Entity<History>,
    snap: &Snapshot,
    clear_focus: &FocusHandle,
) -> impl IntoElement + use<> {
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
    div()
        .id(hook::id("history", "toolbar"))
        .test_support()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .child(
            div().flex_1().min_w_0().child(
                Input::new(&snap.search)
                    .id(hook::id("history", "search"))
                    .aria_label("Search transcripts"),
            ),
        )
        .child(button(
            hook::id("history", "select"),
            if snap.selecting { "Done" } else { "Select" },
            &snap.select_focus,
            snap.total > 0 || snap.selecting,
            select,
        ))
        .child(button(
            hook::id("history", "clear"),
            "Clear all",
            clear_focus,
            snap.total > 0,
            clear,
        ))
}

/// The line above the list while rows are being chosen: how many, and the four formats.
fn select_bar(history: &Entity<History>, snap: &Snapshot) -> impl IntoElement + use<> {
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
    banner(
        "select-bar",
        text,
        div()
            .flex_none()
            .flex()
            .gap(px(space::SM))
            .child(button(
                hook::id("history", "select-all"),
                "All",
                &snap.select_all_focus,
                !snap.rows.is_empty(),
                all,
            ))
            .child(format_buttons(history, &snap.format_focus, count > 0)),
    )
}

/// One button for each export format. They write the rows that are selected, or the open one.
fn format_buttons(
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
        group = group.child(button(
            hook::id("history", &format!("export-{}", format.extension())),
            format.label(),
            focus,
            enabled,
            press,
        ));
    }
    group
}

fn confirm_row(
    history: &Entity<History>,
    total: i64,
    confirm_focus: &FocusHandle,
    cancel_focus: &FocusHandle,
) -> impl IntoElement + use<> {
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
        "confirm",
        format!("Delete all {total} {noun} and their audio? This cannot be undone."),
        div()
            .flex_none()
            .flex()
            .gap(px(space::SM))
            .child(button(
                hook::id("history", "clear-confirm"),
                "Delete all",
                confirm_focus,
                true,
                confirm,
            ))
            .child(button(
                hook::id("history", "clear-cancel"),
                "Keep",
                cancel_focus,
                true,
                cancel,
            )),
    )
}

fn undo_row(history: &Entity<History>, focus: &FocusHandle) -> impl IntoElement + use<> {
    let undo = {
        let history = history.clone();
        move |_: &mut Window, cx: &mut App| {
            let _ = history.update(cx, |history, cx| history.undo(cx));
        }
    };
    banner(
        "undo-notice",
        "Transcript deleted.".to_owned(),
        button(hook::id("history", "undo"), "Undo", focus, true, undo),
    )
}

fn banner(name: &'static str, text: String, controls: impl IntoElement) -> impl IntoElement {
    div()
        .id(hook::id("history", name))
        .test_support()
        .aria_label(text.clone())
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::TEXT_BODY))
        .child(div().flex_1().min_w_0().child(text))
        .child(controls)
}

fn message_row(message: String) -> impl IntoElement {
    div()
        .id(hook::id("history", "message"))
        .test_support()
        .role(Role::Alert)
        .aria_label(message.clone())
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
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

fn note_row(name: &'static str, text: String) -> impl IntoElement {
    div()
        .id(hook::id("history", name))
        .test_support()
        .aria_label(text.clone())
        .px(px(space::LG))
        .py(px(space::LG))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::TEXT_MUTED))
        .child(text)
}

fn entry_row(
    history: &Entity<History>,
    index: usize,
    row: Row,
    focus: FocusHandle,
    selection: Option<bool>,
) -> impl IntoElement + use<> {
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
    let failed = row.status == "failed";
    let label = match selection {
        Some(true) => format!("{headline}, selected"),
        _ => headline.clone(),
    };
    div()
        .id(hook::indexed("history", "row", index))
        .test_support()
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(label)
        .h(px(ROW_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG - size::FOCUS_RING))
        .py(px(space::MD - size::FOCUS_RING))
        .border_2()
        .border_color(transparent_black())
        .border_t_1()
        .cursor_pointer()
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(icon_tile(match (selection, row.status.as_str()) {
            (Some(true), _) => IconName::SquareCheck,
            (Some(false), _) => IconName::Square,
            (None, "failed") => IconName::CircleAlert,
            (None, "cancelled") => IconName::X,
            (None, _) => IconName::Clock,
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(hook::indexed("history", "name", index))
                        .test_support()
                        .aria_label(headline.clone())
                        .text_token(ROW_TITLE)
                        .text_color(theme::rgb_of(if failed {
                            color::STATUS_ERROR
                        } else {
                            color::TEXT_PRIMARY
                        }))
                        .truncate()
                        .child(headline),
                )
                .child(
                    div()
                        .id(hook::indexed("history", "meta", index))
                        .test_support()
                        .aria_label(detail.clone())
                        .w_full()
                        .min_w_0()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .truncate()
                        .child(detail),
                ),
        )
}

fn detail_view(history: &Entity<History>, row: Row, cx: &mut App) -> impl IntoElement + use<> {
    let (focus, audio, reprocess, message, playback, export) = {
        let view = history.read(cx);
        (
            [
                view.focus.back.clone(),
                view.focus.copy.clone(),
                view.focus.repaste.clone(),
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
        )
    };
    let [
        back_focus,
        copy_focus,
        repaste_focus,
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
    let controls = div()
        .flex()
        .flex_wrap()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(button(
            hook::id("history", "copy"),
            "Copy",
            &copy_focus,
            has_text,
            act(|history, id, cx| {
                let _ = history.copy(Some(id), cx);
            }),
        ))
        .child(button(
            hook::id("history", "repaste"),
            "Re-paste",
            &repaste_focus,
            has_text,
            act(|history, id, cx| {
                let _ = history.repaste(Some(id), cx);
            }),
        ))
        .child(button(
            hook::id("history", "reprocess"),
            "Reprocess",
            &reprocess_focus,
            audio && !running,
            act(|history, id, cx| {
                let _ = history.reprocess(Some(id), cx);
            }),
        ))
        .child(button(
            hook::id("history", "export"),
            "Export",
            &export.button_focus,
            true,
            {
                let history = history.clone();
                move |_: &mut Window, cx: &mut App| {
                    history.update(cx, |history, cx| history.toggle_export_menu(cx));
                }
            },
        ))
        .child(button(
            hook::id("history", "delete"),
            "Delete",
            &delete_focus,
            true,
            act(|history, id, cx| {
                let _ = history.delete(Some(id), cx);
            }),
        ));

    let mut panel = panel_shell("detail").child(
        div()
            .flex()
            .items_center()
            .gap(px(space::MD))
            .px(px(space::LG))
            .py(px(space::MD))
            .child(button(
                hook::id("history", "back"),
                "Back",
                &back_focus,
                true,
                back,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_token(BODY_SM)
                    .text_color(theme::rgb_of(color::TEXT_MUTED))
                    .truncate()
                    .child(meta(&row)),
            ),
    );
    panel = panel.child(controls);
    if export.open {
        panel = panel.child(banner(
            "export-bar",
            "Export this transcript as".to_owned(),
            format_buttons(history, &export.format_focus, has_text),
        ));
    }
    if let Some(notice) = export.notice {
        panel = panel.child(note_row("notice", notice));
    }
    if audio {
        panel = panel.child(player_row(history, &row.id, playback));
    }
    if let Some(message) = message {
        panel = panel.child(message_row(message));
    }
    match &reprocess {
        Some(ReprocessState::Running) => {
            panel = panel.child(note_row("reprocess-status", "Reprocessing\u{2026}".into()));
        }
        Some(ReprocessState::Failed(why)) => {
            panel = panel.child(message_row(format!("Reprocess failed. {why}")));
        }
        None => {}
    }
    if let Some(note) = audio_note(&row, audio) {
        panel = panel.child(note_row("audio-missing", note));
    }
    let language = row
        .language_detected
        .clone()
        .or_else(|| row.language_requested.clone());
    let status = match (&row.status[..], &row.error_code) {
        ("failed", Some(code)) => format!("Failed ({code}): {}", History::failure_message(code)),
        _ => outcome(&row),
    };
    let fields: [(&'static str, &'static str, Option<String>); 10] = [
        ("raw", "Raw text", row.raw_text.clone()),
        ("rule", "After the rules", row.rule_text.clone()),
        ("llm", "After AI cleanup", row.llm_text.clone()),
        ("final", "Final text", row.final_text.clone()),
        ("app", "App", row.target_app.clone()),
        ("duration", "Duration", Some(seconds(row.duration_ms))),
        ("model", "Model", row.model_id.clone()),
        ("language", "Language", language),
        ("status", "Result", Some(status)),
        ("when", "When", Some(when(row.created_at))),
    ];
    for (key, label, value) in fields {
        // The AI row appears only when there is AI text.
        if key == "llm" && value.is_none() {
            continue;
        }
        panel = panel.child(field(key, label, value));
    }
    panel
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

fn player_row(history: &Entity<History>, id: &str, player: Player) -> impl IntoElement + use<> {
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
    let track = div()
        .relative()
        .w_full()
        .h(px(6.0))
        .rounded(px(radius::FULL))
        .bg(theme::rgb_of(color::SURFACE_ELEVATED))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .w(relative(fraction))
                .rounded(px(radius::FULL))
                .bg(theme::rgb_of(color::ACCENT_BLUE)),
        );
    let mut row = div()
        .flex()
        .flex_col()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .child(button(
                    hook::id("history", "play"),
                    if info.playing { "Pause" } else { "Play" },
                    &play_focus,
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
                        .h(px(28.0))
                        .px(px(size::FOCUS_RING))
                        .flex()
                        .items_center()
                        .rounded(px(radius::SM))
                        .border_2()
                        .border_color(transparent_black())
                        .cursor_pointer()
                        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
                        .on_click(seek_to)
                        .on_key_down(seek_by_key)
                        .child(track),
                )
                .child(
                    div()
                        .id(hook::id("history", "time"))
                        .test_support()
                        .aria_label(time.clone())
                        .flex_none()
                        .text_token(LABEL_MD)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .child(time),
                ),
        );
    if let Some(error) = info.error {
        row = row.child(
            div()
                .id(hook::id("history", "player-error"))
                .test_support()
                .role(Role::Alert)
                .aria_label(error.clone())
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(color::STATUS_ERROR))
                .child(error),
        );
    }
    row
}

fn field(key: &'static str, label: &'static str, value: Option<String>) -> impl IntoElement {
    let shown = value.clone().unwrap_or_else(|| "Not available".to_owned());
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(
            div()
                .text_token(LABEL_CAPS)
                .text_color(theme::rgb_of(color::TEXT_MUTED))
                .child(label.to_uppercase()),
        )
        .child(
            div()
                .id(hook::id("history", &format!("detail.{key}")))
                .test_support()
                .aria_label(value.clone().unwrap_or_default())
                .min_w_0()
                .text_token(BODY_MD)
                .text_color(theme::rgb_of(if value.is_some() {
                    color::TEXT_PRIMARY
                } else {
                    color::TEXT_SUBTLE
                }))
                .child(shown),
        )
}

fn button<F: Fn(&mut Window, &mut App) + Clone + 'static>(
    id: SharedString,
    label: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    press: F,
) -> impl IntoElement + use<F> {
    hook::mark_disabled(&id, !enabled);
    let mut element = div()
        .id(id)
        .test_support()
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
        .text_color(theme::rgb_of(if enabled {
            color::TEXT_PRIMARY
        } else {
            color::TEXT_DISABLED
        }));
    if enabled {
        let on_click = {
            let press = press.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| press(window, cx)
        };
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                press(window, cx);
                cx.stop_propagation();
            }
        };
        element = element
            .track_focus(focus)
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key);
    }
    element.child(label)
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
