//! The Transcribe side of the Manual view: the drop zone, the options for the next file, and
//! the queue with a row for each file. Files dropped anywhere on the window land here.

use super::item::BUTTONS;
use super::{Import, Item, Menu, SUPPORTED, State};
use crate::files::DROP_GROUP;
use crate::files::extras::{self, QuickLook};
use crate::history::playback::clock;
use crate::hook;
use crate::models::panel::format_size;
use crate::theme::{self, BODY, HEADING, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::progress::ProgressCircle;
use gpui_kit::component::{Sizable as _, Size};
use gpui_kit::gpui::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, ExternalDragPayload, ExternalPaths,
    FileDragPaths, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, Role,
    StatefulInteractiveElement, Styled, Transformation, Window, div, px, radians, svg,
};
use hushpen_core::export::Format;
use hushpen_core::language;
use std::path::{Path, PathBuf};

const SNIPPET_CHARS: usize = 120;

/// What a row button does to its item.
type Action = fn(&mut Import, u64, &mut Context<Import>) -> Result<(), String>;

/// What one row draws, read from the item before the view borrows the app again.
struct Line {
    id: u64,
    name: String,
    state: State,
    fraction: f32,
    percent: u32,
    detail: String,
    focus: [FocusHandle; BUTTONS],
}

/// The parts of the view that read the queue, taken in one borrow.
struct Snapshot {
    lines: Vec<Line>,
    notices: Vec<(&'static str, String, Notice)>,
    blocked: bool,
    has_finished: bool,
    choosing: bool,
    menu: Option<Menu>,
    model: String,
    language: String,
    language_off: Option<&'static str>,
    translate: bool,
    translate_off: Option<&'static str>,
    choose_focus: FocusHandle,
    clear_focus: FocusHandle,
    language_focus: FocusHandle,
    translate_focus: FocusHandle,
    option_focus: Vec<FocusHandle>,
    dragging: usize,
    selected: Option<u64>,
    queue_focus: FocusHandle,
}

fn snapshot(import: &Entity<Import>, cx: &App) -> Snapshot {
    let view = import.read(cx);
    let home = std::env::home_dir();
    let now = view.now_ms();
    let lines = view
        .items()
        .iter()
        .map(|item| Line {
            id: item.id,
            name: item.name.clone(),
            state: item.state,
            fraction: item.progress.fraction(),
            percent: item.progress.percent(),
            detail: line_of(item, home.as_deref(), now),
            focus: item.focus.clone(),
        })
        .collect();
    let blocker = view.blocker(cx);
    let notices = [
        blocker.clone().map(|text| ("blocker", text, Notice::Alert)),
        view.notice()
            .map(|text| ("notice", text.to_owned(), Notice::Alert)),
        view.info()
            .map(|text| ("info", text.to_owned(), Notice::Info)),
        view.language_notice(cx)
            .map(|text| ("language-notice", text.to_owned(), Notice::Info)),
    ]
    .into_iter()
    .flatten()
    .collect();
    Snapshot {
        lines,
        notices,
        blocked: blocker.is_some(),
        has_finished: view.has_finished(),
        choosing: view.choosing(),
        menu: view.menu(),
        model: view.model_name(cx),
        language: view.language(),
        language_off: view.language_off_reason(cx),
        translate: view.translate(cx),
        translate_off: view.translate_off_reason(cx),
        choose_focus: view.choose_focus.clone(),
        clear_focus: view.clear_focus.clone(),
        language_focus: view.language_focus.clone(),
        translate_focus: view.translate_focus.clone(),
        option_focus: view.option_focus.clone(),
        dragging: view.dragging(),
        selected: view.selected(),
        queue_focus: view.queue_focus.clone(),
    }
}

pub fn render(import: &Entity<Import>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let view = snapshot(import, cx);
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(space::XL))
        .child(drop_zone(palette, import, &view, cx));
    if !view.notices.is_empty() {
        let rows = view
            .notices
            .iter()
            .map(|(name, text, notice)| {
                ui::notice_row(
                    palette,
                    hook::id("import", name),
                    text.clone(),
                    *notice,
                    None,
                )
            })
            .collect();
        page = page.child(ui::group(hook::id("import", "notices"), palette, rows));
    }
    page = page.child(ui::titled_section(
        palette,
        hook::id("import", "options.heading"),
        "Options",
        ui::group(
            hook::id("import", "options"),
            palette,
            option_rows(palette, import, &view),
        ),
        Some((
            hook::id("import", "footer"),
            "Each transcript is saved to History.".to_owned(),
        )),
    ));
    if !view.lines.is_empty() {
        page = page.child(queue(palette, import, view));
    }
    page
}

