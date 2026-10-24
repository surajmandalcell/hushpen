//! The updates step: the one switch for the release check.

use crate::onboarding::Onboarding;

use crate::hook;
use crate::theme::Palette;
use crate::ui;
use gpui_kit::assets::IconName;
use gpui_kit::{AnyElement, App, Entity, IntoElement, ParentElement, Window};

pub(super) fn view(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (on, focus) = {
        let me = onboarding.read(cx);
        (me.updates_on(), me.updates_focus.clone())
    };
    let flip = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                let on = !me.updates_on();
                me.set_updates(on, cx);
            });
        }
    };
    let detail =
        ui::secondary(palette).child("Off unless you turn it on. Hushpen never updates by itself.");
    let rows: Vec<AnyElement> = vec![
        ui::icon_row(
            palette,
            IconName::Download,
            ui::title(palette).child("Check for updates"),
            Some(detail.into_any_element()),
        )
        .child(ui::switch(
            palette,
            hook::id("onboarding", "updates.toggle"),
            "Check for updates",
            on,
            true,
            &focus,
            flip,
        ))
        .into_any_element(),
    ];
    ui::group(hook::id("onboarding", "updates"), palette, rows)
}
