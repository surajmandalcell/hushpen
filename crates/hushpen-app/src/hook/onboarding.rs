//! Test hook for onboarding: the `onboarding` state section. The page is driven by its own
//! buttons through their element ids, like a user would.
//! Compiled only with the `test-automation` feature.

use super::set_state_section;
use crate::onboarding::Onboarding;
use gpui_kit::{App, Entity};

pub fn attach(cx: &mut App, onboarding: Entity<Onboarding>) {
    set_state_section(cx, "onboarding", move |cx| {
        onboarding.read(cx).state_json(cx)
    });
}
