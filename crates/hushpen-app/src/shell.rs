//! The app window: sidebar, toolbar, window controls (drawn on Linux only), and the active view.

use crate::about::{self, About};
use crate::dictation::{self, Dictation};
use crate::dictionary::{self, Dictionary};
use crate::history::{self, History};
use crate::hook;
use crate::mic::{self, Mic};
use crate::models::{self, Models};
use crate::onboarding::{self, Onboarding};
use crate::settings::{self, Settings};
use crate::shortcuts::Shortcuts;
use crate::theme::{self, BODY, Palette, StyledType, TOOLBAR_TITLE, chrome, radius, size, space};
use crate::views::View;
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    App, ClickEvent, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, ParentElement, Point, Render, Role, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, px, rgb, svg, transparent_black,
};

pub struct Shell {
    active: View,
    /// Holds focus at start. Key bindings only run along the path to a
    /// focused element, so without it the first Tab would reach no handler.
    shell_focus: FocusHandle,
    nav_focus: Vec<FocusHandle>,
    mic: Option<Entity<Mic>>,
    models: Option<Entity<Models>>,
    dictation: Option<Entity<Dictation>>,
    dictionary: Option<Entity<Dictionary>>,
    history: Option<Entity<History>>,
    shortcuts: Option<Entity<Shortcuts>>,
    settings: Option<Entity<Settings>>,
    about: Option<Entity<About>>,
    /// While it is active, the window shows onboarding instead of the sidebar and the views.
    onboarding: Option<Entity<Onboarding>>,
    /// Onboarding was on screen in the last frame.
    onboarding_shown: bool,
    /// The scroll position of the content pane. The History list reads it to draw only the
    /// rows near the screen and to see when its end shows.
    content_scroll: ScrollHandle,
    /// The palette of the last frame, for the test hook and for tests.
    rendered: theme::Appearance,
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
            dictation: None,
            dictionary: None,
            history: None,
            shortcuts: None,
            settings: None,
            about: None,
            onboarding: None,
            onboarding_shown: false,
            content_scroll: ScrollHandle::new(),
            rendered: theme::Appearance::Dark,
        }
    }

    /// The appearance the window was last drawn with.
    pub fn rendered_appearance(&self) -> theme::Appearance {
        self.rendered
    }

    /// Shows the shortcut recorder in the Settings view and repaints when it changes.
    pub fn attach_shortcuts(&mut self, shortcuts: Entity<Shortcuts>, cx: &mut Context<Self>) {
        cx.observe(&shortcuts, |_, _, cx| cx.notify()).detach();
        self.shortcuts = Some(shortcuts);
        cx.notify();
    }

    /// Shows the Settings view and repaints when it changes.
    pub fn attach_settings(&mut self, settings: Entity<Settings>, cx: &mut Context<Self>) {
        cx.observe(&settings, |_, _, cx| cx.notify()).detach();
        self.settings = Some(settings);
        cx.notify();
    }

    /// Shows the About view and repaints when it changes.
    pub fn attach_about(&mut self, about: Entity<About>, cx: &mut Context<Self>) {
        cx.observe(&about, |_, _, cx| cx.notify()).detach();
        self.about = Some(about);
        cx.notify();
    }

    /// Shows onboarding in place of the normal window while it is active.
    pub fn attach_onboarding(&mut self, onboarding: Entity<Onboarding>, cx: &mut Context<Self>) {
        cx.observe(&onboarding, |_, _, cx| cx.notify()).detach();
        self.onboarding = Some(onboarding);
        cx.notify();
    }

    /// Shows the personal dictionary in the Dictionary view and repaints when it changes.
    pub fn attach_dictionary(&mut self, dictionary: Entity<Dictionary>, cx: &mut Context<Self>) {
        cx.observe(&dictionary, |_, _, cx| cx.notify()).detach();
        self.dictionary = Some(dictionary);
        cx.notify();
    }

    /// Shows the transcript history in the History view and repaints when it changes.
    pub fn attach_history(&mut self, history: Entity<History>, cx: &mut Context<Self>) {
        cx.observe(&history, |_, _, cx| cx.notify()).detach();
        let scroll = self.content_scroll.clone();
        history.update(cx, |history, _| history.use_scroll(scroll));
        self.history = Some(history);
        cx.notify();
    }

    /// Shows dictation on Home and repaints when it changes.
    pub fn attach_dictation(&mut self, dictation: Entity<Dictation>, cx: &mut Context<Self>) {
        cx.observe(&dictation, |_, _, cx| cx.notify()).detach();
        self.dictation = Some(dictation);
        cx.notify();
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
            self.content_scroll.set_offset(Point::default());
            hook::record_event("view", view.key());
            // The Settings view re-reads launch at login from the system.
            if view == View::Settings
                && let Some(settings) = &self.settings
            {
                settings.update(cx, |settings, cx| settings.opened(cx));
            }
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

    fn sidebar_title(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        self.title_bar(hook::id("sidebar", "title"), cx)
    }

    /// The strip at the top of the window, over the window controls. It is also the place to
    /// drag the window by. On macOS it is empty: AppKit draws the traffic lights over it.
    #[cfg_attr(target_os = "macos", allow(unused_variables))]
    fn title_bar(
        &self,
        id: gpui_kit::SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strip = div()
            .id(id)
            .h(px(space::SIDEBAR_TOP))
            .w_full()
            .flex_none()
            .flex()
            .items_start()
            .pl(px(size::TRAFFIC_LIGHT_INSET))
            .pt(px(size::TRAFFIC_LIGHT_INSET))
            .on_mouse_down(MouseButton::Left, start_move);
        #[cfg(not(target_os = "macos"))]
        let strip = strip.child(crate::window_controls::controls(cx));
        strip
    }

    fn nav_item(
        &self,
        view: View,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
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
            .flex_none()
            .px(px(size::NAV_ITEM_PADDING - size::FOCUS_RING))
            .rounded(px(radius::SELECTION))
            .border_2()
            .border_color(transparent_black())
            .cursor_pointer()
            .text_token(BODY)
            .text_color(palette.label)
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(view, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.nav_key(view, event, window, cx)
            }));
        let item = if active {
            item.bg(palette.selection)
        } else {
            item.hover(move |style| style.bg(palette.hover()))
        };
        item.child(
            svg()
                .path(view.icon().path())
                .size(px(size::NAV_ICON))
                .flex_none()
                .text_color(if active {
                    palette.label
                } else {
                    palette.label_secondary
                }),
        )
        .child(view.title())
    }

    fn sidebar(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<_> = View::ALL
            .into_iter()
            .map(|view| self.nav_item(view, palette, cx))
            .collect();
        let panel = div()
            .id(hook::id("sidebar", "panel"))
            .test_support()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(space::SIDEBAR_WIDTH))
            .h_full();
        // No fill on macOS: the blurred window shows through the sidebar.
        let panel =
            match chrome::sidebar_fill(chrome::TRANSLUCENT_SIDEBAR, palette.appearance.is_dark()) {
                Some(fill) => panel.bg(rgb(fill)),
                None => panel,
            };
        panel.child(self.sidebar_title(cx)).child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::XXS))
                .px(px(space::SM))
                .children(items),
        )
    }

    /// The title sits in the title bar area, beside the window controls, and has no line under it.
    fn toolbar(&self, palette: Palette) -> impl IntoElement {
        div()
            .id(hook::id("toolbar", "drag"))
            .test_support()
            .flex()
            .items_center()
            .flex_none()
            .w_full()
            .h(px(space::TOOLBAR_HEIGHT))
            .px(px(space::XXL))
            .on_mouse_down(MouseButton::Left, start_move)
            .child(
                div()
                    .id(hook::id("view", "title"))
                    .test_support()
                    .aria_label(self.active.title())
                    .text_token(TOOLBAR_TITLE)
                    .text_color(palette.label)
                    .child(self.active.title()),
            )
    }

    fn content(&self, shell: &Entity<Shell>, palette: Palette, cx: &mut App) -> impl IntoElement {
        let view = self.active;
        let body = match (
            &self.mic,
            &self.models,
            &self.dictionary,
            &self.history,
            view,
        ) {
            (Some(mic), _, _, _, View::Home) => {
                let mut home = div().flex().flex_col().gap(px(space::XXL));
                if let Some(dictation) = &self.dictation {
                    home = home.child(dictation::panel::render(dictation, shell, cx));
                }
                home.child(mic::panel::render(mic, cx)).into_any_element()
            }
            (_, Some(models), _, _, View::Models) => {
                models::panel::render(models, cx).into_any_element()
            }
            (_, _, _, Some(history), View::History) => {
                history::panel::render(history, cx).into_any_element()
            }
            (_, _, Some(dictionary), _, View::Dictionary) => {
                dictionary::panel::render(dictionary, cx).into_any_element()
            }
            (_, _, _, _, View::Settings) => match &self.settings {
                Some(settings) => {
                    settings::render(settings, self.shortcuts.as_ref(), cx).into_any_element()
                }
                None => placeholder(view, palette).into_any_element(),
            },
            (_, _, _, _, View::About) => match &self.about {
                Some(about) => about::render(about, cx).into_any_element(),
                None => placeholder(view, palette).into_any_element(),
            },
            _ => placeholder(view, palette).into_any_element(),
        };
        div()
            .id(hook::id("content", "pane"))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(palette.content)
            .child(self.toolbar(palette))
            .child(
                div()
                    .id(hook::id("content", "scroll"))
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.content_scroll)
                    .child(
                        div()
                            .w_full()
                            .px(px(space::XXL))
                            .pt(px(space::XS))
                            .pb(px(space::XXL))
                            .child(body),
                    ),
            )
    }
}

