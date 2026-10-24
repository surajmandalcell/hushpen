//! The Transcribe side of the Files view: the drop zone, the options for the next file, and
//! the queue with a row for each file. Files dropped anywhere on the window land here.

use super::item::BUTTONS;
use super::{Import, Item, Menu, SUPPORTED, State};
use crate::files::DROP_GROUP;
use crate::history::playback::clock;
use crate::hook;
use crate::models::panel::format_size;
use crate::theme::{self, HEADING, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Context, Entity, ExternalPaths, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, px, svg,
};
use hushpen_core::export::Format;
use hushpen_core::language;
use std::path::Path;

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
        translate: view.translate(),
        translate_off: view.translate_off_reason(cx),
        choose_focus: view.choose_focus.clone(),
        clear_focus: view.clear_focus.clone(),
        language_focus: view.language_focus.clone(),
        translate_focus: view.translate_focus.clone(),
        option_focus: view.option_focus.clone(),
    }
}

pub fn render(import: &Entity<Import>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let view = snapshot(import, cx);
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(space::XL))
        .child(drop_zone(palette, import, &view));
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

/// The big target at the top: an icon, what to do, the formats, and the picker button. It
/// lights up while files are dragged anywhere over the window.
fn drop_zone(palette: Palette, import: &Entity<Import>, view: &Snapshot) -> impl IntoElement {
    let choose = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal is the picker being open already; the view says nothing more.
            let _ = import.update(cx, |import, cx| import.choose(cx));
        }
    };
    let (accent, selection) = (palette.accent, palette.selection);
    div()
        .id(hook::id("import", "drop"))
        .test_support()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(space::SM))
        .w_full()
        .h(px(size::DROP_ZONE_HEIGHT))
        .rounded(px(radius::GROUP))
        .border_1()
        .border_dashed()
        .border_color(palette.label_tertiary)
        .bg(palette.group)
        // A cursor gives the zone a hitbox; GPUI applies a drag style only to an element with one.
        .cursor_default()
        .group_drag_over::<ExternalPaths>(DROP_GROUP, move |style| {
            style.bg(selection).border_color(accent)
        })
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
        )))
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
        ui::menu_button(
            palette,
            hook::id("import", "language"),
            "Language",
            language::label(&view.language).to_owned(),
            view.language_off.is_none(),
            &view.language_focus,
            toggle_language,
        ),
    ));
    if view.menu == Some(Menu::Language) {
        rows.push(language_list(palette, import, view));
    }
    let on = view.translate && view.translate_off.is_none();
    let toggle = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal is a model that cannot translate; the switch is off then.
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

fn language_list(palette: Palette, import: &Entity<Import>, view: &Snapshot) -> AnyElement {
    let mut codes = vec![language::AUTO];
    codes.extend(language::picker_order(&[]));
    let rows = codes
        .into_iter()
        .zip(&view.option_focus)
        .map(|(code, focus)| {
            let import = import.clone();
            ui::option_row(
                palette,
                hook::id("import", &format!("language.option.{code}")),
                language::label(code).to_owned(),
                None,
                code == view.language,
                focus,
                move |_, cx| {
                    // Every code here is one the setting takes.
                    let _ = import.update(cx, |import, cx| import.set_language(code, cx));
                },
            )
        })
        .collect();
    let close = import.clone();
    ui::option_list(hook::id("import", "language.list"), rows, move |cx| {
        close.update(cx, |import, cx| import.close_menu(cx))
    })
}

/// The heading with Clear Finished at its end, and the group of files.
fn queue(palette: Palette, import: &Entity<Import>, view: Snapshot) -> impl IntoElement {
    let clear = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            import.update(cx, |import, cx| import.clear_finished(cx));
        }
    };
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
        .child(ui::button(
            palette,
            hook::id("import", "clear"),
            "Clear Finished",
            Kind::Plain,
            Some(&view.clear_focus),
            view.has_finished,
            clear,
        ));
    let mut rows = Vec::new();
    for (index, line) in view.lines.into_iter().enumerate() {
        let open = view.menu == Some(Menu::Export(line.id));
        let id = line.id;
        rows.push(item_row(palette, import, index, line));
        if open {
            rows.push(format_list(palette, import, id, &view.option_focus));
        }
    }
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS + space::XXS))
        .child(header)
        .child(ui::group(hook::id("import", "queue"), palette, rows))
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
/// buttons its state offers.
fn item_row(palette: Palette, import: &Entity<Import>, index: usize, line: Line) -> AnyElement {
    let (status, color) = status_of(palette, &line);
    let running = matches!(line.state, State::Running | State::Cancelling);
    let buttons = buttons(palette, import, index, &line);
    ui::row(palette)
        .id(hook::indexed("import", "row", index))
        .test_support()
        .child(ui::named_lines(
            palette,
            hook::indexed("import", "name", index),
            line.name,
            Some((hook::indexed("import", "detail", index), line.detail)),
        ))
        .children(running.then(|| {
            div()
                .id(hook::indexed("import", "progress", index))
                .test_support()
                .flex_none()
                .w(px(size::QUEUE_PROGRESS_WIDTH))
                .child(ui::track(palette, line.fraction))
        }))
        .child(
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
                ui::menu_button(
                    palette,
                    hook::indexed("import", "export", index),
                    "Export",
                    "Export".to_owned(),
                    true,
                    &line.focus[1],
                    toggle,
                )
                .into_any_element(),
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
fn format_list(
    palette: Palette,
    import: &Entity<Import>,
    id: u64,
    focus: &[FocusHandle],
) -> AnyElement {
    let rows = Format::ALL
        .into_iter()
        .zip(focus)
        .map(|(format, focus)| {
            let import = import.clone();
            ui::option_row(
                palette,
                hook::id("import", &format!("export.option.{}", format.extension())),
                format.label().to_owned(),
                Some(format_detail(format).to_owned()),
                false,
                focus,
                move |_, cx| {
                    // A refusal shows in the info line.
                    let _ = import.update(cx, |import, cx| import.export(id, format, cx));
                },
            )
        })
        .collect();
    let close = import.clone();
    ui::option_list(hook::id("import", "export.list"), rows, move |cx| {
        close.update(cx, |import, cx| import.close_menu(cx))
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
