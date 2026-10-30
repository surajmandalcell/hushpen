//! Tooltips, and the rule that none shows over a sheet. GPUI keeps a tooltip up while the
//! pointer is inside the bounds of its element, even when a sheet covers that element, and a
//! tooltip whose delay started before the sheet opened shows on top of it. The page under a
//! sheet is built inside [`covered`], where [`tooltip`] adds none, and GPUI drops the tooltip of
//! an element that no longer has one.

use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    AnyView, AppContext as _, Context, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Window, div,
};
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
        let tip = Tooltip::element(move |_, _| {
            div()
                .id(hook::id("tooltip", "text"))
                .test_support()
                .aria_label(text.clone())
                .child(text.clone())
        })
        .build(window, cx);
        cx.new(|_| Arrival(tip)).into()
    })
}

/// A tooltip that fades in and rises a few pixels into place when it shows. GPUI builds it once
/// each time it shows, so it plays once and not on every frame it stays up.
struct Arrival(AnyView);

impl Render for Arrival {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let progress = motion::enter("tooltip.arrival", motion::FAST, window, cx);
        // The offset moves a child, so the place GPUI picks for the tooltip at the pointer stays
        // the same.
        div().child(motion::arrive(
            div().child(self.0.clone()),
            progress,
            motion::TRAVEL,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AnyWindowHandle, Pixels, Styled, TestAppContext, px, size};
    use std::time::Duration;

    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(tooltip(
                div().id("probe").test_support().size(px(40.0)),
                "Details",
            ))
        }
    }

    fn frame(cx: &mut TestAppContext, handle: AnyWindowHandle, millis: u64) -> Option<Pixels> {
        cx.executor().advance_clock(Duration::from_millis(millis));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            // The frames the tooltip asked for, as the display would deliver them.
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            window
                .try_find("tooltip.text")
                .map(|found| found.bounds().top())
        })
        .unwrap()
    }

    #[gpui_kit::test]
    fn a_tooltip_rises_into_place_once_as_it_shows(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| cx.set_global(motion::tests::Play));
        let handle: AnyWindowHandle = cx
            .open_window(size(px(300.0), px(200.0)), |_, _| Probe)
            .into();
        cx.update_window(handle, |_, window, cx| window.hover("probe", cx))
            .unwrap();
        let shown = frame(cx, handle, 500).expect("the tooltip shows after its delay");
        let settled = frame(cx, handle, motion::FAST.as_millis() as u64).unwrap();
        assert_eq!(
            shown - settled,
            px(motion::TRAVEL),
            "it shows lower and rises into place"
        );
        assert_eq!(
            frame(cx, handle, 1_000),
            Some(settled),
            "it stays in place while it is up"
        );
    }
}