/// The big card at the top. At rest: an icon, what to do, the formats, and the picker button.
/// While files are dragged over the window: a fan of file icons, the count, and "Release to
/// transcribe", in an accent frame.
fn drop_zone(
    palette: Palette,
    import: &Entity<Import>,
    view: &Snapshot,
    cx: &App,
) -> impl IntoElement {
    let choose = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal is the picker being open already; the view says nothing more.
            let _ = import.update(cx, |import, cx| import.choose(cx));
        }
    };
    let counted = import.clone();
    // The count is kept from the last move; it shows only while a drag is still on.
    let dragging = (cx.has_active_drag() && view.dragging > 0).then_some(view.dragging);
    let height = if view.lines.is_empty() {
        size::DROP_ZONE_EMPTY_HEIGHT
    } else {
        size::DROP_ZONE_HEIGHT
    };
    let (accent, selection) = (palette.accent, palette.selection);
    let card = div()
        .id(hook::id("import", "drop"))
        .test_support()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(space::SM))
        .w_full()
        .h(px(height))
        .rounded(px(radius::GROUP))
        .bg(palette.group)
        // A cursor gives the card a hitbox; GPUI applies a drag style only to an element with one.
        .cursor_default()
        .on_drag_move::<ExternalPaths>(move |event, _, cx| {
            let count = event.drag(cx).paths().len();
            counted.update(cx, |import, cx| import.drag_moved(count, cx));
        })
        .group_drag_over::<ExternalPaths>(DROP_GROUP, move |style| {
            style.bg(selection).border_color(accent)
        });
    let Some(count) = dragging else {
        return card
            .border_1()
            .border_dashed()
            .border_color(palette.label_tertiary)
            .child(
                svg()
                    .path(IconName::AudioLines.path())
                    .size(px(size::DROP_ICON))
                    .text_color(palette.label_secondary),
            )
            .child(
                div()
                    .id(hook::id("import", "drop.title"))
                    .test_support()
                    .aria_label("Drop audio files here")
                    .text_token(HEADING)
                    .text_color(palette.label)
                    .child("Drop audio files here"),
            )
            .child(
                ui::secondary(palette)
                    .id(hook::id("import", "drop.formats"))
                    .test_support()
                    .child(format!("{SUPPORTED}, up to 12 hours each")),
            )
            .child(div().mt(px(space::XS)).child(ui::button(
                palette,
                hook::id("import", "choose"),
                "Choose Files\u{2026}",
                Kind::Default,
                Some(&view.choose_focus),
                !view.blocked && !view.choosing,
                choose,
            )));
    };
    let label = match count {
        1 => "1 file".to_owned(),
        n => format!("{n} files"),
    };
    card.border_2()
        .border_color(accent)
        .bg(selection)
        .child(stack(palette, count))
        .child(
            div()
                .id(hook::id("import", "drop.count"))
                .test_support()
                .aria_label(label.clone())
                .text_token(HEADING)
                .text_color(palette.label)
                .child(label),
        )
        .child(ui::secondary(palette).child("Release to transcribe"))
}

/// Up to three file icons, fanned out like cards in a hand.
fn stack(palette: Palette, count: usize) -> impl IntoElement {
    let angles: &[f32] = match count {
        1 => &[0.0],
        2 => &[-0.5, 0.5],
        _ => &[-1.0, 0.0, 1.0],
    };
    div()
        .flex()
        .items_end()
        .children(angles.iter().enumerate().map(|(index, side)| {
            let turn = radians((side * size::DROP_STACK_DEGREES).to_radians());
            let icon = svg()
                .path(IconName::File.path())
                .size(px(size::DROP_STACK_ICON))
                .text_color(palette.accent)
                .with_transformation(Transformation::rotate(turn));
            match index {
                0 => icon,
                _ => icon.ml(px(-size::DROP_STACK_OVERLAP)),
            }
        }))
}

