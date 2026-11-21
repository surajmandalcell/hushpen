//! "Share anonymous usage" in Settings > Updates and its "See what's sent" dialog, which shows
//! the exact request the first update check of a day sends and the same data in words (D-56).

use super::{Settings, slots};
use crate::theme::{self, Palette, SECONDARY, StyledType, TOOLBAR_TITLE, radius, size, space};
use crate::ui::{self, Kind, Notice};
use crate::updater::SHARE_USAGE;
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::{
    AnyElement, App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

fn id(name: &str) -> SharedString {
    hook::id("settings", &format!("updates.usage.{name}"))
}

/// The dialog's open state and focus handles. `trap` keeps Tab inside it.
pub struct UsageSheet {
    open: bool,
    trap: FocusHandle,
    done: FocusHandle,
}

impl UsageSheet {
    pub fn new(cx: &mut App) -> Self {
        Self {
            open: false,
            trap: cx.focus_handle(),
            done: cx.focus_handle().tab_stop(true),
        }
    }
}

impl Settings {
    pub fn toggle_share_usage(&mut self, cx: &mut Context<Self>) {
        self.flip(SHARE_USAGE, cx);
    }

    /// Opens the dialog with the focus on Done.
    pub fn open_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.usage.open = true;
        self.picker = None;
        window.focus(&self.usage.done, cx);
        hook::record_event("settings", "usage sheet");
        cx.notify();
    }

    /// Closes the dialog and gives the focus back to "See what's sent".
    pub fn close_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.usage.open {
            self.dismiss_usage(cx);
            window.focus(&self.control_focus[slots::USAGE_SENT], cx);
        }
    }

    /// Closes the dialog without moving the focus, for a view change that hides it.
    pub fn dismiss_usage(&mut self, cx: &mut Context<Self>) {
        if self.usage.open {
            self.usage.open = false;
            hook::record_event("settings", "usage sheet closed");
            cx.notify();
        }
    }
}

/// The dialog, or nothing while it is closed. It comes in like every sheet.
pub fn render(
    settings: &Entity<Settings>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let me = settings.read(cx);
    if !me.usage.open {
        return None;
    }
    let request = me
        .updater
        .as_ref()
        .map(|updater| updater.read(cx).daily_request());
    let (trap, done) = (me.usage.trap.clone(), me.usage.done.clone());
    let arrival = motion::enter(id("arrival"), motion::BASE, window, cx);
    let palette = theme::palette(cx);
    let close = {
        let settings = settings.clone();
        move |window: &mut Window, cx: &mut App| {
            settings.update(cx, |me, cx| me.close_usage(window, cx));
        }
    };
    let on_key = {
        let close = close.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                close(window, cx);
                cx.stop_propagation();
            }
        }
    };
    let body = match request {
        Some((url, Some(usage))) => {
            let json = serde_json::to_string_pretty(&usage.to_json()).unwrap_or_default();
            vec![
                ui::group(id("request"), palette, vec![text_row(palette, "url", url)])
                    .into_any_element(),
                ui::group(id("data"), palette, vec![text_row(palette, "json", json)])
                    .into_any_element(),
            ]
        }
        _ => vec![
            ui::group(
                id("off"),
                palette,
                vec![ui::notice_row(
                    palette,
                    id("nothing"),
                    "Nothing is sent. Update checks still send the app version.".to_owned(),
                    Notice::Info,
                    None,
                )],
            )
            .into_any_element(),
        ],
    };
    let title = "What's sent";
    let header = div()
        .flex()
        .flex_col()
        .gap(px(space::XS))
        .child(
            div()
                .id(id("title"))
                .test_support()
                .role(Role::Heading)
                .aria_label(title)
                .text_token(TOOLBAR_TITLE)
                .text_color(palette.label)
                .child(title),
        )
        .child(
            div()
                .id(id("line"))
                .test_support()
                .text_token(SECONDARY)
                .text_color(palette.label_secondary)
                .child("The first update check of each day sends this for the day before."),
        );
    let sheet = div()
        .id(id("sheet"))
        .test_support()
        .role(Role::Dialog)
        .aria_label(title)
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
        .child(header)
        .children(body)
        .child(div().flex().justify_end().child(ui::button(
            palette,
            id("done"),
            "Done",
            Kind::Default,
            Some(&done),
            true,
            close,
        )));
    Some(
        div()
            .focus_trap(id("scrim"), &trap)
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

/// A row that shows `text` whole, wrapped over as many lines as it needs.
fn text_row(palette: Palette, name: &str, text: String) -> AnyElement {
    div()
        .id(id(name))
        .test_support()
        .aria_label(text.clone())
        .p(px(size::ROW_PADDING))
        .text_token(SECONDARY)
        .text_color(palette.label)
        .child(text)
        .into_any_element()
}
