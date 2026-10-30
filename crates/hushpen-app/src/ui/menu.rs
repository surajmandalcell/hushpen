//! Pop-up buttons and their menus. An open menu floats above the page under its button, so no
//! group cuts it, and it shows whole options only, scrolling past the eighth. A choice, Esc, or
//! a press outside closes it, and the button keeps the focus; Tab closes it on the way to the
//! next control. Up and Down move through the options.

use super::controls::menu_button;
use super::{secondary, title};
use crate::motion;
use crate::theme::{self, BODY, Palette, StyledType, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::Popup;
use gpui_kit::{
    Anchor, AnyElement, App, BoxShadow, ClickEvent, Div, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, MouseDownEvent, ParentElement, RenderOnce, Role, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, Window, div, point, px, svg,
    transparent_black,
};
use std::rc::Rc;

type Close = Rc<dyn Fn(&mut App)>;
type Choose = Box<dyn Fn(&mut Window, &mut App)>;

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
            choose: Box::new(choose),
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

/// The scroll of the list, and whether the menu was open in the last frame.
struct ListScroll {
    handle: ScrollHandle,
    open: bool,
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
                }
            });
        let (scroll, opening) = state.update(cx, |state, _| {
            let opening = menu.is_some() && !state.open;
            state.open = menu.is_some();
            (state.handle.clone(), opening)
        });
        let host = Popup::new(SharedString::from(format!("{id}.popup")), button)
            .anchor(Anchor::TopRight)
            .offset(px(space::XS))
            .flex()
            .flex_none();
        let Some(menu) = menu else {
            return host;
        };
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
        host.on_key_down(keys(&menu, &focus, &scroll))
            .content(motion::arrive(
                panel(palette, menu, &focus, &scroll),
                arrival,
                -motion::TRAVEL,
            ))
    }
}

/// Esc closes the menu; Up and Down move through the options, starting at the chosen one, and
/// scroll the one they reach into view.
fn keys(
    menu: &Menu,
    button: &FocusHandle,
    scroll: &ScrollHandle,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    let close = Rc::clone(&menu.close);
    let options: Vec<FocusHandle> = menu.choices.iter().map(|each| each.focus.clone()).collect();
    let chosen = menu.chosen();
    let (button, scroll) = (button.clone(), scroll.clone());
    move |event, window, cx| match event.keystroke.key.as_str() {
        "escape" => {
            close(cx);
            window.focus(&button, cx);
            cx.stop_propagation();
        }
        key @ ("down" | "up") => {
            let next = match options.iter().position(|each| each.is_focused(window)) {
                None => chosen,
                Some(at) if key == "down" => (at + 1).min(options.len().saturating_sub(1)),
                Some(at) => at.saturating_sub(1),
            };
            if let Some(option) = options.get(next) {
                window.focus(option, cx);
                scroll.scroll_to_item(next);
            }
            cx.stop_propagation();
        }
        _ => {}
    }
}

/// The floating surface: the options in a list box that scrolls past `MENU_LIST_MAX_HEIGHT`.
fn panel(palette: Palette, menu: Menu, button: &FocusHandle, scroll: &ScrollHandle) -> Div {
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
    let rows: Vec<AnyElement> = choices
        .into_iter()
        .map(|choice| option_row(palette, choice, &close, button))
        .collect();
    div()
        .min_w(px(size::MENU_MIN_WIDTH))
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
