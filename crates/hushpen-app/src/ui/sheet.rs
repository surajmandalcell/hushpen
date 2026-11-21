//! The frame every sheet shares, drawn over the window like a macOS sheet: the scrim, and the
//! card with its title at the top, its groups in the middle, and its buttons at the bottom. The
//! card is at most 90% of the window tall. When the groups need more room, they scroll between
//! the title and the buttons, which stay in place.

use crate::motion;
use crate::theme::{self, Palette, radius, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::{
    AnyElement, App, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

/// The most of the window height a sheet takes.
const SHEET_MAX_HEIGHT: f32 = 0.9;

/// What one sheet shows. `id` names the card, `scrim` the scrim around it, and the middle part
/// is `<id>.scroll`.
pub struct Sheet {
    pub id: SharedString,
    pub scrim: SharedString,
    pub label: SharedString,
    pub header: AnyElement,
    pub body: Vec<AnyElement>,
    pub footer: AnyElement,
}

/// The scrim with the sheet in its center. `arrival` runs from 0 to 1 as the sheet comes in: the
/// scrim fades in, and the sheet fades in and rises a few pixels into place. `trap` keeps Tab
/// inside the sheet.
pub fn sheet(
    palette: Palette,
    sheet: Sheet,
    trap: &FocusHandle,
    arrival: f32,
    on_key: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let scroll = div()
        .id(SharedString::from(format!("{}.scroll", sheet.id)))
        .test_support()
        .flex_shrink(1.0)
        .min_h_0()
        .overflow_y_scroll()
        // One wrapper, so the groups keep their height and the wrapper overflows instead.
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::LG))
                .children(sheet.body),
        );
    let card = div()
        .id(sheet.id)
        .test_support()
        .role(Role::Dialog)
        .aria_label(sheet.label)
        .on_key_down(on_key)
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w(px(space::SHEET_WIDTH))
        .max_h(relative(SHEET_MAX_HEIGHT))
        .p(px(space::XL))
        .bg(palette.content)
        .rounded(px(radius::WINDOW))
        .border(theme::hairline())
        .border_color(palette.separator)
        .child(div().flex_none().child(sheet.header))
        .child(scroll)
        .child(div().flex_none().child(sheet.footer));
    // The trap takes over the id of the element it wraps, so it goes on the scrim and the sheet
    // keeps its own.
    div()
        .focus_trap(sheet.scrim, trap)
        .absolute()
        .inset_0()
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::faded(palette.scrim, arrival))
        .child(motion::arrive(card, arrival, motion::TRAVEL))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AnyWindowHandle, AppContext as _, Context, Render, ScrollDelta, TestAppContext, point, size,
    };

    /// A sheet with far more rows than a short window holds.
    struct Long;

    impl Render for Long {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let palette = theme::palette(cx);
            let trap = window.focused(cx).unwrap_or_else(|| cx.focus_handle());
            let row = |n: usize| {
                div()
                    .id(SharedString::from(format!("row.{n}")))
                    .test_support()
                    .h(px(40.0))
                    .into_any_element()
            };
            let sheet = Sheet {
                id: "sheet".into(),
                scrim: "sheet.scrim".into(),
                label: "Long".into(),
                header: div()
                    .id("title")
                    .test_support()
                    .h(px(30.0))
                    .into_any_element(),
                body: (0..40).map(row).collect(),
                footer: div()
                    .id("done")
                    .test_support()
                    .h(px(24.0))
                    .into_any_element(),
            };
            div().size_full().relative().child(super::sheet(
                palette,
                sheet,
                &trap,
                1.0,
                |_, _, _| {},
            ))
        }
    }

    #[gpui_kit::test]
    fn a_long_sheet_stays_inside_the_window_and_scrolls_between_title_and_buttons(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let handle: AnyWindowHandle = cx
            .open_window(size(px(600.0), px(500.0)), |_, _| Long)
            .into();
        let find = |cx: &mut TestAppContext, id: &'static str| {
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                window.try_find(id).expect(id).bounds()
            })
            .unwrap()
        };
        let card = find(cx, "sheet");
        assert!(card.size.height <= px(500.0 * SHEET_MAX_HEIGHT) + px(0.5));
        assert!(card.top() > px(0.0) && card.bottom() < px(500.0));
        let (title, done) = (find(cx, "title"), find(cx, "done"));
        assert!(title.top() >= card.top() && done.bottom() <= card.bottom());
        let first = find(cx, "row.0").top();
        cx.update_window(handle, |_, window, cx| {
            let down = ScrollDelta::Pixels(point(px(0.0), px(-200.0)));
            window.scroll("sheet.scroll", down, cx)
        })
        .unwrap();
        cx.run_until_parked();
        assert!(find(cx, "row.0").top() < first, "the rows scroll");
        assert_eq!(find(cx, "title"), title, "the title stays");
        assert_eq!(find(cx, "done"), done, "the buttons stay");
    }
}
