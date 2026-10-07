//! The app window: sidebar, toolbar, traffic lights, and the active view.

use crate::hook;
use crate::mic::{self, Mic};
use crate::models::{self, Models};
use crate::theme::{self, BODY_MD, StyledType, TITLE_MD, color, radius, size, space};
use crate::views::View;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, ParentElement, Render, Role, StatefulInteractiveElement, Styled,
    Window, div, px, svg, transparent_black,
};

const TRAFFIC_GROUP: &str = "traffic-lights";

pub struct Shell {
    active: View,
    /// Holds focus at start. Key bindings only run along the path to a
    /// focused element, so without it the first Tab would reach no handler.
    shell_focus: FocusHandle,
    nav_focus: Vec<FocusHandle>,
    mic: Option<Entity<Mic>>,
    models: Option<Entity<Models>>,
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let shell_focus = cx.focus_handle();
        window.focus(&shell_focus, cx);
        Self {
            active: View::Home,
            shell_focus,
            nav_focus: View::ALL
                .iter()
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            mic: None,
            models: None,
        }
    }

    /// Shows the model library in the Models view and repaints when it changes.
    pub fn attach_models(&mut self, models: Entity<Models>, cx: &mut Context<Self>) {
        cx.observe(&models, |_, _, cx| cx.notify()).detach();
        self.models = Some(models);
        cx.notify();
    }

    /// Shows the microphone panel on Home and repaints when the microphone changes.
    pub fn attach_mic(&mut self, mic: Entity<Mic>, cx: &mut Context<Self>) {
        cx.observe(&mic, |_, _, cx| cx.notify()).detach();
        self.mic = Some(mic);
        cx.notify();
    }

    pub fn active(&self) -> View {
        self.active
    }

    pub fn select(&mut self, view: View, cx: &mut Context<Self>) {
        if self.active != view {
            self.active = view;
            hook::record_event("view", view.key());
            cx.notify();
        }
    }

    fn focus_nav(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.nav_focus.get(index) {
            window.focus(handle, cx);
        }
    }

    fn nav_key(
        &mut self,
        view: View,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = view.index();
        let last = View::ALL.len() - 1;
        match event.keystroke.key.as_str() {
            "enter" | "space" => self.select(view, cx),
            "down" => self.focus_nav((index + 1).min(last), window, cx),
            "up" => self.focus_nav(index.saturating_sub(1), window, cx),
            "home" => self.focus_nav(0, window, cx),
            "end" => self.focus_nav(last, window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn close(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn minimize(&mut self, _: &ClickEvent, window: &mut Window, _cx: &mut Context<Self>) {
        window.minimize_window();
    }

    fn traffic_light(
        &self,
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
                    .group_hover(TRAFFIC_GROUP, |style| style.visible()),
            )
    }

    fn sidebar_title(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id(hook::id("sidebar", "title"))
            .h(px(space::SIDEBAR_TITLE_HEIGHT))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .pl(px(space::XL))
            .on_mouse_down(MouseButton::Left, start_move)
            .child(
                div()
                    .group(TRAFFIC_GROUP)
                    .flex()
                    .gap(px(space::SM))
                    .child(self.traffic_light(
                        "close",
                        "Close",
                        color::WINDOW_CLOSE,
                        color::WINDOW_CLOSE_GLYPH,
                        IconName::X,
                        cx.listener(Self::close),
                    ))
                    .child(self.traffic_light(
                        "minimize",
                        "Minimize",
                        color::WINDOW_MINIMIZE,
                        color::WINDOW_MINIMIZE_GLYPH,
                        IconName::Minus,
                        cx.listener(Self::minimize),
                    )),
            )
    }

    fn nav_item(&self, view: View, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let active = view == self.active;
        // The 2 px border is always there so the focus ring never shifts the row.
        let item = div()
            .id(hook::id("sidebar", view.key()))
            .test_support()
            .track_focus(&self.nav_focus[view.index()])
            .role(Role::Button)
            .aria_label(view.title())
            .flex()
            .items_center()
            .gap(px(space::SM))
            .w_full()
            .h(px(size::NAV_ITEM_HEIGHT))
            .px(px(size::NAV_ITEM_PADDING - size::FOCUS_RING))
            .rounded(px(radius::NAV))
            .border_2()
            .border_color(transparent_black())
            .cursor_pointer()
            .text_token(BODY_MD)
            .text_color(theme::rgb_of(if active {
                color::TEXT_PRIMARY
            } else {
                color::TEXT_BODY
            }))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(view, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.nav_key(view, event, window, cx)
            }));
        let item = if active {
            item.bg(theme::rgb_of(color::SIDEBAR_ACTIVE))
        } else {
            item.hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
        };
        item.child(
            div()
                .size(px(size::NAV_ICON_TILE))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::XS))
                .bg(theme::rgb_of(view.tile_color()))
                .child(
                    svg()
                        .path(view.icon().path())
                        .size(px(12.0))
                        .text_color(theme::rgb_of(
                            if view.tile_color() == color::BORDER_SIDEBAR {
                                color::TEXT_BODY
                            } else {
                                color::TEXT_PRIMARY
                            },
                        )),
                ),
        )
        .child(view.title())
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<_> = View::ALL
            .into_iter()
            .map(|view| self.nav_item(view, cx))
            .collect();
        div()
            .id(hook::id("sidebar", "panel"))
            .test_support()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(space::SIDEBAR_WIDTH))
            .h_full()
            .bg(theme::rgb_of(color::SIDEBAR))
            .border_r_1()
            .border_color(theme::rgb_of(color::BORDER_SIDEBAR))
            .child(self.sidebar_title(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::XS))
                    .px(px(space::SM))
                    .children(items),
            )
    }

    fn toolbar(&self) -> impl IntoElement {
        div()
            .id(hook::id("toolbar", "drag"))
            .test_support()
            .flex()
            .items_center()
            .flex_none()
            .w_full()
            .h(px(space::TOOLBAR_HEIGHT))
            .px(px(space::LG))
            .border_b_1()
            .border_color(theme::rgb_of(color::DIVIDER))
            .on_mouse_down(MouseButton::Left, start_move)
            .child(
                div()
                    .id(hook::id("view", "title"))
                    .test_support()
                    .aria_label(self.active.title())
                    .text_token(TITLE_MD)
                    .text_color(theme::rgb_of(color::TEXT_HEADING))
                    .child(self.active.title()),
            )
    }

    fn content(&self, cx: &mut App) -> impl IntoElement {
        let view = self.active;
        let body = match (&self.mic, &self.models, view) {
            (Some(mic), _, View::Home) => mic::panel::render(mic, cx).into_any_element(),
            (_, Some(models), View::Models) => models::panel::render(models, cx).into_any_element(),
            _ => div()
                .id(hook::id(view.key(), "placeholder"))
                .test_support()
                .text_color(theme::rgb_of(color::TEXT_MUTED))
                .child("Nothing here yet.")
                .into_any_element(),
        };
        div()
            .id(hook::id("content", "pane"))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(theme::rgb_of(color::BACKGROUND))
            .child(self.toolbar())
            .child(
                div()
                    .id(hook::id("content", "scroll"))
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(space::CONTENT_MAX_WIDTH))
                            .mx_auto()
                            .p(px(space::XL))
                            .child(body),
                    ),
            )
    }
}