/// Model, Language, and Translate to English, for the files that start next.
fn option_rows(palette: Palette, import: &Entity<Import>, view: &Snapshot) -> Vec<AnyElement> {
    let mut rows = vec![ui::labeled_row(
        palette,
        hook::id("import", "model"),
        "Model",
        Some("Change it in Models".to_owned()),
        ui::value(palette)
            .id(hook::id("import", "model.value"))
            .test_support()
            .aria_label(view.model.clone())
            .child(view.model.clone()),
    )];
    let toggle_language = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            import.update(cx, |import, cx| import.toggle_menu(Menu::Language, cx));
        }
    };
    rows.push(ui::labeled_row(
        palette,
        hook::id("import", "language.row"),
        "Language",
        Some(
            view.language_off
                .unwrap_or("The language spoken in the files")
                .to_owned(),
        ),
        ui::pop_up(
            palette,
            ui::PopUp {
                id: hook::id("import", "language"),
                label: "Language",
                current: language::label(&view.language).to_owned(),
                enabled: view.language_off.is_none(),
                focus: &view.language_focus,
                menu: (view.menu == Some(Menu::Language)).then(|| language_menu(import, view)),
            },
            toggle_language,
        ),
    ));
    let on = view.translate;
    let toggle = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            let _ = import.update(cx, |import, cx| import.set_translate(!on, cx));
        }
    };
    let switch = ui::switch(
        palette,
        hook::id("import", "translate"),
        "Translate to English",
        on,
        view.translate_off.is_none(),
        &view.translate_focus,
        toggle,
    );
    let switch = match view.translate_off {
        Some(why) => ui::tooltip(div().id(hook::id("import", "translate.why")), why)
            .child(switch)
            .into_any_element(),
        None => switch.into_any_element(),
    };
    rows.push(ui::labeled_row(
        palette,
        hook::id("import", "translate.row"),
        "Translate to English",
        Some("Writes English text from any spoken language".to_owned()),
        switch,
    ));
    rows
}

fn language_menu(import: &Entity<Import>, view: &Snapshot) -> ui::Menu {
    let mut codes = vec![language::AUTO];
    codes.extend(language::picker_order(&[]));
    let close = import.clone();
    let menu = ui::Menu::new(hook::id("import", "language.list"), move |cx| {
        close.update(cx, |import, cx| import.close_menu(cx))
    });
    codes
        .into_iter()
        .zip(&view.option_focus)
        .fold(menu, |menu, (code, focus)| {
            let import = import.clone();
            menu.option(
                hook::id("import", &format!("language.option.{code}")),
                language::label(code),
                None,
                code == view.language,
                focus,
                move |_, cx| {
                    // Every code here is one the setting takes.
                    let _ = import.update(cx, |import, cx| import.set_language(code, cx));
                },
            )
        })
}

/// The heading with Copy All and Clear Finished at its end, and the group of files. A click
/// selects a row; Space then shows it in Quick Look.
fn queue(palette: Palette, import: &Entity<Import>, view: Snapshot) -> impl IntoElement {
    let clear = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            import.update(cx, |import, cx| import.clear_finished(cx));
        }
    };
    let copy_all = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            import.update(cx, |import, cx| import.copy_all(cx));
        }
    };
    let has_done = view.lines.iter().any(|line| line.state == State::Done);
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .pr(px(size::ROW_PADDING))
        .child(ui::heading(
            palette,
            hook::id("import", "queue.heading"),
            "Queue",
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::XS))
                .child(ui::button(
                    palette,
                    hook::id("import", "copy-all"),
                    "Copy All",
                    Kind::Plain,
                    None,
                    has_done,
                    copy_all,
                ))
                .child(ui::button(
                    palette,
                    hook::id("import", "clear"),
                    "Clear Finished",
                    Kind::Plain,
                    Some(&view.clear_focus),
                    view.has_finished,
                    clear,
                )),
        );
    let mut rows = Vec::new();
    for (index, line) in view.lines.into_iter().enumerate() {
        let menu = (view.menu == Some(Menu::Export(line.id)))
            .then(|| format_menu(import, line.id, &view.option_focus));
        let selected = view.selected == Some(line.id);
        rows.push(item_row(
            palette,
            import,
            index,
            line,
            menu,
            selected,
            &view.queue_focus,
        ));
    }
    let look = import.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS + space::XXS))
        .child(header)
        .child(
            div()
                .id(hook::id("import", "queue.keys"))
                .key_context(extras::QUEUE_CONTEXT)
                .track_focus(&view.queue_focus)
                .role(Role::ListBox)
                .aria_label("Files to transcribe")
                .on_action(move |_: &QuickLook, _, cx| {
                    look_at_selected(&look, cx);
                })
                .child(ui::group(hook::id("import", "queue"), palette, rows)),
        )
}

/// Space on the queue: Quick Look on the selected row. Returns the file it shows.
pub fn look_at_selected(import: &Entity<Import>, cx: &mut App) -> Option<PathBuf> {
    let path = import.read(cx).preview_path()?;
    let shown = path.clone();
    // The panel takes key, and AppKit tells the window at once: not in this update.
    cx.spawn(async move |_| extras::quick_look(shown)).detach();
    Some(path)
}

