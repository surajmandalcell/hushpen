//! The dictation panel on the Home view: record button, status, transcript, and language.

use super::{Dictation, Phase};
use crate::hook;
use crate::shell::Shell;
use crate::theme::{
    self, BODY_MD, BODY_SM, LABEL_MD, ROW_TITLE, StyledType, color, radius, size, space,
};
use crate::views::View;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
};
use hushpen_core::language;

const TRANSCRIPT_MIN_HEIGHT: f32 = 72.0;
const OPTION_LIST_MAX_HEIGHT: f32 = 200.0;

struct Snapshot {
    phase: Phase,
    status: String,
    transcript: String,
    notice: Option<(String, bool)>,
    can_record: bool,
    language: String,
    detail: String,
    picker_open: bool,
    picker_off: bool,
    codes: Vec<&'static str>,
}

pub fn render(
    dictation: &Entity<Dictation>,
    shell: &Entity<Shell>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let view = {
        let me = dictation.read(cx);
        let blocker = me.blocker(cx);
        let off = me.picker_off_reason(cx);
        let language = me.language();
        let detected = me
            .detected()
            .map(|code| language::label(code).to_owned())
            .filter(|_| me.phase() == Phase::Done);
        let detail = match (off, detected) {
            (Some(reason), _) => reason.to_owned(),
            (None, Some(name)) => format!("Detected: {name}"),
            (None, None) if language == language::AUTO => "Detects the language you speak".into(),
            (None, None) => String::new(),
        };
        let notice = match (me.notice(), blocker) {
            (Some(notice), _) if me.phase() != Phase::Listening => {
                Some((notice.message.clone(), false))
            }
            (_, Some(blocker)) if !me.phase().busy() => {
                Some((blocker.message, blocker.models_link))
            }
            _ => None,
        };
        Snapshot {
            phase: me.phase(),
            status: status_text(me.phase(), me.can_record(cx)),
            transcript: me.transcript().to_owned(),
            notice,
            can_record: me.can_record(cx),
            language,
            detail,
            picker_open: me.picker_open(),
            picker_off: off.is_some(),
            codes: me.picker_codes(),
        }
    };
    let (record_focus, copy_focus, models_focus, language_focus, option_focus) = {
        let me = dictation.read(cx);
        (
            me.record_focus.clone(),
            me.copy_focus.clone(),
            me.models_focus.clone(),
            me.language_focus.clone(),
            me.option_focus.clone(),
        )
    };

    let mut panel = div()
        .id(hook::id("home", "dictation"))
        .test_support()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .bg(theme::rgb_of(color::SURFACE))
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(header(dictation, &view, &record_focus, &copy_focus));
    if let Some((message, link)) = view.notice.clone() {
        panel = panel.child(notice_row(shell, message, link, &models_focus));
    }
    panel =
        panel
            .child(transcript_row(&view))
            .child(language_row(dictation, &view, &language_focus));
    if view.picker_open {
        panel = panel.child(option_list(dictation, &view, &option_focus));
    }
    panel
}

fn status_text(phase: Phase, can_record: bool) -> String {
    match phase {
        Phase::Listening => "Listening",
        Phase::Transcribing => "Transcribing",
        Phase::Done => "Done",
        Phase::NoSpeech => "No speech heard",
        Phase::Failed => "Failed",
        Phase::Idle if can_record => "Ready",
        Phase::Idle => "Not ready",
    }
    .into()
}

fn header(
    dictation: &Entity<Dictation>,
    view: &Snapshot,
    record_focus: &FocusHandle,
    copy_focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let listening = view.phase == Phase::Listening;
    let label = if listening { "Stop" } else { "Record" };
    let record = {
        let dictation = dictation.clone();
        move |cx: &mut App| {
            dictation.update(cx, |me, cx| {
                // A refused start shows its reason in the notice row.
                let _ = me.toggle(cx);
            });
        }
    };
    let copy = {
        let dictation = dictation.clone();
        move |cx: &mut App| {
            dictation.update(cx, |me, cx| {
                let _ = me.copy(cx);
            });
        }
    };
    let enabled = view.can_record && view.phase != Phase::Transcribing;
    div()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .p(px(space::LG))
        .child(icon_tile(IconName::Mic))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_token(ROW_TITLE)
                        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                        .child("Dictation"),
                )
                .child(
                    div()
                        .id(hook::id("home", "status"))
                        .test_support()
                        .role(Role::Status)
                        .aria_label(view.status.clone())
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .truncate()
                        .child(view.status.clone()),
                ),
        )
        .children(
            (!view.transcript.is_empty())
                .then(|| button(hook::id("home", "copy"), "Copy", copy_focus, true, copy)),
        )
        .child(button(
            hook::id("home", "record"),
            label,
            record_focus,
            enabled,
            record,
        ))
}

fn notice_row(
    shell: &Entity<Shell>,
    message: String,
    models_link: bool,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let open = {
        let shell = shell.clone();
        move |cx: &mut App| {
            shell.update(cx, |shell, cx| shell.select(View::Models, cx));
        }
    };
    div()
        .id(hook::id("home", "notice"))
        .test_support()
        .role(Role::Alert)
        .aria_label(message.clone())
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::STATUS_ERROR))
        .child(
            svg()
                .path(IconName::CircleAlert.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::STATUS_ERROR)),
        )
        .child(div().flex_1().min_w_0().child(message))
        .children(models_link.then(|| {
            button(
                hook::id("home", "notice.models"),
                "Open Models",
                focus,
                true,
                open,
            )
        }))
}

