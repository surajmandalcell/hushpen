//! The About view: a group with the version and the data folder, and a group of rows that open a
//! folder or a repository page.

use super::{About, Target};
use crate::hook;
use crate::theme::{self, Palette, space};
use crate::ui;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};

pub fn render(about: &Entity<About>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (version, folder, notice, focus) = {
        let me = about.read(cx);
        (
            me.version(),
            me.data_folder_display(),
            me.notice.clone(),
            me.focus.clone(),
        )
    };
    let facts = vec![
        fact_row(palette, "Version", version.to_owned(), "version"),
        fact_row(palette, "Data folder", folder, "dataFolder"),
    ];
    let actions = vec![
        action_row(
            palette,
            about,
            Target::DataFolder,
            "Open data folder",
            "The recordings, the history, and the settings.",
            &focus[0],
        ),
        action_row(
            palette,
            about,
            Target::LogFolder,
            "Open log folder",
            "The logs a bug report asks for.",
            &focus[1],
        ),
        action_row(
            palette,
            about,
            Target::Github,
            "GitHub",
            "The source and the releases.",
            &focus[2],
        ),
        action_row(
            palette,
            about,
            Target::Issues,
            "Report issue",
            "Something is wrong? Tell us.",
            &focus[3],
        ),
        action_row(
            palette,
            about,
            Target::Licenses,
            "Open-source licenses",
            "The notices of the libraries Hushpen builds on.",
            &focus[4],
        ),
    ];
    let mut page = div().flex().flex_col().gap(px(space::LG)).w_full();
    if let Some(message) = notice {
        page = page.child(ui::group(
            hook::id("about", "notices"),
            palette,
            vec![ui::note_row(
                palette,
                hook::id("about", "notice"),
                message,
                true,
            )],
        ));
    }
    page.child(ui::plain_group(hook::id("about", "panel"), palette, facts))
        .child(ui::plain_group(
            hook::id("about", "actions"),
            palette,
            actions,
        ))
}

/// A title with its value on the right. The value carries the tree id so a test reads it
/// exactly.
fn fact_row(palette: Palette, title: &'static str, value: String, id: &'static str) -> AnyElement {
    ui::labeled_row(
        palette,
        hook::id("about", &format!("row.{id}")),
        title,
        None,
        ui::value(palette)
            .id(hook::id("about", id))
            .test_support()
            .aria_label(value.clone())
            .max_w(px(420.0))
            .truncate()
            .child(value),
    )
}

fn action_row(
    palette: Palette,
    about: &Entity<About>,
    target: Target,
    title: &'static str,
    detail: &'static str,
    focus: &FocusHandle,
) -> AnyElement {
    let open = {
        let about = about.clone();
        move |_: &mut Window, cx: &mut App| about.update(cx, |me, cx| me.open(target, cx))
    };
    ui::press_row(
        palette,
        hook::id("about", &format!("open.{}", target.key())),
        title,
        focus,
        true,
        open,
    )
    .child(
        ui::text_column()
            .child(ui::title(palette).child(title))
            .child(ui::secondary(palette).child(detail)),
    )
    .child(ui::glyph(palette, IconName::ArrowUpRight))
    .into_any_element()
}
