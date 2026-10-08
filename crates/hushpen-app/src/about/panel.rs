//! The About view: one grouped surface with the product summary, the data
//! folder, and the rows that open a folder or a repository page.

use super::{About, Target};
use crate::hook;
use crate::theme::{self, BODY_SM, LABEL_CAPS, LABEL_MD, StyledType, color, radius, size, space};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
};

pub fn render(about: &Entity<About>, cx: &mut App) -> impl IntoElement + use<> {
    let (version, folder, notice, focus) = {
        let me = about.read(cx);
        (
            me.version(),
            me.data_folder_display(),
            me.notice.clone(),
            me.focus.clone(),
        )
    };
    let surface = surface()
        .child(fact_row("VERSION", version.to_owned(), false))
        .child(fact_row("DATA FOLDER", folder, true))
        .child(action_row(
            about,
            Target::DataFolder,
            IconName::FolderOpen,
            "Open data folder",
            "The recordings, the history, and the settings.",
            &focus[0],
        ))
        .child(action_row(
            about,
            Target::LogFolder,
            IconName::Folder,
            "Open log folder",
            "The logs a bug report asks for.",
            &focus[1],
        ))
        .child(action_row(
            about,
            Target::Github,
            IconName::Github,
            "GitHub",
            "The source and the releases.",
            &focus[2],
        ))
        .child(action_row(
            about,
            Target::Issues,
            IconName::LifeBuoy,
            "Report issue",
            "Something is wrong? Tell us.",
            &focus[3],
        ))
        .child(licenses_row());
    let mut page = div().flex().flex_col().gap(px(space::LG)).w_full();
    if let Some(message) = notice {
        page = page.child(notice_row(message));
    }
    page.child(surface)
}

fn surface() -> impl IntoElement + ParentElement + Styled {
    div()
        .id(hook::id("about", "panel"))
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
}

/// A caps label with its value, the About definition-list style.
fn fact_row(key: &'static str, value: String, is_folder: bool) -> impl IntoElement {
    let id = if is_folder {
        hook::id("about", "dataFolder")
    } else {
        hook::id("about", "version")
    };
    div()
        .id(id)
        .test_support()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(
            div()
                .w(px(120.0))
                .flex_none()
                .text_token(LABEL_CAPS)
                .text_color(theme::rgb_of(color::TEXT_MUTED))
                .child(key),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                .child(value),
        )
}

fn action_row(
    about: &Entity<About>,
    target: Target,
    icon: IconName,
    title: &'static str,
    detail: &'static str,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let open = {
        let about = about.clone();
        move |cx: &mut App| about.update(cx, |me, cx| me.open(target, cx))
    };
    let on_click = {
        let open = open.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| open(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            open(cx);
            cx.stop_propagation();
        }
    };
    div()
        .id(hook::id("about", &format!("open.{}", target.key())))
        .test_support()
        .track_focus(focus)
        .role(Role::Button)
        .aria_label(title)
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG - size::FOCUS_RING))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .border_2()
        .border_color(transparent_black())
        .cursor_pointer()
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(icon_tile(icon))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_token(crate::theme::ROW_TITLE)
                        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                        .child(title),
                )
                .child(
                    div()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .truncate()
                        .child(detail),
                ),
        )
        .child(
            svg()
                .path(IconName::ArrowUpRight.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::TEXT_MUTED)),
        )
}

/// The licenses row ships with m6-licenses; until then it is visibly off.
fn licenses_row() -> impl IntoElement {
    let id = hook::id("about", "open.licenses");
    hook::mark_disabled(&id, true);
    div()
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label("Open-source licenses, later")
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(icon_tile(IconName::BookOpen))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_token(crate::theme::ROW_TITLE)
                        .text_color(theme::rgb_of(color::TEXT_DISABLED))
                        .child("Open-source licenses"),
                )
                .child(
                    div()
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_DISABLED))
                        .child("The notices of the libraries Hushpen builds on."),
                ),
        )
        .child(
            div()
                .flex_none()
                .text_token(LABEL_MD)
                .text_color(theme::rgb_of(color::TEXT_DISABLED))
                .child("Later"),
        )
}

fn icon_tile(icon: IconName) -> impl IntoElement {
    div()
        .size(px(28.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .bg(theme::rgb_of(color::SURFACE_SOFT))
        .child(
            svg()
                .path(icon.path())
                .size(px(14.0))
                .text_color(theme::rgb_of(color::TEXT_BODY)),
        )
}

fn notice_row(message: String) -> impl IntoElement {
    div()
        .id(hook::id("about", "notice"))
        .test_support()
        .role(Role::Alert)
        .px(px(space::LG))
        .py(px(space::MD))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .bg(theme::rgb_of(color::SURFACE))
        .text_token(BODY_SM)
        .text_color(theme::rgb_of(color::STATUS_ERROR))
        .child(message)
}
