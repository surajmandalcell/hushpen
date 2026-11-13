//! The sheets of Devices, drawn over the whole window like a macOS sheet: Add a Device and the
//! details of one device. Tab stays inside the sheet, Done has the focus as it opens, Esc and
//! Done close it, and the focus goes back to the control that opened it.

use super::{Devices, Picker, Sheet, add, detail, focus};
use crate::theme::{self, Palette, SECONDARY, StyledType, TOOLBAR_TITLE, radius, space};
use crate::ui::{self, Kind};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div, px,
};

/// What one sheet shows: its title, the line under it, its groups, and its buttons. `left` sits
/// at the start of the button row, `right` at its end.
pub(super) struct Page {
    pub title: String,
    pub subtitle: String,
    pub label: String,
    pub body: Vec<AnyElement>,
    pub left: Option<AnyElement>,
    pub right: Vec<AnyElement>,
}

/// The open sheet, or nothing while none is open. As it opens, the scrim fades in and the sheet
/// fades in and rises a few pixels into place.
pub fn render(devices: &Entity<Devices>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let sheet = devices.read(cx).sheet.clone()?;
    let palette = theme::palette(cx);
    let page = match &sheet {
        Sheet::Add(method) => add::render(palette, devices, *method, cx),
        Sheet::Detail(id) => detail::render(palette, devices, id, cx)?,
    };
    let arrival = motion::enter(
        hook::id("devices", "sheet.arrival"),
        motion::BASE,
        window,
        cx,
    );
    let trap = devices.read(cx).focus.trap(cx);
    let new_comparison = devices.read(cx).focus.confirmation.replace(false);
    // Each new number starts on Cancel, even if Pair had focus on an earlier try.
    let opened = window.use_keyed_state(hook::id("devices", "sheet.opened"), cx, |_, _| false);
    if !*opened.read(cx) || new_comparison {
        opened.update(cx, |opened, _| *opened = true);
        let done = devices.read(cx).focus.get(focus::DONE, cx);
        window.defer(cx, move |window, cx| window.focus(&done, cx));
    }
    let on_key = {
        let devices = devices.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                close(&devices, window, cx);
                cx.stop_propagation();
            }
        }
    };
    let buttons = div()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .children(page.left)
        .child(div().flex_1())
        .children(page.right);
    let sheet = div()
        .id(hook::id("devices", "sheet"))
        .test_support()
        .role(Role::Dialog)
        .aria_label(page.label)
        .on_key_down(on_key)
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w(px(space::SHEET_WIDTH))
        .p(px(space::XL))
        .bg(palette.content)
        .rounded(px(radius::WINDOW))
        .border(theme::hairline())
        .border_color(palette.separator)
        .child(header(palette, page.title, page.subtitle))
        .children(page.body)
        .child(buttons);
    // The trap takes over the id of the element it wraps, so it goes on the scrim and the sheet
    // keeps its own.
    Some(
        div()
            .focus_trap(hook::id("devices", "sheet.scrim"), &trap)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::faded(palette.scrim, arrival))
            .child(motion::arrive(sheet, arrival, motion::TRAVEL))
            .into_any_element(),
    )
}

/// Closes the sheet and gives the focus back to the control that opened it.
pub(super) fn close(devices: &Entity<Devices>, window: &mut Window, cx: &mut App) {
    devices.update(cx, |state, cx| {
        let opener = state.focus.opener(cx);
        state.close(cx);
        window.focus(&opener, cx);
    });
}

/// Done, the default button at the end of a sheet.
pub(super) fn done(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let devices = devices.clone();
    let focus = devices.read(cx).focus.get(focus::DONE, cx);
    ui::button(
        palette,
        hook::id("devices", "close"),
        "Done",
        Kind::Default,
        Some(&focus),
        true,
        move |window, cx| close(&devices, window, cx),
    )
    .into_any_element()
}

/// Opens the menu of `picker`, or closes it while it is open.
pub(super) fn toggle(devices: &Entity<Devices>, picker: Picker) -> impl Fn(&mut App) + 'static {
    let devices = devices.clone();
    move |cx| {
        devices.update(cx, |state, cx| {
            state.menu = (state.menu != Some(picker)).then_some(picker);
            cx.notify();
        })
    }
}

pub(super) fn close_menu(devices: &Entity<Devices>) -> impl Fn(&mut App) + 'static {
    let devices = devices.clone();
    move |cx| {
        devices.update(cx, |state, cx| {
            state.menu = None;
            cx.notify();
        })
    }
}

/// A group of the sheet with the one line of help under it.
pub(super) fn section(
    palette: Palette,
    key: &str,
    rows: Vec<AnyElement>,
    footer: Option<String>,
) -> AnyElement {
    ui::section(
        palette,
        ui::group(hook::id("devices", key), palette, rows),
        footer.map(|text| (hook::id("devices", &format!("{key}.help")), text)),
    )
    .into_any_element()
}

/// The failure the last intent left, as a group of its own at the top of the sheet.
pub(super) fn notice(palette: Palette, state: &Devices) -> Option<AnyElement> {
    let text = state.notice.clone()?;
    Some(section(
        palette,
        "sheet.notice.group",
        vec![ui::notice_row(
            palette,
            hook::id("devices", "sheet.notice"),
            text,
            ui::Notice::Failure,
            None,
        )],
        None,
    ))
}

fn header(palette: Palette, title: String, subtitle: String) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(hook::id("devices", "sheet.title"))
                .test_support()
                .role(Role::Heading)
                .aria_label(title.clone())
                .text_token(TOOLBAR_TITLE)
                .text_color(palette.label)
                .truncate()
                .child(title),
        )
        .child(
            div()
                .id(hook::id("devices", "sheet.subtitle"))
                .test_support()
                .aria_label(subtitle.clone())
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .truncate()
                .child(subtitle),
        )
}