/// The `.txt` file a drag out of a finished row hands to Finder or another app.
pub fn drag_out_file(import: &Entity<Import>, id: u64, cx: &App) -> Option<PathBuf> {
    import
        .read(cx)
        .result_file(id)
        .inspect_err(|error| log::warn!("the transcript was not written for a drag: {error}"))
        .ok()
}

/// A finished row while it is dragged: its name in a small chip. Out of the window, the drag
/// becomes the transcript as a `.txt` file.
#[derive(Clone)]
struct DragOut {
    id: u64,
    name: String,
    palette: Palette,
}

impl Render for DragOut {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(space::XS))
            .px(px(space::SM))
            .py(px(space::XS))
            .rounded(px(radius::GROUP))
            .bg(self.palette.group)
            .border_1()
            .border_color(self.palette.separator)
            .text_token(BODY)
            .text_color(self.palette.label)
            .child(
                svg()
                    .path(IconName::File.path())
                    .size(px(size::DROP_ICON / 2.0))
                    .text_color(self.palette.label_secondary),
            )
            .child(self.name.clone())
    }
}

/// The second line of a file: its size and length, then why it failed (the code stays in the
/// log), its words when it is done, the time left while it runs, or the folder it came from.
fn line_of(item: &Item, home: Option<&Path>, now_ms: u64) -> String {
    let mut parts = vec![format_size(item.size_bytes)];
    parts.extend(item.duration_ms.map(clock));
    parts.push(detail_of(item, home, now_ms));
    parts.retain(|part| !part.is_empty());
    parts.join(" \u{b7} ")
}

pub(super) fn detail_of(item: &Item, home: Option<&Path>, now_ms: u64) -> String {
    match item.state {
        State::Failed => item
            .failure
            .as_ref()
            .map(|(_, message)| message.clone())
            .unwrap_or_default(),
        State::Done => snippet(item.text.as_deref().unwrap_or("")),
        State::Running if let Some(left) = item.time_left_ms(now_ms) => left_text(left),
        _ => item
            .path
            .parent()
            .map(|folder| hushpen_core::paths::display(folder, home))
            .unwrap_or_default(),
    }
}

fn left_text(ms: u64) -> String {
    if ms < 60_000 {
        return "Less than a minute left".to_owned();
    }
    let minutes = ms.div_ceil(60_000);
    if minutes < 60 {
        format!("About {minutes} min left")
    } else {
        format!("About {} h {} min left", minutes / 60, minutes % 60)
    }
}

fn snippet(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(SNIPPET_CHARS) {
        Some((end, _)) => format!("{}\u{2026}", &line[..end]),
        None => line,
    }
}

fn status_of(palette: Palette, line: &Line) -> (String, gpui_kit::Rgba) {
    match line.state {
        State::Queued => ("Waiting".to_owned(), palette.label_secondary),
        State::Running => (format!("{}%", line.percent), palette.label),
        State::Cancelling => ("Canceling".to_owned(), palette.label_secondary),
        State::Done => ("Done".to_owned(), palette.success),
        State::Failed => ("Failed".to_owned(), palette.error),
        State::Cancelled => ("Canceled".to_owned(), palette.label_secondary),
    }
}

/// One file: its name and second line, the progress bar while it runs, the status, and the
/// buttons its state offers, with `menu` under Export while it is open.
fn item_row(
    palette: Palette,
    import: &Entity<Import>,
    index: usize,
    line: Line,
    menu: Option<ui::Menu>,
    selected: bool,
    queue_focus: &FocusHandle,
) -> AnyElement {
    let (status, color) = status_of(palette, &line);
    let running = matches!(line.state, State::Running | State::Cancelling);
    let buttons = buttons(palette, import, index, &line, menu);
    let (id, done) = (line.id, line.state == State::Done);
    let select = {
        let (import, focus) = (import.clone(), queue_focus.clone());
        move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
            import.update(cx, |import, cx| import.select(id, cx));
            window.focus(&focus, cx);
        }
    };
    let row = ui::row(palette)
        .id(hook::indexed("import", "row", index))
        .test_support()
        .when(selected, |row| row.bg(palette.selection))
        .on_click(select)
        .children(running.then(|| {
            div()
                .id(hook::indexed("import", "progress", index))
                .test_support()
                .flex_none()
                .child(
                    ProgressCircle::new(hook::indexed("import", "ring", index))
                        .value(line.fraction * 100.0)
                        .color(palette.accent)
                        .with_size(Size::Size(px(size::QUEUE_RING))),
                )
        }))
        .child(ui::named_lines(
            palette,
            hook::indexed("import", "name", index),
            line.name.clone(),
            Some((hook::indexed("import", "detail", index), line.detail)),
        ));
    let row = if done {
        let chip = DragOut {
            id,
            name: line.name,
            palette,
        };
        let import = import.clone();
        row.on_drag(chip, |chip, _, _, cx| cx.new(|_| chip.clone()))
            .external_drag_payload(move |chip: &DragOut, _, cx| {
                drag_out_file(&import, chip.id, cx)
                    .map(|path| ExternalDragPayload::Files(FileDragPaths::new([(path, false)])))
            })
    } else {
        row
    };
    row.child(
        ui::value(palette)
            .id(hook::indexed("import", "status", index))
            .test_support()
            .aria_label(status.clone())
            .w(px(size::QUEUE_STATUS_WIDTH))
            .text_right()
            .text_color(color)
            .child(status),
    )
    .children(buttons)
    .into_any_element()
}

