//! Pop-up buttons and their menus. An open menu floats above the page under its button, so no
//! group cuts it, and it shows whole options only, scrolling past the eighth. A choice, Esc, or
//! a press outside closes it, and the button keeps the focus; Tab closes it on the way to the
//! next control. Up and Down move through the options.
//!
//! A menu with more options than it shows at once starts with a search field that has the
//! focus. Typing keeps the options whose label has the text, Up and Down walk the ones left,
//! and Enter picks the first. Esc clears the text first and closes the menu the next time.

use super::controls::{menu_button, search_field};
use super::{secondary, title};
use crate::motion;
use crate::theme::{self, BODY, Palette, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::Popup;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    Anchor, AnyElement, App, AppContext as _, BoxShadow, ClickEvent, Div, Entity, FocusHandle,
    Focusable as _, InteractiveElement, IntoElement, KeyDownEvent, MouseDownEvent, ParentElement,
    Pixels, Point, RenderOnce, Role, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Window, div, point, px, svg, transparent_black,
};
use std::rc::Rc;

type Close = Rc<dyn Fn(&mut App)>;
type Choose = Rc<dyn Fn(&mut Window, &mut App)>;

/// One option of a menu.
struct Choice {
    id: SharedString,
    label: String,
    detail: Option<String>,
    selected: bool,
    focus: FocusHandle,
    choose: Choose,
}

/// The open menu of a pop-up button: the list box `list` and its options. `close` runs after a
/// choice, on Esc, and on a press outside the menu.
pub struct Menu {
    list: SharedString,
    choices: Vec<Choice>,
    close: Close,
}

impl Menu {
    pub fn new(list: SharedString, close: impl Fn(&mut App) + 'static) -> Self {
        Self {
            list,
            choices: Vec::new(),
            close: Rc::new(close),
        }
    }

    /// Adds the option `id`. The chosen one has a check before its label, and `detail` is a
    /// short secondary note after it.
    pub fn option(
        mut self,
        id: SharedString,
        label: impl Into<String>,
        detail: Option<String>,
        selected: bool,
        focus: &FocusHandle,
        choose: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.choices.push(Choice {
            id,
            label: label.into(),
            detail,
            selected,
            focus: focus.clone(),
            choose: Rc::new(choose),
        });
        self
    }

    /// The chosen option, or the first when none is.
    fn chosen(&self) -> usize {
        self.choices
            .iter()
            .position(|each| each.selected)
            .unwrap_or(0)
    }

    /// Whether the menu has more options than it shows at once, and so has a search field.
    fn searchable(&self) -> bool {
        self.choices.len() > size::MENU_OPTIONS_SHOWN
    }
}

/// A pop-up button: the current value and a chevron, and its menu while it is open.
pub struct PopUp<'a> {
    /// The button. The element that holds the button and its menu is `<id>.popup`.
    pub id: SharedString,
    /// What the button picks; its accessible name is `<label>: <current>`.
    pub label: &'a str,
    pub current: String,
    pub enabled: bool,
    pub focus: &'a FocusHandle,
    /// The open menu, or `None` while it is closed.
    pub menu: Option<Menu>,
}

/// The pop-up button, with its menu under it while it is open. `toggle` runs on a press of the
/// button.
pub fn pop_up(
    palette: Palette,
    pop_up: PopUp,
    toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let PopUp {
        id,
        label,
        current,
        enabled,
        focus,
        menu,
    } = pop_up;
    Host {
        palette,
        button: menu_button(palette, id.clone(), label, current, enabled, focus, toggle)
            .into_any_element(),
        id,
        focus: focus.clone(),
        menu,
    }
    .into_any_element()
}

/// The button and its menu. It keeps the scroll of the list from frame to frame.
#[derive(IntoElement)]
struct Host {
    palette: Palette,
    id: SharedString,
    button: AnyElement,
    focus: FocusHandle,
    menu: Option<Menu>,
}

