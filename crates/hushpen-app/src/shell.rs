//! The app window: sidebar, toolbar, window controls (drawn on Linux only), and the active view.

use crate::about::{self, About};
use crate::dictation::{self, Dictation};
use crate::dictionary::{self, Dictionary};
use crate::files::{self, DROP_GROUP, Files};
use crate::flow_bar::FlowBar;
use crate::history::{self, History};
use crate::hook;
use crate::import::ImportRequest;
use crate::mic::{self, Mic};
use crate::models::{self, Models};
use crate::motion;
use crate::onboarding::{self, Onboarding};
use crate::read_aloud::ReadRequest;
use crate::settings::{self, Settings};
use crate::shortcuts::Shortcuts;
use crate::storage::Storage;
use crate::theme::{self, BODY, Palette, StyledType, TOOLBAR_TITLE, chrome, radius, size, space};
use crate::ui;
use crate::updater::{self, Updater};
use crate::views::View;
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    App, ClickEvent, Context, Entity, ExternalPaths, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Point, Render, Role, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, px, svg, transparent_black,
};
use serde_json::json;
use std::rc::Rc;

/// The view the window showed last, so the next start opens on it.
pub const LAST_VIEW: &str = "ui.lastView";

pub struct Shell {
    active: View,
    /// Holds focus at start. Key bindings only run along the path to a
    /// focused element, so without it the first Tab would reach no handler.
    shell_focus: FocusHandle,
    nav_focus: Vec<FocusHandle>,
    mic: Option<Entity<Mic>>,
    models: Option<Entity<Models>>,
    dictation: Option<Entity<Dictation>>,
    /// The shell owns the bar: its pop-up window and the tray actions hold it weakly, so
    /// without this handle the bar would be released at the end of start-up.
    flow_bar: Option<Entity<FlowBar>>,
    dictionary: Option<Entity<Dictionary>>,
    history: Option<Entity<History>>,
    files: Option<Entity<Files>>,
    shortcuts: Option<Entity<Shortcuts>>,
    settings: Option<Entity<Settings>>,
    about: Option<Entity<About>>,
    updater: Option<Entity<Updater>>,
    /// While it is active, the window shows onboarding instead of the sidebar and the views.
    onboarding: Option<Entity<Onboarding>>,
    /// Onboarding was on screen in the last frame.
    onboarding_shown: bool,
    /// The scroll position of the content pane. The History list reads it to draw only the
    /// rows near the screen and to see when its end shows.
    content_scroll: ScrollHandle,
    /// The palette of the last frame, for the test hook and for tests.
    rendered: theme::Appearance,
    /// Where the picked view is saved. Without it nothing is saved.
    storage: Option<Rc<Storage>>,
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let shell_focus = cx.focus_handle();
        window.focus(&shell_focus, cx);
        // A focused control that leaves the frame, such as a row a search hides, would take the
        // keys with it: they would reach no handler until the next click.
        cx.on_focus_lost(window, |shell, window, cx| {
            window.focus(&shell.shell_focus, cx)
        })
        .detach();
        // The Kit root does not exist yet: it wraps this view once it is built.
        window.defer(cx, crate::window_chrome::apply_look);
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
            flow_bar: None,
            dictionary: None,
            history: None,
            files: None,
            shortcuts: None,
            settings: None,
            about: None,
            updater: None,
            onboarding: None,
            onboarding_shown: false,
            content_scroll: ScrollHandle::new(),
            rendered: theme::Appearance::Dark,
            storage: None,
        }
    }

    /// Opens the view that was open at the last quit and saves each view picked from now on.
    /// Onboarding, while it runs, still covers the view.
    pub fn attach_storage(&mut self, storage: Rc<Storage>, cx: &mut Context<Self>) {
        let saved = storage
            .settings
            .get(LAST_VIEW)
            .and_then(|value| value.as_str().and_then(View::from_key));
        if let Some(view) = saved {
            self.select(view, cx);
        }
        self.storage = Some(storage);
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
        let scroll = self.content_scroll.clone();
        settings.update(cx, |settings, _| settings.use_scroll(scroll));
        self.settings = Some(settings);
        cx.notify();
    }

    /// Shows the About view and repaints when it changes.
    pub fn attach_about(&mut self, about: Entity<About>, cx: &mut Context<Self>) {
        cx.observe(&about, |_, _, cx| cx.notify()).detach();
        self.about = Some(about);
        cx.notify();
    }

    /// Shows the update banner above every view while a version waits for the person.
    pub fn attach_updater(&mut self, updater: Entity<Updater>, cx: &mut Context<Self>) {
        cx.observe(&updater, |_, _, cx| cx.notify()).detach();
        self.updater = Some(updater);
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

    /// Shows the Manual view and repaints when it changes. Show in History opens History on its
    /// row; Download in Models opens Models.
    pub fn attach_files(&mut self, files: Entity<Files>, cx: &mut Context<Self>) {
        cx.observe(&files, |_, _, cx| cx.notify()).detach();
        let (import, read) = {
            let view = files.read(cx);
            (view.import().clone(), view.read_aloud().cloned())
        };
        cx.subscribe(
            &import,
            |shell, _, request: &ImportRequest, cx| match request {
                ImportRequest::ShowInHistory(id) => shell.show_in_history(id, cx),
            },
        )
        .detach();
        if let Some(read) = read {
            cx.subscribe(&read, |shell, _, request: &ReadRequest, cx| match request {
                ReadRequest::OpenModels => shell.select(View::Models, cx),
            })
            .detach();
        }
        self.files = Some(files);
        cx.notify();
    }

    fn show_in_history(&mut self, id: &str, cx: &mut Context<Self>) {
        self.select(View::History, cx);
        if let Some(history) = &self.history {
            history.update(cx, |history, cx| {
                if let Err(why) = history.open(id, cx) {
                    log::info!("the import row did not open in History: {why}");
                }
            });
        }
    }

    /// Files dropped anywhere on the window open the Manual view and land there.
    fn dropped(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        let Some(files) = self.files.clone() else {
            return;
        };
        self.select(View::Manual, cx);
        let paths = paths.paths().to_vec();
        // The drop runs inside the window's own update, so Read aloud could not reach the
        // window to fill its text area until that ends.
        cx.defer(move |cx| files.update(cx, |files, cx| files.drop_paths(paths, cx)));
    }

    /// Keeps the flow bar alive for the app's lifetime. The bar paints in its own window, so
    /// the shell only owns the handle and does not observe or render it.
    pub fn attach_flow_bar(&mut self, flow_bar: Entity<FlowBar>) {
        self.flow_bar = Some(flow_bar);
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
            if let Some(storage) = &self.storage
                && let Err(error) = storage.settings.set_internal(LAST_VIEW, json!(view.key()))
            {
                log::warn!("could not save the last view: {error}");
            }
            // A tray action can leave Models while a details sheet is open.
            if let Some(models) = &self.models {
                models.update(cx, |models, cx| models.dismiss_details(cx));
            }
            if let Some(tuning) = self
                .settings
                .as_ref()
                .and_then(|s| s.read(cx).tuning.clone())
            {
                tuning.update(cx, |tuning, cx| tuning.dismiss(cx));
            }
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

    fn sidebar_title(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        self.title_bar(hook::id("sidebar", "top"), palette, cx)
    }

    /// The strip at the top of the window: the window controls and the word "Hushpen" beside
    /// them, on the chrome center line. It is also the place to drag the window by. On macOS
    /// AppKit draws the traffic lights over it (see `window_chrome::traffic_light_position`) and
    /// the spacer keeps their width clear; on Linux the app draws the circles itself.
    fn title_bar(
        &self,
        id: gpui_kit::SharedString,
        palette: Palette,
        // Only the drawn Linux controls need the context.
        #[cfg_attr(target_os = "macos", expect(unused_variables))] cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strip = div()
            .id(id)
            .test_support()
            .h(px(space::SIDEBAR_TOP))
            .w_full()
            .flex_none()
            .flex()
            .items_start()
            .pl(px(size::TRAFFIC_LIGHT_INSET))
            .pr(px(space::SM))
            .on_mouse_down(MouseButton::Left, start_move);
        #[cfg(not(target_os = "macos"))]
        let strip = strip.child(
            div()
                .flex_none()
                .mt(px(theme::chrome_top(size::TRAFFIC_LIGHT)))
                .child(crate::window_controls::controls(cx)),
        );
        // The width of the two native lights and their gap, kept clear under AppKit's drawing.
        #[cfg(target_os = "macos")]
        let strip = strip.child(
            div()
                .w(px(2.0 * size::TRAFFIC_LIGHT + space::SM))
                .flex_none(),
        );
        strip.child(
            div()
                .id(hook::id("sidebar", "name"))
                .test_support()
                .aria_label("Hushpen")
                .flex_1()
                .min_w_0()
                .ml(px(space::LG))
                .mt(px(theme::chrome_top(TOOLBAR_TITLE.size)))
                .truncate()
                .text_token(TOOLBAR_TITLE)
                .text_color(palette.label)
                .child("Hushpen"),
        )
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
        let look = chrome::sidebar(cx);
        let panel = div()
            .id(hook::id("sidebar", "panel"))
            .test_support()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(space::SIDEBAR_WIDTH))
            .h_full()
            .rounded_l(px(look.window_radius()));
        // No fill where the sidebar is see-through: the native material on macOS, what is
        // behind the window on a composited Linux.
        let panel = match look.fill(palette.appearance.is_dark()) {
            Some(fill) => panel.bg(fill),
            None => panel,
        };
        panel.child(self.sidebar_title(palette, cx)).child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::XXS))
                .px(px(space::SM))
                .children(items),
        )
    }

    /// The title sits in the title bar area, on the chrome center line with the window controls,
    /// and has no line under it. Settings puts its search field at the right end, on the same
    /// line. Manual puts its mode control in the middle of the pane width.
    fn toolbar(&self, shell: &Entity<Shell>, palette: Palette, cx: &App) -> impl IntoElement {
        let search = self
            .settings
            .as_ref()
            .filter(|_| self.active == View::Settings)
            .and_then(|settings| settings::search_field(settings, cx))
            .map(|field| {
                let shell = shell.clone();
                div()
                    .mt(px(theme::chrome_top(size::BUTTON_HEIGHT)))
                    // A press in the field places the caret; it must not move the window.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        shell.update(cx, |shell, cx| shell.search_key(event, window, cx))
                    })
                    .child(field)
            });
        // Centered on the pane width, not on the space the title leaves. The wrapper has no
        // handler, so a drag on the empty band still moves the window.
        let mode = self
            .files
            .as_ref()
            .filter(|_| self.active == View::Manual)
            .map(|files| {
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .mt(px(theme::chrome_top(size::BUTTON_HEIGHT)))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(files::panel::mode_switch(files, cx)),
                    )
            });
        div()
            .id(hook::id("toolbar", "drag"))
            .test_support()
            .relative()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(space::LG))
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
                    .min_w_0()
                    .truncate()
                    .mt(px(theme::chrome_top(TOOLBAR_TITLE.size)))
                    .text_token(TOOLBAR_TITLE)
                    .text_color(palette.label)
                    .child(self.active.title()),
            )
            .children(mode)
            .children(search)
    }

    /// Esc in the Settings search: the first clears the query, the next leaves the field.
    fn search_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key.as_str() != "escape" {
            return;
        }
        let Some(settings) = self.settings.clone() else {
            return;
        };
        if !settings.update(cx, |settings, cx| settings.clear_search(window, cx)) {
            window.focus(&self.shell_focus, cx);
        }
        cx.stop_propagation();
    }

    /// Cmd+F on macOS, Ctrl+F on Linux: on the Settings view, the caret goes to its search.
    fn find_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if self.active != View::Settings
            || keystroke.key.as_str() != "f"
            || !keystroke.modifiers.secondary()
        {
            return;
        }
        let search = self
            .settings
            .as_ref()
            .and_then(|settings| settings.read(cx).search.clone());
        if let Some(search) = search {
            search.update(cx, |input, cx| input.focus(window, cx));
            cx.stop_propagation();
        }
    }

    /// The title bar and the active view. `arrival` is how far the view has come in since it was
    /// picked, from 0 to 1.
    fn content(
        &self,
        shell: &Entity<Shell>,
        palette: Palette,
        arrival: f32,
        cx: &mut App,
    ) -> impl IntoElement {
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
            (_, _, _, _, View::Manual) => match &self.files {
                Some(files) => files::panel::render(files, cx).into_any_element(),
                None => placeholder(view, palette).into_any_element(),
            },
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
            .rounded_r(px(chrome::sidebar(cx).window_radius()))
            .bg(palette.content)
            .child(self.toolbar(shell, palette, cx))
            .children(
                self.updater
                    .as_ref()
                    .and_then(|updater| updater::banner(updater, cx)),
            )
            .child(
                div()
                    .id(hook::id("content", "scroll"))
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.content_scroll)
                    .child(motion::arrive(
                        div()
                            .w_full()
                            .px(px(space::XXL))
                            .pt(px(space::XS))
                            .pb(px(space::XXL))
                            .child(body),
                        arrival,
                        motion::TRAVEL,
                    )),
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
            .role(Role::Group)
            .aria_label("Hushpen")
            .track_focus(&self.shell_focus)
            .on_key_down(cx.listener(Self::find_key))
            .flex()
            .size_full()
            .overflow_hidden()
            .text_color(palette.label)
            .font_family(theme::FONT_FAMILY)
            .text_token(BODY);
        // With a compositor the Linux window is see-through: rounded corners and a hairline
        // edge, like the frame macOS draws. Without one it stays square and opaque.
        let look = chrome::sidebar(cx);
        let frame = if look == chrome::Sidebar::Blurred {
            frame
                .rounded(px(look.window_radius()))
                .border(theme::hairline())
                .border_color(palette.separator)
        } else {
            frame
        };
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
            // Opaque: the window is transparent on macOS and onboarding has no sidebar.
            Some(onboarding) => frame
                .bg(palette.content)
                .flex_col()
                .child(self.title_bar(hook::id("onboarding", "title-bar"), palette, cx))
                .child(
                    div()
                        .id(hook::id("onboarding", "scroll"))
                        .test_support()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(onboarding::panel::render(&onboarding, window, cx)),
                ),
            None => {
                let sheet = self
                    .models
                    .as_ref()
                    .filter(|_| self.active == View::Models)
                    .and_then(|models| models::sheet::render(models, window, cx))
                    .or_else(|| {
                        let tuning = self
                            .settings
                            .as_ref()
                            .filter(|_| self.active == View::Settings)
                            .and_then(|settings| settings.read(cx).tuning.clone())?;
                        crate::enhance::sheet::render(&tuning, window, cx)
                    });
                // The content of a newly picked view fades in and rises into place; the sidebar
                // and the title bar stay still.
                let arrival = motion::change(
                    hook::id("content", "view"),
                    self.active,
                    motion::BASE,
                    window,
                    cx,
                );
                let (sidebar, content) = ui::covered(sheet.is_some(), || {
                    let sidebar = self.sidebar(palette, cx).into_any_element();
                    (
                        sidebar,
                        self.content(&shell, palette, arrival, cx)
                            .into_any_element(),
                    )
                });
                frame
                    .relative()
                    .group(DROP_GROUP)
                    .on_drop::<ExternalPaths>(
                        cx.listener(|shell, paths, _, cx| shell.dropped(paths, cx)),
                    )
                    .child(sidebar)
                    .child(content)
                    .children(sheet)
            }
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
        open_window(cx)
    }

    fn open_window(cx: &mut TestAppContext) -> (gpui_kit::AnyWindowHandle, Entity<Shell>) {
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
    fn the_shell_owns_the_flow_bar_for_its_whole_run(cx: &mut TestAppContext) {
        // The bar's window and the tray actions hold the bar weakly, and start-up drops its
        // own handle. Only the test hook kept the bar alive, so a build without the hook never
        // opened the flow bar.
        let (_window, shell) = open(cx);
        let rig = crate::controller::testkit::rig(cx, &[], "");
        let weak = cx.update(|cx| {
            let bar = cx.new(|_| {
                crate::flow_bar::FlowBar::new(
                    std::rc::Rc::clone(&rig.storage),
                    rig.controller.clone(),
                    rig.mic.clone(),
                )
            });
            shell.update(cx, |shell, _| shell.attach_flow_bar(bar.clone()));
            bar.downgrade()
        });
        cx.run_until_parked();
        assert!(
            weak.upgrade().is_some(),
            "the flow bar must outlive start-up without the test hook"
        );
    }

    /// A picked view fades in and rises into place once; the sidebar and the title bar stay
    /// still.
    #[gpui_kit::test]
    fn a_picked_view_rises_into_place_while_the_chrome_stays_still(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        cx.update(|cx| cx.set_global(motion::tests::Play));
        let home = rect(cx, handle, "home.placeholder");
        let row = rect(cx, handle, "sidebar.history");
        let title = rect(cx, handle, "view.title");
        click(cx, handle, "sidebar.history");
        render(cx, handle);
        let rising = rect(cx, handle, "history.placeholder");
        assert_eq!(rising.1 - home.1, motion::TRAVEL, "it starts lower");
        assert_eq!(
            rect(cx, handle, "sidebar.history"),
            row,
            "the sidebar stays"
        );
        let moved = rect(cx, handle, "view.title");
        assert_eq!((moved.0, moved.1), (title.0, title.1), "the title stays");

        cx.executor().advance_clock(motion::BASE);
        cx.update_window(handle, |_, window, cx| window.simulate_next_frame(cx))
            .unwrap();
        render(cx, handle);
        assert_eq!(
            rect(cx, handle, "history.placeholder").1,
            home.1,
            "in place"
        );
        cx.executor().advance_clock(motion::BASE);
        render(cx, handle);
        assert_eq!(
            rect(cx, handle, "history.placeholder").1,
            home.1,
            "a later frame does not play it again"
        );
    }

    #[gpui_kit::test]
    fn home_is_active_at_first_launch(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        assert_eq!(shell.read_with(cx, |shell, _| shell.active()), View::Home);
        assert!(present(cx, handle, "view.title"));
        assert!(present(cx, handle, "home.placeholder"));
    }

    fn storage_at(root: &std::path::Path) -> Rc<Storage> {
        let data = hushpen_store::data_dir::DataDir::open(root).unwrap();
        Rc::new(crate::storage::open(data).unwrap())
    }

    fn attach_storage(cx: &mut TestAppContext, shell: &Entity<Shell>, storage: Rc<Storage>) {
        shell.update(cx, |shell, cx| shell.attach_storage(storage, cx));
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn the_next_start_opens_the_view_that_was_open_at_quit(cx: &mut TestAppContext) {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("data");
        let (handle, shell) = open(cx);
        let storage = storage_at(&root);
        attach_storage(cx, &shell, Rc::clone(&storage));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.active()),
            View::Home,
            "a first start opens Home"
        );
        click(cx, handle, "sidebar.files");
        assert_eq!(storage.settings.get(LAST_VIEW), Some(json!("files")));
        click(cx, handle, "sidebar.models");
        assert_eq!(storage.settings.get(LAST_VIEW), Some(json!("models")));

        let (next_handle, next) = open_window(cx);
        attach_storage(cx, &next, storage_at(&root));
        assert_eq!(next.read_with(cx, |shell, _| shell.active()), View::Models);
        render(cx, next_handle);
        assert!(present(cx, next_handle, "models.placeholder"));
    }

    #[gpui_kit::test]
    fn every_view_can_be_saved_as_the_last_view(cx: &mut TestAppContext) {
        let folder = tempfile::tempdir().unwrap();
        let (_, shell) = open(cx);
        let storage = storage_at(&folder.path().join("data"));
        attach_storage(cx, &shell, Rc::clone(&storage));
        for view in View::ALL.into_iter().rev() {
            shell.update(cx, |shell, cx| shell.select(view, cx));
            assert_eq!(
                storage.settings.get(LAST_VIEW),
                Some(json!(view.key())),
                "{view:?}"
            );
        }
    }

    #[gpui_kit::test]
    fn clicking_each_sidebar_item_opens_its_view(cx: &mut TestAppContext) {
        let (handle, shell) = open(cx);
        for (view, id) in View::ALL.into_iter().zip([
            "sidebar.home",
            "sidebar.history",
            "sidebar.files",
            "sidebar.dictionary",
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
            focused(cx, handle, "sidebar.files"),
            "Tab also moves between items"
        );
        press(cx, handle, "space");
        assert_eq!(active(cx), View::Manual);
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
        // The circles sit where the native ones do on macOS: 20 px in from the top left corner.
        assert_eq!(
            rects[0],
            (20.0, 20.0, 12.0, 12.0),
            "close: 12 px circle at (20, 20)"
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
    fn the_sidebar_names_the_app_beside_the_window_buttons(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let band = rect(cx, handle, "sidebar.top");
        assert_eq!(band.3, space::SIDEBAR_TOP, "the band stays 48 px");
        let (label, x, y, w, h) = cx
            .update_window(handle, |_, window, _| {
                let name = window.find("sidebar.name");
                (
                    name.label().map(str::to_owned),
                    f32::from(name.bounds().origin.x),
                    f32::from(name.bounds().origin.y),
                    f32::from(name.bounds().size.width),
                    f32::from(name.bounds().size.height),
                )
            })
            .unwrap();
        assert_eq!(label.as_deref(), Some("Hushpen"));
        // 20 + 12 + 8 + 12 for the two circles, then 16 px of clear space.
        let lights_end = size::TRAFFIC_LIGHT_INSET + 2.0 * size::TRAFFIC_LIGHT + space::SM;
        assert_eq!(x, lights_end + space::LG, "16 px right of the last circle");
        let center = y + h / 2.0;
        assert!(
            (center - size::CHROME_CENTER).abs() <= 0.5,
            "the name centers at {center}, the chrome line is {}",
            size::CHROME_CENTER
        );
        assert!(
            x + w <= space::SIDEBAR_WIDTH - space::SM,
            "the word stays inside the band"
        );
    }

    /// The window controls, the app name, and the view title share one center line. On macOS
    /// the native lights are not in the tree; `window_chrome` pins their math.
    #[gpui_kit::test]
    fn the_chrome_shares_one_center_line(cx: &mut TestAppContext) {
        let (handle, _) = open(cx);
        let mut ids = vec!["view.title", "sidebar.name"];
        if cfg!(not(target_os = "macos")) {
            ids.extend(["window.close", "window.minimize"]);
        }
        for id in ids {
            let (_, y, _, height) = rect(cx, handle, id);
            let center = y + height / 2.0;
            assert!(
                (center - size::CHROME_CENTER).abs() <= 0.5,
                "{id} centers at {center}, the chrome line is {}",
                size::CHROME_CENTER
            );
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
