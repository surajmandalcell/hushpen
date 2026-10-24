//! The detail view of one transcript: the header, the action rows, the player, and the
//! fields with its texts and facts.

use super::History;
use super::panel::{
    audio_note, banner, format_buttons, is_import, meta, notice_row, outcome, seconds, when,
};
use super::playback::{self, KEY_STEP_MS, PlaybackInfo};
use crate::controller::ReprocessState;
use crate::hook;
use crate::theme::{self, BUTTON, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AnyElement, App, Bounds, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Role, StatefulInteractiveElement, Styled, Window, div, px,
    transparent_black,
};
use hushpen_core::{catalog, language};
use hushpen_store::history::Row;
use std::cell::Cell;
use std::rc::Rc;

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

/// `m:ss`, or `h:mm:ss` past an hour, for the start and end of a segment.
fn clock(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    match total / 3600 {
        0 => format!("{}:{:02}", total / 60, total % 60),
        hours => format!("{hours}:{:02}:{:02}", total / 60 % 60, total % 60),
    }
}

type Press = Box<dyn Fn(&mut Window, &mut App)>;

pub(super) fn detail_view(
    history: &Entity<History>,
    row: Row,
    cx: &mut App,
) -> impl IntoElement + use<> {
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