/// The scroll of the list, whether the menu was open in the last frame, and the search of a
/// long menu.
struct ListScroll {
    handle: ScrollHandle,
    open: bool,
    /// Built when the first long menu opens.
    search: Option<Entity<InputState>>,
    /// What the search field holds.
    query: String,
    /// The widest the list has been since the menu opened. The menu keeps it while a query
    /// leaves shorter options, so it does not shrink under the pointer.
    width: Pixels,
    /// Moves Tab out of the search field while a long menu is open.
    leave: Option<Subscription>,
}

impl ListScroll {
    /// The search field, built on first use. Its text drives the options that show.
    fn search_in(
        &mut self,
        window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> Entity<InputState> {
        if let Some(search) = &self.search {
            return search.clone();
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        cx.subscribe_in(&search, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.query = input.read(cx).value().to_string();
                this.handle.set_offset(Point::default());
                cx.notify();
            }
        })
        .detach();
        self.search = Some(search.clone());
        search
    }
}

/// Whether `label` has the text of `query`, in any case.
fn matches(label: &str, query: &str) -> bool {
    label.to_lowercase().contains(&query.trim().to_lowercase())
}

impl RenderOnce for Host {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            palette,
            id,
            button,
            focus,
            menu,
        } = self;
        let state =
            window.use_keyed_state(SharedString::from(format!("{id}.scroll")), cx, |_, _| {
                ListScroll {
                    handle: ScrollHandle::new(),
                    open: false,
                    search: None,
                    query: String::new(),
                    width: px(0.0),
                    leave: None,
                }
            });
        let (scroll, opening) = state.update(cx, |state, _| {
            let opening = menu.is_some() && !state.open;
            state.open = menu.is_some();
            if menu.is_none() {
                state.leave = None;
            }
            if opening {
                state.query.clear();
                state.width = px(0.0);
            } else if menu.is_some() {
                state.width = state.width.max(state.handle.bounds().size.width);
            }
            (state.handle.clone(), opening)
        });
        let host = Popup::new(SharedString::from(format!("{id}.popup")), button)
            .anchor(Anchor::TopRight)
            .offset(px(space::XS))
            .flex()
            .flex_none();
        let Some(mut menu) = menu else {
            return host;
        };
        let search = menu
            .searchable()
            .then(|| state.update(cx, |state, cx| state.search_in(window, cx)));
        let (query, width) = {
            let state = state.read(cx);
            (state.query.clone(), state.width)
        };
        if let Some(search) = &search
            && opening
        {
            search.update(cx, |input, cx| input.set_value("", window, cx));
            let field = search.read(cx).focus_handle(cx);
            window.defer(cx, {
                let field = field.clone();
                move |window, cx| window.focus(&field, cx)
            });
            // The search field is the last tab stop of the window, so Tab would wrap around to
            // the sidebar. The tab key goes before the action that moves the focus, to leave the
            // menu from its button, as from the other pop-ups.
            let (close, button) = (Rc::clone(&menu.close), focus.clone());
            let leave = cx.intercept_keystrokes(move |event, window, cx| {
                if event.keystroke.key != "tab" || !field.is_focused(window) {
                    return;
                }
                close(cx);
                window.focus(&button, cx);
                if event.keystroke.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                cx.stop_propagation();
            });
            state.update(cx, |state, _| state.leave = Some(leave));
        }
        if search.is_some() {
            menu.choices.retain(|each| matches(&each.label, &query));
        }
        // The menu drops a few pixels into place as it fades in. Its place stays in the frame only
        // while it is open, so each opening plays once.
        let arrival = motion::enter(
            SharedString::from(format!("{id}.arrival")),
            motion::FAST,
            window,
            cx,
        );
        if opening {
            // A menu opens with its chosen option in view. The list has not been measured yet,
            // so the offset comes from the fixed option height.
            let hidden = (menu.chosen() + 1).saturating_sub(size::MENU_OPTIONS_SHOWN);
            scroll.set_offset(point(px(0.0), -px(hidden as f32 * size::MENU_ROW_HEIGHT)));
        }
        let inside = focus.is_focused(window)
            || search
                .as_ref()
                .is_some_and(|search| search.read(cx).focus_handle(cx).is_focused(window))
            || menu
                .choices
                .iter()
                .any(|each| each.focus.is_focused(window));
        if !inside && window.focused(cx).is_some() {
            // Tab and the other moves of the focus run as actions, before any key listener, so
            // the menu closes once the focus is somewhere else.
            let close = Rc::clone(&menu.close);
            window.defer(cx, move |_, cx| close(cx));
        }
        // The keys reach the host from the button and from the options, which draw later but
        // stay inside it for key dispatch.
        let search = search.map(|input| Search {
            input,
            state: state.clone(),
            width,
        });
        host.on_key_down(keys(&menu, &focus, &scroll, search.clone()))
            .content(motion::arrive(
                panel(palette, menu, &focus, &scroll, search),
                arrival,
                -motion::TRAVEL,
            ))
    }
}

