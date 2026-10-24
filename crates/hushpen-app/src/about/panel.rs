//! The About view: the app header, a group of facts, a group of links, and the copyright line.

use super::{About, Build, CHECK_SLOT, COPYRIGHT, TAGLINE, Target};
use crate::assets::APP_ICON;
use crate::hook;
use crate::theme::{self, BODY, Palette, SECONDARY, StyledType, TITLE, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div, img, px,
};

/// The rows of the links group, top to bottom.
const LINKS: [(Target, &str); 5] = [
    (Target::Website, "Website"),
    (Target::Github, "GitHub"),
    (Target::Issues, "Report an issue"),
    (Target::Licenses, "Open-source licenses"),
    (Target::LogFolder, "Open log folder"),
];

pub fn render(about: &Entity<About>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (build, folder, engine, notice, focus, can_check, status) = {
        let me = about.read(cx);
        (
            me.build(),
            me.data_folder_display(),
            me.engine(cx),
            me.notice.clone(),
            me.focus.clone(),
            me.can_check_for_updates(cx),
            me.update_status(cx),
        )
    };
    let show = ui::button(
        palette,
        hook::id("about", &format!("open.{}", Target::DataFolder.key())),
        "Show",
        Kind::Plain,
        Some(&focus[Target::DataFolder.slot()]),
        true,
        opens(about, Target::DataFolder),
    )
    .into_any_element();
    let facts = vec![
        fact_row(
            palette,
            "Version",
            build.version.to_owned(),
            "version",
            None,
        ),
        fact_row(palette, "Engine", engine, "engine", None),
        fact_row(palette, "Data folder", folder, "dataFolder", Some(show)),
    ];
    let links = LINKS
        .into_iter()
        .map(|(target, title)| link_row(palette, about, target, title, &focus[target.slot()]))
        .collect();
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w_full()
        .child(header(
            palette,
            about,
            build,
            can_check,
            status,
            &focus[CHECK_SLOT],
        ))
        .child(ui::group(hook::id("about", "facts"), palette, facts));
    // Under the facts and over the links, so it shows beside the control that failed.
    if let Some(message) = notice {
        page = page.child(ui::group(
            hook::id("about", "notices"),
            palette,
            vec![ui::notice_row(
                palette,
                hook::id("about", "notice"),
                message,
                Notice::Alert,
                None,
            )],
        ));
    }
    page.child(ui::group(hook::id("about", "links"), palette, links))
        .child(
            line(
                hook::id("about", "copyright"),
                COPYRIGHT.to_owned(),
                SECONDARY,
            )
            .text_color(palette.label_secondary),
        )
}

/// The icon, the name, the line under it, the build, and Check for Updates with what the last
/// check found, centered like the About window of a Mac app.
fn header(
    palette: Palette,
    about: &Entity<About>,
    build: Build,
    can_check: bool,
    status: Option<(String, bool)>,
    focus: &FocusHandle,
) -> impl IntoElement {
    let check = {
        let about = about.clone();
        move |_: &mut Window, cx: &mut App| about.update(cx, |me, cx| me.check_for_updates(cx))
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .w_full()
        .pt(px(space::SM))
        .child(
            div()
                .id(hook::id("about", "icon"))
                .test_support()
                .role(Role::Image)
                .aria_label("Hushpen app icon")
                .flex_none()
                .size(px(size::APP_ICON))
                .child(img(APP_ICON).size_full().rounded(px(radius::APP_ICON))),
        )
        .child(
            line(hook::id("about", "name"), "Hushpen".to_owned(), TITLE)
                .mt(px(space::SM))
                .text_color(palette.label),
        )
        .child(
            line(hook::id("about", "tagline"), TAGLINE.to_owned(), BODY)
                .mt(px(space::XXS))
                .text_color(palette.label_secondary),
        )
        .child(
            line(hook::id("about", "build"), build.line(), SECONDARY)
                .mt(px(space::XXS))
                .text_color(palette.label_secondary),
        )
        .child(div().mt(px(space::MD)).child(ui::button(
            palette,
            hook::id("about", "checkForUpdates"),
            "Check for Updates",
            Kind::Plain,
            Some(focus),
            can_check,
            check,
        )))
        .child(update_status(palette, status))
}

/// What the last check found, in the error color when it failed. The line keeps its height while
/// it is empty, so a check does not move the page.
fn update_status(palette: Palette, status: Option<(String, bool)>) -> impl IntoElement {
    let id = hook::id("about", "updates.status");
    match status {
        Some((text, failed)) => {
            line(id, text, SECONDARY)
                .role(Role::Status)
                .text_color(if failed {
                    palette.error
                } else {
                    palette.label_secondary
                })
        }
        None => line(id, String::new(), SECONDARY),
    }
    .mt(px(space::XS))
    .min_h(px(SECONDARY.size * SECONDARY.line_height))
}

/// One centered line of text under `id`, with its text as the label so a test reads it exactly.
fn line(
    id: SharedString,
    text: String,
    token: theme::TextToken,
) -> impl IntoElement + StatefulInteractiveElement + ParentElement + Styled {
    div()
        .id(id)
        .test_support()
        .aria_label(text.clone())
        .w_full()
        .text_center()
        .truncate()
        .text_token(token)
        .child(text)
}

/// A title with its value on the right. The value carries the tree id so a test reads it
/// exactly.
fn fact_row(
    palette: Palette,
    title: &'static str,
    value: String,
    id: &'static str,
    trailing: Option<AnyElement>,
) -> AnyElement {
    ui::fact_row(
        palette,
        hook::id("about", &format!("row.{id}")),
        hook::id("about", id),
        title,
        Some(value),
        trailing,
    )
}

fn opens(about: &Entity<About>, target: Target) -> impl Fn(&mut Window, &mut App) + 'static {
    let about = about.clone();
    move |_, cx| about.update(cx, |me, cx| me.open(target, cx))
}

fn link_row(
    palette: Palette,
    about: &Entity<About>,
    target: Target,
    title: &'static str,
    focus: &FocusHandle,
) -> AnyElement {
    ui::press_row(
        palette,
        hook::id("about", &format!("open.{}", target.key())),
        title,
        focus,
        true,
        opens(about, target),
    )
    .child(ui::title(palette).flex_1().min_w_0().child(title))
    .child(ui::glyph(palette, IconName::ExternalLink))
    .into_any_element()
}