fn start_move(event: &MouseDownEvent, window: &mut Window, _cx: &mut gpui_kit::App) {
    // A double click must not reach the window manager as a second move.
    if event.click_count == 1 {
        window.start_window_move();
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("shell")
            .track_focus(&self.shell_focus)
            .flex()
            .size_full()
            .overflow_hidden()
            .bg(theme::rgb_of(color::BACKGROUND))
            .text_color(theme::rgb_of(color::TEXT_BODY))
            .font_family(theme::FONT_FAMILY)
            .text_token(BODY_MD)
            .child(self.sidebar(cx))
            .child(self.content(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, Entity, Point, TestAppContext, WindowBounds, WindowOptions, size,
    };

    fn open(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, Entity<Shell>) {
        cx.update(gpui_kit::init);
        let (handle, shell) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Shell::new(window, cx)),
            )
            .expect("open test window")
        });
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        (handle, shell)
    }

    fn click(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, id: &'static str) {
        cx.update_window(handle, |_, window, cx| window.click(id, cx))
            .unwrap();
        cx.run_until_parked();
    }

    fn press(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle, key: &'static str) {
        cx.update_window(handle, |_, window, cx| window.press(key, cx))
            .unwrap();
        cx.run_until_parked();
    }

    fn present(
        cx: &mut TestAppContext,
        handle: gpui_kit::AnyWindowHandle,
        id: &'static str,
    ) -> bool {
        cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
            .unwrap()
    }

    fn focused(
        cx: &mut TestAppContext,
        handle: gpui_kit::AnyWindowHandle,
        id: &'static str,
    ) -> bool {
        cx.update_window(handle, |_, window, _| {
            window.find(id).focused() == Some(true)
        })
        .unwrap()
    }

    #[gpui_kit::test]
    fn home_is_active_at_first_launch(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        assert_eq!(shell.read_with(cx, |shell, _| shell.active()), View::Home);
        assert!(present(cx, handle, "view.title"));
        assert!(present(cx, handle, "home.placeholder"));
    }

    #[gpui_kit::test]
    fn clicking_each_sidebar_item_opens_its_view(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        for (view, id) in View::ALL.into_iter().zip([
            "sidebar.home",
            "sidebar.history",
            "sidebar.dictionary",
            "sidebar.import",
            "sidebar.models",
            "sidebar.settings",
            "sidebar.about",
        ]) {
            click(cx, handle, id);
            assert_eq!(shell.read_with(cx, |shell, _| shell.active()), view, "{id}");
            for other in View::ALL {
                let placeholder = format!("{}.placeholder", other.key());
                let placeholder: &'static str = Box::leak(placeholder.into_boxed_str());
                assert_eq!(
                    present(cx, handle, placeholder),
                    other == view,
                    "{placeholder} after {id}"
                );
            }
        }
    }

    #[gpui_kit::test]
    fn the_keyboard_alone_moves_through_the_sidebar_and_opens_views(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        let active = |cx: &mut TestAppContext| shell.read_with(cx, |shell, _| shell.active());

        press(cx, handle, "tab");
        assert!(
            focused(cx, handle, "sidebar.home"),
            "the first Tab lands in the sidebar"
        );
        press(cx, handle, "down");
        assert!(focused(cx, handle, "sidebar.history"));
        assert_eq!(active(cx), View::Home, "moving focus does not open a view");
        press(cx, handle, "enter");
        assert_eq!(active(cx), View::History);
        press(cx, handle, "tab");
        assert!(
            focused(cx, handle, "sidebar.dictionary"),
            "Tab also moves between items"
        );
        press(cx, handle, "space");
        assert_eq!(active(cx), View::Dictionary);
        press(cx, handle, "end");
        assert!(focused(cx, handle, "sidebar.about"));
        press(cx, handle, "down");
        assert!(
            focused(cx, handle, "sidebar.about"),
            "Down stops at the last item"
        );
        press(cx, handle, "enter");
        assert_eq!(active(cx), View::About);
        press(cx, handle, "home");
        assert!(focused(cx, handle, "sidebar.home"));
        press(cx, handle, "up");
        assert!(
            focused(cx, handle, "sidebar.home"),
            "Up stops at the first item"
        );
    }

    #[gpui_kit::test]
    fn only_close_and_minimize_traffic_lights_exist(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        for id in ["window.close", "window.minimize"] {
            assert!(present(cx, handle, id), "{id}");
            let bounds = cx
                .update_window(handle, |_, window, _| window.find(id).bounds())
                .unwrap();
            assert_eq!(
                (f32::from(bounds.size.width), f32::from(bounds.size.height)),
                (13.0, 13.0),
                "{id}"
            );
        }
        for id in ["window.maximize", "window.zoom", "window.fullscreen"] {
            assert!(!present(cx, handle, id), "{id}");
        }
        let (close_x, minimize_x) = cx
            .update_window(handle, |_, window, _| {
                (
                    window.find("window.close").bounds().origin.x,
                    window.find("window.minimize").bounds().origin.x,
                )
            })
            .unwrap();
        assert!(close_x < minimize_x, "close comes first");
    }

    #[gpui_kit::test]
    fn the_layout_follows_the_design_tokens(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let (sidebar, toolbar) = cx
            .update_window(handle, |_, window, _| {
                (
                    window.find("sidebar.panel").bounds(),
                    window.find("toolbar.drag").bounds(),
                )
            })
            .unwrap();
        assert_eq!(f32::from(sidebar.size.width), space::SIDEBAR_WIDTH);
        assert_eq!(f32::from(sidebar.size.height), space::WINDOW_HEIGHT);
        assert_eq!(f32::from(toolbar.size.height), space::TOOLBAR_HEIGHT);
        assert_eq!(f32::from(toolbar.origin.x), space::SIDEBAR_WIDTH);
        assert_eq!(f32::from(toolbar.origin.y), 0.0);
    }

    #[gpui_kit::test]
    fn sidebar_items_are_36_px_high_in_order(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let mut last_y = f32::MIN;
        for view in View::ALL {
            let id: &'static str = Box::leak(format!("sidebar.{}", view.key()).into_boxed_str());
            let bounds = cx
                .update_window(handle, |_, window, _| window.find(id).bounds())
                .unwrap();
            assert_eq!(f32::from(bounds.size.height), size::NAV_ITEM_HEIGHT, "{id}");
            assert!(
                f32::from(bounds.origin.y) > last_y,
                "{id} is below the previous item"
            );
            last_y = f32::from(bounds.origin.y);
        }
    }
}