/// The search field of a long menu, and the state that holds its text.
#[derive(Clone)]
struct Search {
    input: Entity<InputState>,
    state: Entity<ListScroll>,
    /// The width the list has had since the menu opened.
    width: Pixels,
}

impl Search {
    /// Empties the field. `set_value` sends no change event, so the query is set here too.
    fn clear(&self, window: &mut Window, cx: &mut App) {
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.state.update(cx, |state, cx| {
            state.query.clear();
            state.handle.set_offset(Point::default());
            cx.notify();
        });
    }

    fn focused(&self, window: &Window, cx: &App) -> bool {
        self.input.read(cx).focus_handle(cx).is_focused(window)
    }
}

/// Esc clears the search text, and closes the menu when there is none; Up and Down move through
/// the options, starting at the chosen one, and scroll the one they reach into view. Up from the
/// first option goes back to the search field, where Enter picks the first option left.
fn keys(
    menu: &Menu,
    button: &FocusHandle,
    scroll: &ScrollHandle,
    search: Option<Search>,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    let close = Rc::clone(&menu.close);
    let options: Vec<FocusHandle> = menu.choices.iter().map(|each| each.focus.clone()).collect();
    let first = menu.choices.first().map(|each| Rc::clone(&each.choose));
    let chosen = menu.chosen();
    let (button, scroll) = (button.clone(), scroll.clone());
    move |event, window, cx| match event.keystroke.key.as_str() {
        "escape" => {
            if let Some(search) = &search
                && !search.input.read(cx).value().is_empty()
            {
                search.clear(window, cx);
                search.input.update(cx, |input, cx| input.focus(window, cx));
            } else {
                close(cx);
                window.focus(&button, cx);
            }
            cx.stop_propagation();
        }
        "enter"
            if search
                .as_ref()
                .is_some_and(|search| search.focused(window, cx)) =>
        {
            if let Some(choose) = &first
                && search
                    .as_ref()
                    .is_some_and(|search| !search.input.read(cx).value().is_empty())
            {
                choose(window, cx);
                close(cx);
                window.focus(&button, cx);
            }
            cx.stop_propagation();
        }
        key @ ("down" | "up") => {
            let focused = options.iter().position(|each| each.is_focused(window));
            let next = match focused {
                None if key == "up" && search.is_some() => None,
                None => Some(chosen),
                Some(0) if key == "up" && search.is_some() => None,
                Some(at) if key == "down" => Some((at + 1).min(options.len().saturating_sub(1))),
                Some(at) => Some(at.saturating_sub(1)),
            };
            match (next, &search) {
                (Some(next), _) => {
                    if let Some(option) = options.get(next) {
                        window.focus(option, cx);
                        scroll.scroll_to_item(next);
                    }
                }
                (None, Some(search)) => {
                    search.input.update(cx, |input, cx| input.focus(window, cx))
                }
                (None, None) => {}
            }
            cx.stop_propagation();
        }
        _ => {}
    }
}