fn transcript_row(view: &Snapshot) -> impl IntoElement + use<> {
    let empty = view.transcript.is_empty();
    let placeholder = match view.phase {
        Phase::Listening => "Speak now. Stop when you are done.",
        Phase::Transcribing => "Transcribing your recording.",
        Phase::NoSpeech => "No speech was heard.",
        Phase::Failed => "Nothing was transcribed.",
        Phase::Idle | Phase::Done => "Your words appear here and are copied to the clipboard.",
    };
    let text: SharedString = if empty {
        placeholder.into()
    } else {
        view.transcript.clone().into()
    };
    div()
        .id(hook::id("home", "transcript"))
        .test_support()
        .role(Role::Paragraph)
        .aria_value(if empty { "" } else { &view.transcript }.to_owned())
        .min_h(px(TRANSCRIPT_MIN_HEIGHT))
        .w_full()
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(if empty {
            color::TEXT_SUBTLE
        } else {
            color::TEXT_PRIMARY
        }))
        .child(text)
}

fn language_row(
    dictation: &Entity<Dictation>,
    view: &Snapshot,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let off = view.picker_off;
    let toggle = {
        let dictation = dictation.clone();
        move |cx: &mut App| {
            dictation.update(cx, |me, cx| {
                let _ = me.toggle_picker(cx);
            });
        }
    };
    let id = hook::id("home", "language");
    hook::mark_disabled(&id, off);
    let on_click = {
        let toggle = toggle.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| toggle(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            toggle(cx);
            cx.stop_propagation();
        }
    };
    let label = language::label(&view.language);
    let tone = if off {
        color::TEXT_DISABLED
    } else {
        color::TEXT_PRIMARY
    };
    let mut trigger = div()
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .h(px(32.0))
        .px(px(10.0 - size::FOCUS_RING))
        .rounded(px(radius::MD))
        .border_2()
        .border_color(transparent_black())
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(tone))
        .child(label)
        .child(
            svg()
                .path(IconName::ChevronDown.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(tone)),
        );
    if !off {
        trigger = trigger
            .track_focus(focus)
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key);
    }
    div()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(icon_tile(IconName::Globe))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_token(ROW_TITLE)
                        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                        .child("Language"),
                )
                .child(
                    div()
                        .id(hook::id("home", "language.detail"))
                        .test_support()
                        .aria_label(view.detail.clone())
                        .text_token(BODY_SM)
                        .text_color(theme::rgb_of(color::TEXT_MUTED))
                        .truncate()
                        .child(view.detail.clone()),
                ),
        )
        .child(trigger)
}

fn option_list(
    dictation: &Entity<Dictation>,
    view: &Snapshot,
    focus: &[FocusHandle],
) -> impl IntoElement + use<> {
    let rows = view.codes.iter().enumerate().map(|(index, code)| {
        option_row(
            dictation,
            code,
            *code == view.language,
            focus[index].clone(),
        )
    });
    div()
        .id(hook::id("home", "language.list"))
        .test_support()
        .role(Role::ListBox)
        .flex()
        .flex_col()
        .max_h(px(OPTION_LIST_MAX_HEIGHT))
        .overflow_y_scroll()
        // The page behind scrolls on the same wheel event unless the list keeps it.
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .p(px(space::XS))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .children(rows)
}

fn option_row(
    dictation: &Entity<Dictation>,
    code: &'static str,
    selected: bool,
    focus: FocusHandle,
) -> impl IntoElement + use<> {
    let choose = {
        let dictation = dictation.clone();
        move |cx: &mut App| {
            dictation.update(cx, |me, cx| {
                let _ = me.set_language(code, cx);
            });
        }
    };
    let on_click = {
        let choose = choose.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| choose(cx)
    };
    let close = dictation.clone();
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        match event.keystroke.key.as_str() {
            "enter" | "space" => choose(cx),
            "escape" => close.update(cx, |me, cx| me.close_picker(cx)),
            _ => return,
        }
        cx.stop_propagation();
    };
    let label = language::label(code);
    let mut row = div()
        .id(hook::id("home", &format!("language.option.{code}")))
        .test_support()
        .track_focus(&focus)
        .role(Role::ListBoxOption)
        .aria_label(label)
        .aria_selected(selected)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .h(px(32.0))
        .px(px(space::MD - size::FOCUS_RING))
        .rounded(px(radius::CONTROL))
        .border_2()
        .border_color(transparent_black())
        .cursor_pointer()
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(label);
    if selected {
        row = row.bg(theme::rgb_of(color::SURFACE_SELECTED)).child(
            svg()
                .path(IconName::Check.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::FOCUS)),
        );
    }
    row
}

fn button<F: Fn(&mut App) + Clone + 'static>(
    id: SharedString,
    label: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    press: F,
) -> impl IntoElement + use<F> {
    hook::mark_disabled(&id, !enabled);
    let mut element = div()
        .id(id)
        .test_support()
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .flex()
        .items_center()
        .h(px(32.0))
        .px(px(10.0 - size::FOCUS_RING))
        .rounded(px(radius::MD))
        .border_2()
        .border_color(transparent_black())
        .bg(theme::rgb_of(color::SURFACE_CONTROL))
        .text_token(LABEL_MD)
        .text_color(theme::rgb_of(if enabled {
            color::TEXT_PRIMARY
        } else {
            color::TEXT_DISABLED
        }));
    if enabled {
        let on_click = {
            let press = press.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| press(cx)
        };
        let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                press(cx);
                cx.stop_propagation();
            }
        };
        element = element
            .track_focus(focus)
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key);
    }
    element.child(label)
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
