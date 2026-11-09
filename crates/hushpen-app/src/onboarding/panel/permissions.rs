//! Shared permission rows for onboarding and Settings.

use crate::hook;
use crate::onboarding::Onboarding;
use crate::theme::{Palette, size, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement, Styled, div, px, svg,
};
use hushpen_core::permission::{Action, Check, CheckId, Status};

pub(super) fn view(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    ui::group(
        hook::id("onboarding", "permissions"),
        palette,
        rows(palette, onboarding, "onboarding", "", cx),
    )
}

pub(crate) fn rows(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    scope: &'static str,
    query: &str,
    cx: &mut App,
) -> Vec<AnyElement> {
    let me = onboarding.read(cx);
    let focus = me.permission_focus.clone();
    let checks = me.rows(cx);
    let query = query.to_lowercase();
    checks
        .into_iter()
        .enumerate()
        .filter(|(_, check)| {
            query.split_whitespace().all(|word| {
                "permissions capabilities access".contains(word)
                    || check.title.to_lowercase().contains(word)
                    || check.detail.to_lowercase().contains(word)
            })
        })
        .map(|(index, check)| {
            let key = check.id.key();
            let hint = check
                .id
                .permission()
                .filter(|_| check.status != Status::Ready)
                .and_then(|permission| onboarding.read(cx).hint(permission));
            let line = permission_row(palette, onboarding, scope, check, &focus[index * 2..]);
            match hint {
                Some(text) => div()
                    .w_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(line)
                    .child(
                        ui::secondary(palette)
                            .id(hook::id(scope, &format!("permissions.hint.{key}")))
                            .test_support()
                            .px(px(size::ROW_PADDING))
                            .pb(px(space::SM))
                            .child(text),
                    )
                    .into_any_element(),
                None => line,
            }
        })
        .collect()
}

fn permission_row(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    scope: &'static str,
    row: Check,
    focus: &[FocusHandle],
) -> AnyElement {
    let key = row.id.key();
    let state_text = row.status.label();
    let tone = match row.status {
        Status::Ready => palette.success,
        Status::NeedsAttention => palette.warning,
        Status::Info | Status::NotAvailable => palette.label_secondary,
    };
    let icon = match row.id {
        CheckId::Microphone => IconName::Mic,
        CheckId::X11Session | CheckId::TrayWatcher => IconName::Monitor,
        CheckId::AdminApps => IconName::Shield,
        _ => IconName::Keyboard,
    };
    let status = if row.status == Status::NeedsAttention {
        ui::tooltip(
            div()
                .id(hook::id(scope, &format!("permissions.state.{key}")))
                .test_support()
                .role(Role::Image)
                .aria_label(state_text)
                .flex_none()
                .child(
                    svg()
                        .path(IconName::CircleAlert.path())
                        .size(px(size::NAV_ICON))
                        .flex_none()
                        .text_color(tone),
                ),
            state_text,
        )
        .into_any_element()
    } else {
        ui::value(palette)
            .id(hook::id(scope, &format!("permissions.state.{key}")))
            .test_support()
            .aria_label(state_text)
            .text_color(tone)
            .child(state_text)
            .into_any_element()
    };
    let detail = ui::secondary(palette)
        .id(hook::id(scope, &format!("permissions.detail.{key}")))
        .test_support()
        .child(row.detail);
    let buttons: Vec<_> = row
        .actions
        .into_iter()
        .enumerate()
        .filter_map(|(slot, action)| action.map(|action| (slot, action)))
        .map(|(slot, action)| {
            let onboarding = onboarding.clone();
            let id = row.id;
            let relaunch = row.relaunch && action == Action::Retry;
            let name = if relaunch { "relaunch" } else { action.key() };
            let label = if relaunch {
                "Quit and Reopen"
            } else {
                action.label()
            };
            ui::button(
                palette,
                if relaunch {
                    hook::id(scope, "permissions.relaunch")
                } else {
                    hook::id(scope, &format!("permissions.{name}.{key}"))
                },
                label,
                if action == Action::Allow || relaunch {
                    Kind::Default
                } else {
                    Kind::Plain
                },
                Some(&focus[slot]),
                true,
                move |_, cx| onboarding.update(cx, |me, cx| me.check_action(id, action, cx)),
            )
            .into_any_element()
        })
        .collect();
    ui::icon_row(
        palette,
        icon,
        ui::title(palette).child(row.title),
        Some(detail.into_any_element()),
    )
    .id(hook::id(scope, &format!("permissions.row.{key}")))
    .test_support()
    .aria_label(format!("{}: {state_text}", row.title))
    .child(status)
    .children((!buttons.is_empty()).then(|| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .children(buttons)
    }))
    .into_any_element()
}