/// The floating surface: the search field of a long menu, then the options in a list box that
/// scrolls past `MENU_LIST_MAX_HEIGHT`.
fn panel(
    palette: Palette,
    menu: Menu,
    button: &FocusHandle,
    scroll: &ScrollHandle,
    search: Option<Search>,
) -> Div {
    let Menu {
        list,
        choices,
        close,
    } = menu;
    let outside = {
        let (close, button) = (Rc::clone(&close), button.clone());
        // Like a native menu, the press that closes it does not reach what is under it.
        move |_: &MouseDownEvent, window: &mut Window, cx: &mut App| {
            close(cx);
            window.focus(&button, cx);
            cx.stop_propagation();
        }
    };
    let mut rows: Vec<AnyElement> = choices
        .into_iter()
        .map(|choice| option_row(palette, choice, &close, button))
        .collect();
    if rows.is_empty() {
        rows.push(no_matches(palette, &list));
    }
    let min_width = search.as_ref().map_or(size::MENU_MIN_WIDTH, |search| {
        let seen = f32::from(search.width) + 2.0 * size::MENU_PADDING + 1.0;
        seen.clamp(size::MENU_MIN_WIDTH, size::MENU_MAX_WIDTH)
    });
    let field = search.map(|search| {
        div().w_full().mb(px(space::XS)).child(search_field(
            palette,
            SharedString::from(format!("{list}.search")),
            "Search options",
            &search.input,
        ))
    });
    div()
        .min_w(px(min_width))
        .max_w(px(size::MENU_MAX_WIDTH))
        .p(px(size::MENU_PADDING))
        .rounded(px(radius::GROUP))
        .border(theme::hairline())
        .border_color(palette.separator)
        .bg(palette.menu)
        .shadow(vec![BoxShadow {
            color: palette.shadow.into(),
            offset: point(px(0.0), px(space::XS)),
            blur_radius: px(space::MD),
            spread_radius: px(0.0),
            inset: false,
        }])
        .on_mouse_down_out(outside)
        .children(field)
        .child(
            div()
                .id(list)
                .test_support()
                .role(Role::ListBox)
                .flex()
                .flex_col()
                .max_h(px(size::MENU_LIST_MAX_HEIGHT))
                .overflow_y_scroll()
                .track_scroll(scroll)
                .children(rows),
        )
}

/// The one disabled row of a menu whose search leaves no option. Its text lines up with the
/// labels of the options.
fn no_matches(palette: Palette, list: &SharedString) -> AnyElement {
    div()
        .id(SharedString::from(format!("{list}.empty")))
        .test_support()
        .flex()
        .flex_none()
        .items_center()
        .w_full()
        .h(px(size::MENU_ROW_HEIGHT))
        .pl(px(space::SM + size::GLYPH + space::XS + space::XXS))
        .text_token(BODY)
        .text_color(palette.label_tertiary)
        .child("No matches")
        .into_any_element()
}

/// One option: the check, the label, and the note. Picking it closes the menu and gives the
/// focus back to the button.
fn option_row(palette: Palette, choice: Choice, close: &Close, button: &FocusHandle) -> AnyElement {
    let Choice {
        id,
        label,
        detail,
        selected,
        focus,
        choose,
    } = choice;
    let (close, button) = (Rc::clone(close), button.clone());
    let on_click = move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
        choose(window, cx);
        close(cx);
        window.focus(&button, cx);
    };
    let check = svg()
        .path(IconName::Check.path())
        .size(px(size::GLYPH))
        .flex_none()
        .text_color(if selected {
            palette.label
        } else {
            transparent_black().into()
        });
    let element = div()
        .id(id)
        .test_support()
        .track_focus(&focus)
        .role(Role::ListBoxOption)
        .aria_label(label.clone())
        .aria_selected(selected)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(space::XS + space::XXS))
        .w_full()
        .h(px(size::MENU_ROW_HEIGHT))
        .px(px(space::SM - size::FOCUS_RING))
        .rounded(px(radius::BUTTON))
        .border_2()
        .border_color(transparent_black())
        .text_token(BODY)
        .text_color(palette.label)
        .cursor_pointer()
        .hover(move |style| style.bg(palette.hover()))
        .focus_visible(move |style| style.border_color(palette.accent))
        .on_click(on_click)
        .child(check);
    match detail {
        Some(detail) => element
            .child(title(palette).flex_none().max_w_full().child(label))
            .child(secondary(palette).flex_1().min_w_0().child(detail)),
        None => element.child(title(palette).flex_1().min_w_0().child(label)),
    }
    .into_any_element()
}
