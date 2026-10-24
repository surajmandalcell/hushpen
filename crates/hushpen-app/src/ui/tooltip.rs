//! Tooltips, and the rule that none shows over a sheet. GPUI keeps a tooltip up while the
//! pointer is inside the bounds of its element, even when a sheet covers that element, and a
//! tooltip whose delay started before the sheet opened shows on top of it. The page under a
//! sheet is built inside [`covered`], where [`tooltip`] adds none, and GPUI drops the tooltip of
//! an element that no longer has one.

use crate::hook;
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{InteractiveElement, ParentElement, SharedString, StatefulInteractiveElement, div};
use std::cell::Cell;

thread_local! {
    static COVERED: Cell<bool> = const { Cell::new(false) };
}

/// Runs `build` with the tooltips of what it builds turned off while `covered`. Only builders
/// that run inside `build` see it, so a view that renders later on its own keeps its tooltips.
pub fn covered<R>(covered: bool, build: impl FnOnce() -> R) -> R {
    let before = COVERED.replace(covered);
    let built = build();
    COVERED.set(before);
    built
}

/// Gives `element` a tooltip that says `text`, unless a sheet covers the page. The text is the
/// label of `tooltip.text`, so a test reads the tooltip that shows.
pub fn tooltip<E: StatefulInteractiveElement>(element: E, text: impl Into<SharedString>) -> E {
    if COVERED.get() {
        return element;
    }
    let text = text.into();
    element.tooltip(move |window, cx| {
        let text = text.clone();
        Tooltip::element(move |_, _| {
            div()
                .id(hook::id("tooltip", "text"))
                .test_support()
                .aria_label(text.clone())
                .child(text.clone())
        })
        .build(window, cx)
    })
}
