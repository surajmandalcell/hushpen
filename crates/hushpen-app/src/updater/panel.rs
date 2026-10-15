//! What the update check looks like: the status rows of Settings > Updates and the banner above
//! the content pane.

use super::{BANNER_DISMISS, BANNER_INSTALL, BannerKind, State, Updater};
use crate::hook;
use crate::theme::{self, space};
use crate::ui::{self, Kind};
use gpui_kit::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, ParentElement, Styled, div, px,
};

/// The status of the check with its Check now button, and, once a newer version is known, a row
/// with the release notes.
pub fn rows(
    updater: &Entity<Updater>,
    check_now_focus: &FocusHandle,
    notes_focus: &FocusHandle,
    cx: &mut App,
) -> Vec<AnyElement> {
    let palette = theme::palette(cx);
    let (text, can_check, version, failed, ready) = {
        let me = updater.read(cx);
        (
            me.status_text(),
            me.can_check_now(),
            me.state().manifest().map(|m| m.version.as_str().to_owned()),
            matches!(me.state(), State::Error(_)),
            matches!(me.state(), State::Ready(_)),
        )
    };
    let action = if ready {
        let updater = updater.clone();
        ui::button(
            palette,
            hook::id("settings", "updates.restart"),
            "Restart to update",
            Kind::Default,
            Some(check_now_focus),
            true,
            move |_, cx| updater.update(cx, |me, cx| me.restart_to_update(cx)),
        )
        .into_any_element()
    } else {
        let updater = updater.clone();
        ui::button(
            palette,
            hook::id("settings", "updates.checkNow"),
            "Check now",
            Kind::Plain,
            Some(check_now_focus),
            can_check,
            move |_, cx| updater.update(cx, |me, cx| me.check_now(cx)),
        )
        .into_any_element()
    };
    let mut rows = vec![ui::status_row(
        palette,
        hook::id("settings", "updates.status"),
        text,
        failed,
        Some(action),
    )];
    if let Some(version) = version {
        let open = {
            let updater = updater.clone();
            move |_: &mut gpui_kit::Window, cx: &mut App| {
                updater.update(cx, |me, cx| me.open_notes(cx))
            }
        };
        rows.push(ui::labeled_row(
            palette,
            hook::id("settings", "updates.notes.row"),
            format!("Version {version}"),
            Some("What changed in this version.".to_owned()),
            ui::button(
                palette,
                hook::id("settings", "updates.notes"),
                "Release notes",
                Kind::Plain,
                Some(notes_focus),
                true,
                open,
            ),
        ));
    }
    rows
}

/// The banner of an offered version: shown on every view until the person sends it away or
/// turns on automatic installs.
pub fn banner(updater: &Entity<Updater>, cx: &mut App) -> Option<AnyElement> {
    let palette = theme::palette(cx);
    let (kind, version, current, focus) = {
        let me = updater.read(cx);
        (
            me.banner_kind()?,
            me.state().manifest()?.version.as_str().to_owned(),
            me.current().as_str().to_owned(),
            me.focus.clone(),
        )
    };
    let dismiss = {
        let updater = updater.clone();
        move |_: &mut gpui_kit::Window, cx: &mut App| updater.update(cx, |me, cx| me.dismiss(cx))
    };
    let not_now = ui::button(
        palette,
        hook::id("update", "banner.dismiss"),
        "Not now",
        Kind::Plain,
        Some(&focus[BANNER_DISMISS]),
        true,
        dismiss,
    )
    .into_any_element();
    let primary =
        |id: &'static str, label: &'static str, run: fn(&mut Updater, &mut Context<Updater>)| {
            let updater = updater.clone();
            ui::button(
                palette,
                hook::id("update", id),
                label,
                Kind::Default,
                Some(&focus[BANNER_INSTALL]),
                true,
                move |_: &mut gpui_kit::Window, cx: &mut App| updater.update(cx, run),
            )
            .into_any_element()
        };
    let (title, detail, buttons) = match kind {
        BannerKind::Install => (
            format!("Version {version} is available"),
            format!("You have version {current}."),
            vec![
                not_now,
                primary("banner.install", "Download and install", Updater::install),
            ],
        ),
        BannerKind::Notice => (
            format!("Version {version} is available"),
            "Your installer owns updates. The release page has the download.".to_owned(),
            vec![
                not_now,
                primary("banner.notes", "Open release page", Updater::open_notes),
            ],
        ),
        BannerKind::Downloading(percent) => (
            match percent {
                Some(percent) => format!("Downloading version {version} ({percent}%)"),
                None => format!("Downloading version {version}"),
            },
            "You can keep working.".to_owned(),
            vec![],
        ),
        BannerKind::Restart => (
            format!("Version {version} is ready"),
            "Restart Hushpen to finish the update.".to_owned(),
            vec![
                not_now,
                primary(
                    "banner.restart",
                    "Restart to update",
                    Updater::restart_to_update,
                ),
            ],
        ),
    };
    let row = ui::labeled_row(
        palette,
        hook::id("update", "banner.row"),
        title,
        Some(detail),
        div()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .children(buttons),
    );
    Some(
        div()
            .w_full()
            .flex_none()
            .px(px(space::XXL))
            .pt(px(space::XS))
            .pb(px(space::SM))
            .child(ui::plain_group(
                hook::id("update", "banner"),
                palette,
                vec![row],
            ))
            .into_any_element(),
    )
}
