//! The onboarding page: a step header, one grouped panel for the step, and the Continue button.
//! There is no skip control and no way back: each step has to pass before the next one opens.

mod mic_test;
mod model;
mod permissions;
mod practice;
mod updates;

use super::{Mode, Onboarding};
use crate::theme::{self, BODY, Palette, SECONDARY, StyledType, TITLE, space};
use crate::ui::{self, Kind, Notice};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, px,
};
use hushpen_core::onboarding::Step;

/// The page of the current step. A new step fades in and rises into place, as a view does.
pub fn render(
    onboarding: &Entity<Onboarding>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (step, mode, can_continue, notice) = {
        let me = onboarding.read(cx);
        (
            me.step(),
            me.mode(),
            me.can_continue(cx),
            me.notice().map(str::to_owned),
        )
    };
    let arrival = motion::change(
        hook::id("onboarding", "step"),
        (step, mode),
        motion::BASE,
        window,
        cx,
    );
    let continue_focus = onboarding.read(cx).continue_focus.clone();
    let body = match step {
        Step::Permissions => permissions::view(palette, onboarding, cx).into_any_element(),
        Step::MicTest => mic_test::view(palette, onboarding, cx).into_any_element(),
        Step::Model => model::view(palette, onboarding, cx).into_any_element(),
        Step::Practice => practice::view(palette, onboarding, cx).into_any_element(),
        Step::Updates => updates::view(palette, onboarding, cx).into_any_element(),
    };
    let last = step == Step::Updates && mode == Mode::Setup;
    let label = if last { "Finish" } else { "Continue" };
    let advance = {
        let onboarding = onboarding.clone();
        move |_: &mut Window, cx: &mut App| {
            onboarding.update(cx, |me, cx| {
                // A refused Continue has nothing to say: the disabled button already did.
                let _ = me.advance(cx);
            });
        }
    };
    let page = div()
        .id(hook::id("onboarding", "page"))
        .test_support()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .w(px(space::ONBOARDING_WIDTH))
        .max_w_full()
        .mx_auto()
        .pt(px(space::SM))
        .pb(px(space::XXL))
        .gap(px(space::LG))
        .child(header(palette, step, mode))
        .child(body)
        .children(notice.map(|message| {
            ui::group(
                hook::id("onboarding", "notice.group"),
                palette,
                vec![ui::notice_row(
                    palette,
                    hook::id("onboarding", "notice"),
                    message,
                    Notice::Alert,
                    None,
                )],
            )
        }))
        .child(div().flex().justify_end().child(ui::button(
            palette,
            hook::id("onboarding", "continue"),
            label,
            Kind::Default,
            Some(&continue_focus),
            can_continue,
            advance,
        )));
    motion::arrive(page, arrival, motion::TRAVEL)
}

fn header(palette: Palette, step: Step, mode: Mode) -> impl IntoElement + use<> {
    let (eyebrow, title, detail) = if mode == Mode::Repair {
        (
            "Permissions changed".to_owned(),
            "Permissions".to_owned(),
            "Hushpen lost something it was allowed to use before. Allow it again to go on."
                .to_owned(),
        )
    } else {
        (
            format!("Step {} of {}", step.index() + 1, Step::ALL.len()),
            step.title().to_owned(),
            step_detail(step).to_owned(),
        )
    };
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(hook::id("onboarding", "progress"))
                .test_support()
                .aria_label(eyebrow.clone())
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .child(eyebrow),
        )
        .child(
            div()
                .id(hook::id("onboarding", "title"))
                .test_support()
                .aria_label(title.clone())
                .text_token(TITLE)
                .text_color(palette.label)
                .child(title),
        )
        .child(
            div()
                .id(hook::id("onboarding", "detail"))
                .test_support()
                .text_token(BODY)
                .text_color(palette.label_secondary)
                .child(detail),
        )
}

fn step_detail(step: Step) -> &'static str {
    match step {
        Step::Permissions => {
            "Hushpen needs a few things from your system. Each line shows where it stands now."
        }
        Step::MicTest => {
            "Press Test, say a sentence, then press Stop. You will see the level move and then your words."
        }
        Step::Model => {
            "Hushpen turns speech into text on this computer with one model file, checked before use."
        }
        Step::Practice => {
            "Try your dictation shortcut here. The shortcuts turn on for other apps once this works."
        }
        Step::Updates => {
            "Choose if Hushpen may look for new versions. You can change this later in Settings."
        }
    }
}
