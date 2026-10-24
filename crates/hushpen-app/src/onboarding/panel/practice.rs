//! The practice step: one real dictation, with the hold key or the Record button.

use crate::onboarding::Onboarding;

use crate::hook;
use crate::theme::{BODY, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div, px, transparent_black,
};

const FIELD_MIN_HEIGHT: f32 = 84.0;

pub(super) fn view(
    palette: Palette,
    onboarding: &Entity<Onboarding>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (text, hold, keys_ready, listening, busy, can_record, field_focus, record_focus) = {
        let me = onboarding.read(cx);
        (
            me.practice_text().map(str::to_owned),
            me.hold_key(cx),
            me.keys_ready(cx),
            me.practice_listening(cx),
            me.practice_busy(cx),
            me.can_practice_record(cx),
            me.practice_focus.clone(),
            me.record_focus.clone(),
        )
    };
    let passed = text.is_some();
    let instruction = if keys_ready {
        format!("Hold {hold}, say a sentence, then let go.")
    } else {
        "The hold key is not available on this system. Use the Record button instead.".to_owned()
    };
    let status = if passed {
        "That worked. The shortcuts are on now."
    } else if listening {
        "Listening"
    } else if busy {
        "Reading your words"
    } else if !can_record {
        "Download the speech model first."
    } else {
        "Waiting for your voice"
    };
    let toggle = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                let _ = me.toggle_practice_record(cx);
            });
        }
    };
    let shown = text.clone().unwrap_or_default();
    let status = ui::secondary(palette)
        .id(hook::id("onboarding", "practice.status"))
        .test_support()
        .role(Role::Status)
        .aria_label(status)
        .child(status);
    let rows: Vec<AnyElement> = vec![
        ui::icon_row(
            palette,
            IconName::Keyboard,
            ui::title(palette)
                .id(hook::id("onboarding", "practice.instruction"))
                .test_support()
                .child(instruction),
            Some(status.into_any_element()),
        )
        .child(ui::button(
            palette,
            hook::id("onboarding", "practice.record"),
            if listening { "Stop" } else { "Record" },
            Kind::Plain,
            Some(&record_focus),
            can_record || listening,
            toggle,
        ))
        .into_any_element(),
        div()
            .w_full()
            .flex_none()
            .p(px(size::ROW_PADDING))
            .child(
                div()
                    .id(hook::id("onboarding", "practice.field"))
                    .test_support()
                    .track_focus(&field_focus)
                    .role(Role::Paragraph)
                    .aria_label(if shown.is_empty() {
                        "Practice field, empty".to_owned()
                    } else {
                        shown.clone()
                    })
                    .w_full()
                    .min_h(px(FIELD_MIN_HEIGHT))
                    .p(px(space::SM - size::FOCUS_RING))
                    .rounded(px(radius::BUTTON))
                    .border_2()
                    .border_color(transparent_black())
                    .bg(palette.control)
                    .text_token(BODY)
                    .text_color(if passed {
                        palette.label
                    } else {
                        palette.label_secondary
                    })
                    .focus_visible(move |style| style.border_color(palette.accent))
                    .child(if passed {
                        shown
                    } else {
                        "Your words will show here.".to_owned()
                    }),
            )
            .into_any_element(),
    ];
    ui::group(hook::id("onboarding", "practice"), palette, rows)
}
