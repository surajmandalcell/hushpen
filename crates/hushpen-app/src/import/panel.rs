//! The Import view: one group to add files, and one group with a line for each file in the
//! queue. Files can also be dropped anywhere on the view.

use super::{Import, SUPPORTED, State};
use crate::hook;
use crate::theme::{self, Palette, radius, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AnyElement, App, Entity, ExternalPaths, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, px,
};

const EMPTY_TEXT: &str = "No files yet. Choose audio files or drop them here.";
const SNIPPET_CHARS: usize = 120;

struct Line {
    id: u64,
    name: String,
    state: State,
    percent: u32,
    detail: String,
    focus: FocusHandle,
}

pub fn render(import: &Entity<Import>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (lines, notice, blocker, has_finished, choosing, focus) = {
        let view = import.read(cx);
        let home = std::env::home_dir();
        let lines: Vec<Line> = view
            .items()
            .iter()
            .map(|item| Line {
                id: item.id,
                name: item.name.clone(),
                state: item.state,
                percent: item.progress.percent(),
                detail: detail_of(item, home.as_deref()),
                focus: item.focus.clone(),
            })
            .collect();
        (
            lines,
            view.notice().map(str::to_owned),
            view.blocker(cx),
            view.has_finished(),
            view.choosing(),
            (view.choose_focus.clone(), view.clear_focus.clone()),
        )
    };

    let choose = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal is the picker being open already; the view says nothing more.
            let _ = import.update(cx, |import, cx| import.choose(cx));
        }
    };
    let clear = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            import.update(cx, |import, cx| import.clear_finished(cx));
        }
    };
    let controls = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(ui::button(
            palette,
            hook::id("import", "clear"),
            "Clear finished",
            Kind::Plain,
            Some(&focus.1),
            has_finished,
            clear,
        ))
        .child(ui::button(
            palette,
            hook::id("import", "choose"),
            "Choose files\u{2026}",
            Kind::Default,
            Some(&focus.0),
            blocker.is_none() && !choosing,
            choose,
        ));
    let mut add_rows: Vec<AnyElement> = vec![ui::labeled_row(
        palette,
        hook::id("import", "add"),
        "Audio files",
        Some("Choose files or drop them here".to_owned()),
        controls,
    )];
    if let Some(why) = blocker {
        add_rows.push(ui::note_row(
            palette,
            hook::id("import", "blocker"),
            why,
            true,
        ));
    }
    if let Some(notice) = notice {
        add_rows.push(ui::note_row(
            palette,
            hook::id("import", "notice"),
            notice,
            true,
        ));
    }
    let footer = (
        hook::id("import", "footer"),
        format!("Reads {SUPPORTED} files. Each result is saved to History."),
    );
    let mut view = div()
        .flex()
        .flex_col()
        .gap(px(space::XL))
        .child(ui::section(
            palette,
            ui::plain_group(hook::id("import", "files"), palette, add_rows),
            Some(footer),
        ));
    if lines.is_empty() {
        view = view.child(ui::plain_group(
            hook::id("import", "queue"),
            palette,
            vec![ui::note_row(
                palette,
                hook::id("import", "empty"),
                EMPTY_TEXT.to_owned(),
                false,
            )],
        ));
    } else {
        let rows = lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| item_row(palette, import, index, line))
            .collect();
        view = view.child(ui::plain_group(hook::id("import", "queue"), palette, rows));
    }

    let dropped = {
        let import = import.clone();
        move |paths: &ExternalPaths, _: &mut Window, cx: &mut App| {
            let paths = paths.paths().to_vec();
            import.update(cx, |import, cx| {
                import.add_paths(paths, cx);
            });
        }
    };
    let selection = palette.selection;
    view.id(hook::id("import", "view"))
        .test_support()
        .rounded(px(radius::GROUP))
        .on_drop::<ExternalPaths>(dropped)
        .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(selection))
}

fn detail_of(item: &super::Item, home: Option<&std::path::Path>) -> String {
    match item.state {
        State::Failed => item
            .failure
            .as_ref()
            .map(|(code, message)| format!("{message} ({code})"))
            .unwrap_or_default(),
        State::Done => snippet(item.text.as_deref().unwrap_or("")),
        _ => item
            .path
            .parent()
            .map(|folder| hushpen_core::paths::display(folder, home))
            .unwrap_or_default(),
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
        State::Cancelling => ("Cancelling".to_owned(), palette.label_secondary),
        State::Done => ("Done".to_owned(), palette.success),
        State::Failed => ("Failed".to_owned(), palette.error),
        State::Cancelled => ("Cancelled".to_owned(), palette.label_secondary),
    }
}

fn item_row(palette: Palette, import: &Entity<Import>, index: usize, line: Line) -> AnyElement {
    let (status, color) = status_of(palette, &line);
    let id = line.id;
    let finished = line.state.finished();
    let cancelling = line.state == State::Cancelling;
    let press = {
        let import = import.clone();
        move |_: &mut Window, cx: &mut App| {
            // A refusal means the item already moved on; the row repaints either way.
            let _ = import.update(cx, |import, cx| {
                if finished {
                    import.remove(id, cx)
                } else {
                    import.cancel(id, cx)
                }
            });
        }
    };
    let button = ui::button(
        palette,
        if finished {
            hook::indexed("import", "remove", index)
        } else {
            hook::indexed("import", "cancel", index)
        },
        if finished { "Remove" } else { "Cancel" },
        Kind::Plain,
        Some(&line.focus),
        !cancelling,
        press,
    );
    ui::row(palette)
        .id(hook::indexed("import", "row", index))
        .test_support()
        .child(
            ui::text_column()
                .child(
                    ui::title(palette)
                        .id(hook::indexed("import", "name", index))
                        .test_support()
                        .aria_label(line.name.clone())
                        .child(line.name),
                )
                .child(
                    ui::secondary(palette)
                        .id(hook::indexed("import", "detail", index))
                        .test_support()
                        .aria_label(line.detail.clone())
                        .w_full()
                        .child(line.detail),
                ),
        )
        .child(
            ui::value(palette)
                .id(hook::indexed("import", "status", index))
                .test_support()
                .aria_label(status.clone())
                .text_color(color)
                .child(status),
        )
        .child(button)
        .into_any_element()
}
