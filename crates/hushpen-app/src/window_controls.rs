//! The close and minimize circles that Linux draws, because its window has no system frame.
//! macOS never compiles this module: AppKit draws the real traffic lights there.

use crate::hook;
use crate::main_window;
use crate::theme::{self, color, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    ClickEvent, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div, px, svg,
};

const GROUP: &str = "traffic-lights";

/// Close, then minimize. There is no maximize: the window has one size.
pub fn controls<V: 'static>(cx: &mut Context<V>) -> impl IntoElement + use<V> {
    div()
        .group(GROUP)
        .flex()
        .gap(px(space::SM))
        .child(light(
            "close",
            "Close",
            color::WINDOW_CLOSE,
            color::WINDOW_CLOSE_GLYPH,
            IconName::X,
            cx.listener(|_, _: &ClickEvent, window, cx| main_window::close_requested(window, cx)),
        ))
        .child(light(
            "minimize",
            "Minimize",
            color::WINDOW_MINIMIZE,
            color::WINDOW_MINIMIZE_GLYPH,
            IconName::Minus,
            cx.listener(|_, _: &ClickEvent, window, _| window.minimize_window()),
        ))
}

fn light(
    name: &'static str,
    label: &'static str,
    fill: u32,
    glyph: u32,
    icon: IconName,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> impl IntoElement {
    div()
        .id(hook::id("window", name))
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .size(px(size::TRAFFIC_LIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme::rgb_of(fill))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(
            svg()
                .path(icon.path())
                .size(px(9.0))
                .text_color(theme::rgb_of(glyph))
                .invisible()
                .group_hover(GROUP, |style| style.visible()),
        )
}