fn placeholder(view: View, palette: Palette) -> impl IntoElement {
    div()
        .id(hook::id(view.key(), "placeholder"))
        .test_support()
        .text_color(palette.label_secondary)
        .child("Nothing here yet.")
}

fn start_move(event: &MouseDownEvent, window: &mut Window, _cx: &mut gpui_kit::App) {
    // A double click must not reach the window manager as a second move.
    if event.click_count == 1 {
        window.start_window_move();
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = cx.entity();
        theme::sync(window);
        let palette = theme::palette(cx);
        self.rendered = palette.appearance;
        let frame = div()
            .id("shell")
            .track_focus(&self.shell_focus)
            .flex()
            .size_full()
            .overflow_hidden()
            .text_color(palette.label)
            .font_family(theme::FONT_FAMILY)
            .text_token(BODY);
        let onboarding = self.onboarding.clone().filter(|o| o.read(cx).active());
        if self.onboarding_shown && onboarding.is_none() {
            // The control that held focus left with onboarding, and with no focus the keys reach
            // no handler.
            let home = self.shell_focus.clone();
            window.defer(cx, move |window, cx| window.focus(&home, cx));
        }
        if onboarding.is_some() && window.focused(cx).is_none() {
            // A control can leave the page while it has focus, such as the Download button when
            // the model is ready. Onboarding repaints every second, so the loss is seen here.
            let home = self.shell_focus.clone();
            window.defer(cx, move |window, cx| window.focus(&home, cx));
        }
        self.onboarding_shown = onboarding.is_some();
        match onboarding {
            // Opaque: the window background is blurred on macOS and onboarding has no sidebar.
            Some(onboarding) => frame
                .bg(palette.content)
                .flex_col()
                .child(self.title_bar(hook::id("onboarding", "title-bar"), cx))
                .child(
                    div()
                        .id(hook::id("onboarding", "scroll"))
                        .test_support()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(onboarding::panel::render(&onboarding, cx)),
                ),
            None => frame
                .child(self.sidebar(palette, cx))
                .child(self.content(&shell, palette, cx)),
        }
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
        cx.update(theme::install);
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
                |window, cx| {
                    theme::observe(window).detach();
                    cx.new(|cx| Shell::new(window, cx))
                },
            )
            .expect("open test window")
        });
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        (handle, shell)
    }

    fn render(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }

    fn rendered(cx: &mut TestAppContext, shell: &Entity<Shell>) -> theme::Appearance {
        shell.read_with(cx, |shell, _| shell.rendered_appearance())
    }

    fn rect(
        cx: &mut TestAppContext,
        handle: gpui_kit::AnyWindowHandle,
        id: &'static str,
    ) -> (f32, f32, f32, f32) {
        let bounds = cx
            .update_window(handle, |_, window, _| window.find(id).bounds())
            .unwrap();
        (
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        )
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

    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn only_close_and_minimize_traffic_lights_exist(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let mut rects = Vec::new();
        for id in ["window.close", "window.minimize"] {
            assert!(present(cx, handle, id), "{id}");
            let bounds = cx
                .update_window(handle, |_, window, _| window.find(id).bounds())
                .unwrap();
            rects.push((
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ));
        }
        for id in ["window.maximize", "window.zoom", "window.fullscreen"] {
            assert!(!present(cx, handle, id), "{id}");
        }
        assert_eq!(
            rects[0],
            (20.0, 20.0, 12.0, 12.0),
            "close: 12 px circle at 20, 20"
        );
        assert_eq!(
            rects[1],
            (40.0, 20.0, 12.0, 12.0),
            "minimize: 8 px after close"
        );
    }

    #[cfg(target_os = "macos")]
    #[gpui_kit::test]
    fn macos_draws_no_window_controls(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        for id in [
            "window.close",
            "window.minimize",
            "window.maximize",
            "window.zoom",
            "window.fullscreen",
        ] {
            assert!(!present(cx, handle, id), "{id}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn the_close_light_hides_the_window_to_the_tray(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let calls = cx.update(|cx| crate::main_window::testkit::install_fake(cx, handle, true));
        click(cx, handle, "window.close");
        assert_eq!((calls.hidden.get(), calls.minimized.get()), (1, 0));
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn the_close_light_minimizes_when_no_tray_can_bring_the_window_back(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let calls = cx.update(|cx| crate::main_window::testkit::install_fake(cx, handle, false));
        click(cx, handle, "window.close");
        assert_eq!((calls.hidden.get(), calls.minimized.get()), (0, 1));
    }

    #[gpui_kit::test]
    fn the_layout_follows_the_design_tokens(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let sidebar = rect(cx, handle, "sidebar.panel");
        let toolbar = rect(cx, handle, "toolbar.drag");
        assert_eq!(sidebar.2, space::SIDEBAR_WIDTH);
        assert_eq!(sidebar.3, space::WINDOW_HEIGHT);
        assert_eq!(toolbar.3, space::TOOLBAR_HEIGHT);
        assert_eq!(toolbar.0, space::SIDEBAR_WIDTH);
        assert_eq!(toolbar.1, 0.0);
        assert_eq!(
            toolbar.2,
            space::WINDOW_WIDTH - space::SIDEBAR_WIDTH,
            "the toolbar spans the content pane"
        );
    }

    #[gpui_kit::test]
    fn the_content_pane_has_a_24_px_margin_and_groups_fill_its_width(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let scroll = rect(cx, handle, "content.scroll");
        assert_eq!(scroll.0, space::SIDEBAR_WIDTH);
        assert_eq!(scroll.2, space::WINDOW_WIDTH - space::SIDEBAR_WIDTH);
        let title = rect(cx, handle, "view.title");
        assert_eq!(
            title.0,
            space::SIDEBAR_WIDTH + space::XXL,
            "title at the margin"
        );
        assert!(
            title.1 + title.3 <= space::TOOLBAR_HEIGHT,
            "the title sits in the title bar area"
        );
    }

    #[gpui_kit::test]
    fn sidebar_rows_are_28_px_high_2_px_apart_and_start_below_the_window_buttons(
        cx: &mut TestAppContext,
    ) {
        let (handle, _) = open(cx);
        let mut last_bottom = None;
        for view in View::ALL {
            let id: &'static str = Box::leak(format!("sidebar.{}", view.key()).into_boxed_str());
            let (x, y, width, height) = rect(cx, handle, id);
            assert_eq!(height, size::NAV_ITEM_HEIGHT, "{id}");
            assert_eq!(x, space::SM, "{id} has 8 px at its left");
            assert_eq!(width, space::SIDEBAR_WIDTH - 2.0 * space::SM, "{id}");
            match last_bottom {
                None => assert!(
                    y >= space::SIDEBAR_TOP,
                    "the first row is below the window buttons, at {y}"
                ),
                Some(bottom) => assert_eq!(y, bottom + space::XXS, "{id} follows its neighbor"),
            }
            last_bottom = Some(y + height);
        }
    }

    #[gpui_kit::test]
    fn the_window_follows_the_system_appearance_live_and_keeps_its_state(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        click(cx, handle, "sidebar.history");
        shell.update(cx, |shell, _| {
            shell
                .content_scroll
                .set_offset(Point::new(px(0.0), px(-10.0)));
        });
        render(cx, handle);
        assert_eq!(
            rendered(cx, &shell),
            theme::Appearance::Light,
            "a light system"
        );
        let scroll = shell.read_with(cx, |shell, _| shell.content_scroll.offset());

        cx.update(|cx| theme::set_system(theme::System::Dark, cx));
        render(cx, handle);
        assert_eq!(
            rendered(cx, &shell),
            theme::Appearance::Dark,
            "a dark system"
        );
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.active()),
            View::History
        );
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.content_scroll.offset()),
            scroll,
            "the scroll position stays"
        );

        cx.update(|cx| theme::set_system(theme::System::Light, cx));
        render(cx, handle);
        assert_eq!(rendered(cx, &shell), theme::Appearance::Light);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.active()),
            View::History
        );
    }

    #[gpui_kit::test]
    fn an_unknown_system_appearance_draws_dark(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        assert_eq!(rendered(cx, &shell), theme::Appearance::Light);
        cx.update(|cx| theme::set_system(theme::System::Unknown, cx));
        render(cx, handle);
        assert_eq!(rendered(cx, &shell), theme::Appearance::Dark);
    }

    #[gpui_kit::test]
    fn a_forced_palette_wins_over_the_system_until_it_is_released(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        cx.update(|cx| theme::force(Some(theme::Appearance::Dark), cx));
        render(cx, handle);
        assert_eq!(rendered(cx, &shell), theme::Appearance::Dark);
        cx.update(|cx| theme::set_system(theme::System::Light, cx));
        render(cx, handle);
        assert_eq!(
            rendered(cx, &shell),
            theme::Appearance::Dark,
            "a system change does not move a forced palette"
        );
        cx.update(|cx| theme::force(None, cx));
        render(cx, handle);
        assert_eq!(
            rendered(cx, &shell),
            theme::Appearance::Light,
            "back to the system"
        );
    }
}