/// Cancel while a file waits or runs; Copy, Export, Show in History, and Remove once it is
/// done; Retry and Remove after a failure; Remove after a cancel.
fn buttons(
    palette: Palette,
    import: &Entity<Import>,
    index: usize,
    line: &Line,
    menu: Option<ui::Menu>,
) -> Vec<AnyElement> {
    let id = line.id;
    let button = |slot: usize, name: &str, label: &'static str, enabled: bool, run: Action| {
        let import = import.clone();
        let press = move |_: &mut Window, cx: &mut App| {
            // A refusal means the item already moved on; the row repaints either way.
            let _ = import.update(cx, |import, cx| run(import, id, cx));
        };
        ui::button(
            palette,
            hook::indexed("import", name, index),
            label,
            Kind::Plain,
            Some(&line.focus[slot]),
            enabled,
            press,
        )
        .into_any_element()
    };
    match line.state {
        State::Queued | State::Running | State::Cancelling => vec![button(
            0,
            "cancel",
            "Cancel",
            line.state != State::Cancelling,
            Import::cancel,
        )],
        State::Done => {
            let toggle = {
                let import = import.clone();
                move |_: &mut Window, cx: &mut App| {
                    import.update(cx, |import, cx| import.toggle_menu(Menu::Export(id), cx));
                }
            };
            vec![
                button(0, "copy", "Copy", true, Import::copy),
                ui::pop_up(
                    palette,
                    ui::PopUp {
                        id: hook::indexed("import", "export", index),
                        label: "Export",
                        current: "Export".to_owned(),
                        enabled: true,
                        focus: &line.focus[1],
                        menu,
                    },
                    toggle,
                ),
                button(2, "show", "Show in History", true, Import::show_in_history),
                button(3, "remove", "Remove", true, Import::remove),
            ]
        }
        State::Failed => vec![
            button(0, "retry", "Retry", true, Import::retry),
            button(3, "remove", "Remove", true, Import::remove),
        ],
        State::Cancelled => vec![button(3, "remove", "Remove", true, Import::remove)],
    }
}

/// The formats under a row's Export button.
fn format_menu(import: &Entity<Import>, id: u64, focus: &[FocusHandle]) -> ui::Menu {
    let close = import.clone();
    let menu = ui::Menu::new(hook::id("import", "export.list"), move |cx| {
        close.update(cx, |import, cx| import.close_menu(cx))
    });
    Format::ALL
        .into_iter()
        .zip(focus)
        .fold(menu, |menu, (format, focus)| {
            let import = import.clone();
            menu.option(
                hook::id("import", &format!("export.option.{}", format.extension())),
                format.label(),
                Some(format_detail(format).to_owned()),
                false,
                focus,
                move |_, cx| {
                    // A refusal shows in the info line.
                    let _ = import.update(cx, |import, cx| import.export(id, format, cx));
                },
            )
        })
}

fn format_detail(format: Format) -> &'static str {
    match format {
        Format::Txt => "Plain text",
        Format::Srt => "Subtitles with times",
        Format::Vtt => "Web subtitles with times",
        Format::Json => "Every detail, for other apps",
    }
}

#[cfg(test)]
mod tests {
    use super::left_text;

    #[test]
    fn the_time_left_reads_in_minutes_then_hours() {
        assert_eq!(left_text(20_000), "Less than a minute left");
        assert_eq!(left_text(61_000), "About 2 min left");
        assert_eq!(left_text(59 * 60_000), "About 59 min left");
        assert_eq!(left_text(125 * 60_000), "About 2 h 5 min left");
    }
}
