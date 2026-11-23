//! Settings > About, the centered hero every samhq app shares: the icon, the name, the build and
//! the update state, then Links, the About window Animation switch, this install, and the footer.

use super::{
    ANIMATION_SLOT, About, Build, CHECK_SLOT, COPYRIGHT, PRIVACY, TAGLINE, Target, app_icon,
};
use crate::hook;
use crate::theme::{self, BODY, Palette, SECONDARY, StyledType, about as metrics, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

/// The rows of the links group, top to bottom, with the short address shown on the right.
const LINKS: [(Target, &str, Option<&str>); 5] = [
    (Target::Website, "Website", Some("hushpen.samhq.app")),
    (Target::Github, "GitHub", Some("samhqapp/hushpen")),
    (Target::Issues, "Report an issue", None),
    (Target::Licenses, "Open-source licenses", None),
    (Target::LogFolder, "Open log folder", None),
];

pub fn render(about: &Entity<About>, cx: &mut App) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (build, folder, engine, notice, focus, can_check, status, animates) = {
        let me = about.read(cx);
        (
            me.build(),
            me.data_folder_display(),
            me.engine(cx),
            me.notice.clone(),
            me.focus.clone(),
            me.can_check_for_updates(cx),
            me.update_status(cx),
            me.animates(),
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
        .map(|(target, title, address)| {
            link_row(
                palette,
                about,
                target,
                title,
                address,
                &focus[target.slot()],
            )
        })
        .collect();
    let animation = {
        let about = about.clone();
        ui::toggle_row(
            palette,
            hook::id("about", "animation"),
            "Animation",
            "The glow behind the icon breathes slowly.",
            animates,
            &focus[ANIMATION_SLOT],
            move |cx| about.update(cx, |me, cx| me.set_animation(!animates, cx)),
        )
    };
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
        .child(ui::titled_section(
            palette,
            hook::id("about", "links.heading"),
            "Links",
            ui::group(hook::id("about", "links"), palette, links),
            None,
        ))
        .child(ui::titled_section(
            palette,
            hook::id("about", "window.heading"),
            "About window",
            ui::group(hook::id("about", "window"), palette, vec![animation]),
            None,
        ));
    // Over the facts, so a failed open or save shows near the control.
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
    page.child(ui::titled_section(
        palette,
        hook::id("about", "facts.heading"),
        "This install",
        ui::group(hook::id("about", "facts"), palette, facts),
        None,
    ))
    .child(
        div()
            .id(hook::id("about", "privacy"))
            .test_support()
            .aria_label(PRIVACY)
            .w_full()
            .text_center()
            .text_token(SECONDARY)
            .text_color(palette.label_secondary)
            .child(PRIVACY),
    )
    .child(
        line(
            hook::id("about", "copyright"),
            COPYRIGHT.to_owned(),
            SECONDARY,
        )
        .text_color(palette.label_secondary),
    )
}

/// The icon, the name, the line under it, the build with what the last check found, and Check
/// for Updates, centered like the About window.
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
        .pt(px(metrics::HERO_TOP))
        .child(
            div()
                .id(hook::id("about", "icon"))
                .test_support()
                .role(Role::Image)
                .aria_label("Hushpen app icon")
                .child(app_icon(metrics::HERO_ICON, &metrics::HERO_ICON_SHADOW)),
        )
        .child(
            line(hook::id("about", "name"), "Hushpen".to_owned(), BODY)
                .mt(px(metrics::HERO_TITLE_TOP))
                .text_size(px(metrics::HERO_TITLE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.label),
        )
        .child(
            line(hook::id("about", "tagline"), TAGLINE.to_owned(), BODY)
                .mt(px(metrics::TAGLINE_TOP))
                .text_color(palette.label_secondary),
        )
        .child(
            div()
                .flex()
                .justify_center()
                .w_full()
                .mt(px(metrics::VERSION_TOP))
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .child(
                    div()
                        .id(hook::id("about", "build"))
                        .test_support()
                        .aria_label(build.line())
                        .flex_none()
                        .child(build.line()),
                )
                .child(update_status(palette, status)),
        )
        .child(div().mt(px(metrics::BUTTONS_TOP)).child(ui::button(
            palette,
            hook::id("about", "checkForUpdates"),
            "Check for Updates",
            Kind::Plain,
            Some(focus),
            can_check,
            check,
        )))
}

/// What the last check found, after the build, in the error color when it failed. Empty before
/// the first check.
pub(super) fn update_status(palette: Palette, status: Option<(String, bool)>) -> impl IntoElement {
    let (text, failed) = status.unwrap_or_default();
    let shown = if text.is_empty() {
        String::new()
    } else {
        format!("  ·  {text}")
    };
    div()
        .id(hook::id("about", "updates.status"))
        .test_support()
        .role(Role::Status)
        .aria_label(text)
        .truncate()
        .text_color(if failed {
            palette.error
        } else {
            palette.label_secondary
        })
        .child(shown)
}

/// One centered line of text under `id`, with its text as the label so a test reads it exactly.
pub(super) fn line(
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
    address: Option<&'static str>,
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
    .children(address.map(|address| ui::value(palette).child(address)))
    .child(ui::glyph(palette, IconName::ExternalLink))
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::links;

    #[test]
    fn each_link_address_is_part_of_its_url() {
        for (target, _, address) in LINKS {
            let url = match target {
                Target::Website => links::SITE_URL,
                Target::Github => links::REPO_URL,
                _ => continue,
            };
            assert!(url.contains(address.unwrap()), "{url}");
        }
    }
}
