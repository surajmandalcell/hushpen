//! The permissions step: one row for each thing Hushpen needs, with its state and buttons.

use crate::onboarding::Onboarding;

use crate::hook;
use crate::theme::{Palette, size, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Rgba, Role,
    StatefulInteractiveElement, Styled, Window, div, px, svg,
};
use hushpen_core::onboarding::{Row, RowState};
use hushpen_core::permission::Permission;

pub(super) fn view(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (rows, focus, relaunch_focus) = {
        let me = onboarding.read(cx);
        (
            me.rows(cx),
            me.permission_focus.clone(),
            me.relaunch_focus.clone(),
        )
    };
    let rows = rows
        .into_iter()
        .map(|row| {
            let key = row.key.key();
            let hint = row
                .opens
                .filter(|_| row.state != RowState::Ready)
                .and_then(|permission| onboarding.read(cx).hint(permission));
            let line = permission_row(palette, onboarding, row, &focus, &relaunch_focus);
            match hint {
                Some(text) => div()
                    .w_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(line)
                    .child(
                        ui::secondary(palette)
                            .id(hook::id("onboarding", &format!("permissions.hint.{key}")))
                            .test_support()
                            .px(px(size::ROW_PADDING))
                            .pb(px(space::SM))
                            .child(text),
                    )
                    .into_any_element(),
                None => line.into_any_element(),
            }
        })
        .collect();
    ui::group(hook::id("onboarding", "permissions"), palette, rows)
}

/// What the status mark of a row that needs the user says as a tooltip and to a screen reader.
const ATTENTION: &str = "Needs attention";

/// The status of a row: a word, or for a row that needs attention a warning icon with a
/// tooltip and no word. The hook tree and screen readers get the same text.
fn status(
    palette: Palette,
    id: gpui_kit::SharedString,
    text: &'static str,
    tone: Rgba,
    attention: bool,
) -> impl IntoElement + use<> {
    if attention {
        div()
            .id(id)
            .test_support()
            .role(Role::Image)
            .aria_label(text)
            .flex_none()
            .child(
                svg()
                    .path(IconName::CircleAlert.path())
                    .size(px(size::NAV_ICON))
                    .flex_none()
                    .text_color(tone),
            )
            .tooltip(move |window, cx| Tooltip::new(text).build(window, cx))
            .into_any_element()
    } else {
        ui::value(palette)
            .id(id)
            .test_support()
            .aria_label(text)
            .text_color(tone)
            .child(text)
            .into_any_element()
    }
}

fn permission_row(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    row: Row,
    focus: &[FocusHandle],
    relaunch_focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let key = row.key.key();
    let (state_text, tone) = match row.state {
        RowState::Ready => ("Ready", palette.success),
        RowState::Missing => (ATTENTION, palette.warning),
        RowState::Unavailable => ("Not available", palette.label_secondary),
    };
    let icon = match row.key {
        hushpen_core::onboarding::RowKey::Microphone => IconName::Mic,
        hushpen_core::onboarding::RowKey::X11Session => IconName::Monitor,
        hushpen_core::onboarding::RowKey::Paste => IconName::ClipboardPaste,
        _ => IconName::Keyboard,
    };
    let ready = row.state == RowState::Ready;
    let buttons = row.opens.map(|permission| {
        let allow = {
            let onboarding = onboarding.clone();
            move |_: &mut Window, cx: &mut App| {
                onboarding.update(cx, |me, cx| me.allow(permission, cx))
            }
        };
        let open = {
            let onboarding = onboarding.clone();
            move |_: &mut Window, cx: &mut App| {
                onboarding.update(cx, |me, cx| me.open_settings(permission, cx))
            }
        };
        (permission, allow, open)
    });
    let relaunch = row.relaunch.then(|| {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| onboarding.update(cx, |me, cx| me.relaunch(cx))
    });
    let detail = ui::secondary(palette)
        .id(hook::id("onboarding", &format!("permissions.detail.{key}")))
        .test_support()
        .child(row.detail);
    ui::icon_row(
        palette,
        icon,
        ui::title(palette).child(row.title),
        Some(detail.into_any_element()),
    )
    .id(hook::id("onboarding", &format!("permissions.row.{key}")))
    .test_support()
    .aria_label(format!("{}: {state_text}", row.title))
    .child(status(
        palette,
        hook::id("onboarding", &format!("permissions.state.{key}")),
        state_text,
        tone,
        row.state == RowState::Missing,
    ))
    .children(buttons.map(|(permission, allow, open)| {
        let slot = Permission::ALL
            .iter()
            .position(|candidate| *candidate == permission)
            .unwrap_or(0);
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(space::SM))
            // Allow asks the system, so it has nothing left to do once the row is Ready.
            .children((!ready).then(|| {
                ui::button(
                    palette,
                    hook::id("onboarding", &format!("permissions.allow.{key}")),
                    "Allow",
                    Kind::Default,
                    Some(&focus[slot * 2]),
                    true,
                    allow,
                )
            }))
            .child(ui::button(
                palette,
                hook::id("onboarding", &format!("permissions.open.{key}")),
                "Open Settings",
                Kind::Plain,
                Some(&focus[slot * 2 + 1]),
                true,
                open,
            ))
    }))
    .children(relaunch.map(|press| {
        ui::button(
            palette,
            hook::id("onboarding", "permissions.relaunch"),
            "Quit and Reopen",
            Kind::Default,
            Some(relaunch_focus),
            true,
            press,
        )
    }))
}
