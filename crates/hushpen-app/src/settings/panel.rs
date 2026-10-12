//! The Settings view: a section bar on top, then one grouped panel per
//! section. Every control has a stable hook id: `settings.<section>.<name>`.

use super::endpoint::{EndpointForm, Test};
use super::{AiStatus, MoveState, Section, Settings, slots};
use crate::hook;
use crate::mic;
use crate::shortcuts::{self, Shortcuts};
use crate::theme::{
    self, BODY_MD, BODY_SM, LABEL_MD, ROW_TITLE, StyledType, color, radius, size, space,
};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
};
use hushpen_core::endpoint::Preset;

use slots::{
    AI_CLEANUP, AI_DOWNLOAD, AI_MODEL, AI_MODEL_OPTION, AI_PROVIDER, AI_PROVIDER_OPTION,
    CHANGE_FOLDER, ENDPOINT_KEY_CLEAR, ENDPOINT_PRESET, ENDPOINT_PRESET_OPTION, ENDPOINT_TEST,
    FLOWBAR, FLOWBAR_IDLE, HIDDEN, LOGIN, MIC, POSITION, POSITION_OPTION, RETENTION,
    RETENTION_OPTION, RULES, SOUNDS,
};

pub fn render(
    settings: &Entity<Settings>,
    shortcuts: Option<&Entity<Shortcuts>>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (section, notice, focus, section_focus) = {
        let me = settings.read(cx);
        (
            me.section,
            me.notice.clone(),
            me.control_focus.clone(),
            me.section_focus.clone(),
        )
    };
    let body = match section {
        Section::General => general(settings, &focus, cx).into_any_element(),
        Section::Shortcuts => match shortcuts {
            Some(shortcuts) => shortcuts::panel::render(shortcuts, cx).into_any_element(),
            None => muted_row(IconName::Keyboard, "Shortcuts", "Later.").into_any_element(),
        },
        Section::Audio => audio(settings, &focus, cx).into_any_element(),
        Section::Cleanup => cleanup(settings, &focus, cx).into_any_element(),
        Section::FlowBar => flow_bar(settings, &focus, cx).into_any_element(),
        Section::Storage => storage_section(settings, &focus, cx).into_any_element(),
        Section::Updates => updates().into_any_element(),
    };
    // Esc with an open picker closes it and focuses the trigger again.
    let on_escape = {
        let settings = settings.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key.as_str() == "escape" {
                let focus = settings.update(cx, |me, cx| me.close_picker(cx));
                if let Some(focus) = focus {
                    window.focus(&focus, cx);
                    cx.stop_propagation();
                }
            }
        }
    };
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w_full()
        .on_key_down(on_escape)
        .child(section_bar(settings, section, &section_focus));
    if let Some(message) = notice {
        page = page.child(notice_row(message));
    }
    page.child(body)
}

fn section_bar(
    settings: &Entity<Settings>,
    active: Section,
    focus: &[FocusHandle],
) -> impl IntoElement + use<> {
    let tabs = Section::ALL.iter().enumerate().map(|(index, section)| {
        let select = {
            let settings = settings.clone();
            let section = *section;
            move |cx: &mut App| {
                settings.update(cx, |me, cx| me.select_section(section, cx));
            }
        };
        let on_click = {
            let select = select.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| select(cx)
        };
        let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                select(cx);
                cx.stop_propagation();
            }
        };
        let selected = *section == active;
        let mut tab = div()
            .id(hook::id("settings", &format!("section.{}", section.key())))
            .test_support()
            .track_focus(&focus[index])
            .role(Role::Button)
            .aria_label(section.title())
            .aria_selected(selected)
            .flex_none()
            .flex()
            .items_center()
            .h(px(28.0))
            // 6px of gutter a side: the seven tabs fit the 520px content column on one row.
            .px(px(8.0 - size::FOCUS_RING))
            .rounded(px(radius::CONTROL))
            .border_2()
            .border_color(transparent_black())
            .text_token(LABEL_MD)
            .text_color(theme::rgb_of(if selected {
                color::TEXT_PRIMARY
            } else {
                color::TEXT_MUTED
            }))
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key)
            .child(section.title());
        if selected {
            tab = tab.bg(theme::rgb_of(color::SURFACE_SELECTED));
        }
        tab
    });
    div()
        .id(hook::id("settings", "sections"))
        .test_support()
        .flex()
        .flex_wrap()
        .gap(px(space::XXS))
        .p(px(space::XXS))
        .w_full()
        .rounded(px(radius::MD))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .bg(theme::rgb_of(color::SURFACE_ELEVATED))
        .children(tabs)
}

fn surface(id: gpui_kit::SharedString) -> impl IntoElement + ParentElement + Styled {
    div()
        .id(id)
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

/// A row with an icon, a title and detail, and a trailing element.
fn row(
    icon: IconName,
    title: &'static str,
    detail: String,
    trailing: impl IntoElement,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(icon_tile(icon))
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
        .child(trailing)
}

fn switch(on: bool) -> impl IntoElement {
    let knob = div()
        .size(px(14.0))
        .rounded(px(radius::FULL))
        .bg(theme::rgb_of(color::TEXT_PRIMARY));
    let track = div()
        .w(px(36.0))
        .h(px(20.0))
        .flex_none()
        .flex()
        .items_center()
        .px(px(3.0))
        .rounded(px(radius::FULL))
        .border_1()
        .border_color(theme::rgb_of(color::BORDER_CONTROL))
        .bg(theme::rgb_of(if on {
            color::ACCENT_BLUE
        } else {
            color::SURFACE_CONTROL
        }));
    if on {
        track.justify_end().child(knob)
    } else {
        track.justify_start().child(knob)
    }
}

/// A row whose trailing element is a switch. The whole control is the switch
/// button with the stable id.
#[allow(clippy::too_many_arguments)]
fn toggle_row(
    id: &str,
    icon: IconName,
    title: &'static str,
    detail: &str,
    on: bool,
    enabled: bool,
    focus: &FocusHandle,
    toggle: impl Fn(&mut App) + 'static + Clone,
) -> gpui_kit::AnyElement {
    let element_id = hook::id("settings", id);
    hook::mark_disabled(&element_id, !enabled);
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
    let mut control = div()
        .id(element_id)
        .test_support()
        .role(Role::Switch)
        .aria_label(format!("{title}, {}", if on { "on" } else { "off" }))
        .flex_none()
        .flex()
        .items_center()
        .h(px(32.0))
        .px(px(space::SM - size::FOCUS_RING))
        .rounded(px(radius::MD))
        .border_2()
        .border_color(transparent_black())
        .child(switch(on));
    if enabled {
        control = control
            .track_focus(focus)
            .cursor_pointer()
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key);
    }
    row(icon, title, detail.to_owned(), control).into_any_element()
}

/// The trigger of a picker row: the current value and a chevron.
#[allow(clippy::too_many_arguments)]
fn picker_trigger(
    id: &str,
    icon: IconName,
    title: &'static str,
    detail: &str,
    current: String,
    enabled: bool,
    focus: &FocusHandle,
    toggle: impl Fn(&mut App) + 'static + Clone,
) -> gpui_kit::AnyElement {
    let element_id = hook::id("settings", id);
    hook::mark_disabled(&element_id, !enabled);
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
    let mut trigger = div()
        .id(element_id)
        .test_support()
        .role(Role::Button)
        .aria_label(format!("{title}: {current}"))
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
        .text_color(theme::rgb_of(color::TEXT_PRIMARY))
        .max_w(px(220.0))
        .child(div().flex_1().min_w_0().truncate().child(current))
        .child(
            svg()
                .path(IconName::ChevronDown.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::TEXT_MUTED)),
        );
    if enabled {
        trigger = trigger
            .track_focus(focus)
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key);
    }
    row(icon, title, detail.to_owned(), trigger).into_any_element()
}

/// One option of an open picker.
fn picker_option(
    id: &str,
    value: &str,
    label: String,
    selected: bool,
    focus: &FocusHandle,
    choose: impl Fn(&mut App) + 'static + Clone,
) -> gpui_kit::AnyElement {
    let on_click = {
        let choose = choose.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| choose(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            choose(cx);
            cx.stop_propagation();
        }
    };
    let id = hook::id("settings", &format!("{id}.option.{value}"));
    let mut element = div()
        .id(id)
        .test_support()
        .track_focus(focus)
        .role(Role::ListBoxOption)
        .aria_label(label.clone())
        .aria_selected(selected)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .h(px(32.0))
        .px(px(space::MD - size::FOCUS_RING))
        .mx(px(space::SM))
        .rounded(px(radius::CONTROL))
        .border_2()
        .border_color(transparent_black())
        .text_token(BODY_MD)
        .text_color(theme::rgb_of(color::TEXT_BODY))
        .cursor_pointer()
        .hover(|style| style.bg(theme::rgb_of(color::SURFACE_ROW_HOVER)))
        .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
        .on_click(on_click)
        .on_key_down(on_key)
        .child(div().flex_1().min_w_0().truncate().child(label));
    element = if selected {
        element.child(
            svg()
                .path(IconName::Check.path())
                .size(px(14.0))
                .flex_none()
                .text_color(theme::rgb_of(color::FOCUS)),
        )
    } else {
        element.child(div().size(px(14.0)).flex_none())
    };
    element.into_any_element()
}

/// A row that names a control that does nothing yet.
fn muted_row(icon: IconName, title: &'static str, detail: &str) -> impl IntoElement {
    row(
        icon,
        title,
        detail.to_owned(),
        div()
            .flex_none()
            .text_token(LABEL_MD)
            .text_color(theme::rgb_of(color::TEXT_DISABLED))
            .child("Later"),
    )
}

fn notice_row(message: String) -> impl IntoElement {
    div()
        .id(hook::id("settings", "notice"))
        .test_support()
        .role(Role::Alert)
        .flex()
        .items_center()
        .gap(px(space::SM))
        .px(px(space::LG))
        .py(px(space::MD))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .bg(theme::rgb_of(color::SURFACE))
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
}

// ---- Sections ----

fn general(
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (launch_on, hidden_on) = {
        let me = settings.read(cx);
        (me.launch_at_login(), me.stored_bool("startup.startHidden"))
    };
    let flip_login = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_launch_at_login(cx))
    };
    let flip_hidden = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_start_hidden(cx))
    };
    surface(hook::id("settings", "general"))
        .child(div().h(px(space::XS)))
        .child(toggle_row(
            "general.launchAtLogin",
            IconName::Rocket,
            "Launch at login",
            "Hushpen starts when you log in.",
            launch_on,
            true,
            &focus[LOGIN],
            flip_login,
        ))
        .child(toggle_row(
            "general.startHidden",
            IconName::EyeOff,
            "Start hidden",
            "A start at login shows only the tray icon.",
            hidden_on,
            true,
            &focus[HIDDEN],
            flip_hidden,
        ))
        .child(div().h(px(space::XS)))
}

fn audio(
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (picker_open, sounds_on, current, options, mic_focus) = {
        let me = settings.read(cx);
        let mic = me.mic.read(cx);
        let saved = mic.saved();
        let current = match saved {
            hushpen_audio::DEFAULT_ID => mic
                .default_name()
                .map(|name| format!("Default ({name})"))
                .unwrap_or_else(|| "Default".to_owned()),
            id => mic
                .devices()
                .iter()
                .find(|device| device.id == id)
                .map(|device| device.name.clone())
                .unwrap_or_else(|| id.to_owned()),
        };
        let options: Vec<(String, String, bool)> = std::iter::once((
            hushpen_audio::DEFAULT_ID.to_owned(),
            mic.default_name()
                .map(|name| format!("Default ({name})"))
                .unwrap_or_else(|| "Default".to_owned()),
            saved == hushpen_audio::DEFAULT_ID,
        ))
        .chain(
            mic.devices()
                .iter()
                .take(mic::MAX_DEVICE_ROWS)
                .map(|device| (device.id.clone(), device.name.clone(), saved == device.id)),
        )
        .collect();
        (
            me.picker == Some("audio.mic"),
            me.stored_bool("audio.cueSounds"),
            current,
            options,
            mic.row_focus.clone(),
        )
    };
    let open_picker = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_picker("audio.mic", cx))
    };
    let flip_sounds = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_sounds(cx))
    };
    let mut panel = surface(hook::id("settings", "audio"))
        .child(div().h(px(space::XS)))
        .child(picker_trigger(
            "audio.mic",
            IconName::Mic,
            "Microphone",
            "The microphone dictation listens on.",
            current,
            true,
            &focus[MIC],
            open_picker,
        ));
    if picker_open {
        for (index, (id, label, selected)) in options.into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| {
                    settings.update(cx, |me, cx| {
                        let id = id.clone();
                        let _ = me.mic.update(cx, |mic, cx| mic.select(&id, cx));
                        me.picker = None;
                        cx.notify();
                    });
                }
            };
            let handle = mic_focus
                .get(index)
                .cloned()
                .unwrap_or_else(|| focus[MIC].clone());
            panel = panel.child(picker_option(
                "audio.mic",
                &index.to_string(),
                label,
                selected,
                &handle,
                choose,
            ));
        }
    }
    panel
        .child(toggle_row(
            "audio.sounds",
            IconName::Volume2,
            "Sounds",
            "A short cue marks the start and end of a dictation.",
            sounds_on,
            true,
            &focus[SOUNDS],
            flip_sounds,
        ))
        .child(div().h(px(space::XS)))
}

fn cleanup(
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (rules_on, ai_on, provider, model, offered, status, picker_open, provider_open) = {
        let me = settings.read(cx);
        (
            me.stored_bool("cleanup.rules"),
            me.stored_bool("cleanup.llm.enabled"),
            me.ai_provider(),
            me.ai_model(),
            me.ai_offered(),
            me.ai_status(cx),
            me.picker == Some("cleanup.aiModel"),
            me.picker == Some("cleanup.provider"),
        )
    };
    let flip_rules = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_rules(cx))
    };
    let flip_ai = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_ai_cleanup(cx))
    };
    let mut panel = surface(hook::id("settings", "cleanup"))
        .child(div().h(px(space::XS)))
        .child(toggle_row(
            "cleanup.rules",
            IconName::Sparkles,
            "Clean up text",
            "Fillers and stutters are removed before the text is pasted.",
            rules_on,
            true,
            &focus[RULES],
            flip_rules,
        ))
        .child(toggle_row(
            "cleanup.ai",
            IconName::WandSparkles,
            "AI cleanup",
            "A language model fixes self-corrections. Off until you turn it on.",
            ai_on,
            true,
            &focus[AI_CLEANUP],
            flip_ai,
        ))
        .child(picker_trigger(
            "cleanup.provider",
            IconName::Network,
            "AI source",
            "Where the model runs. An endpoint is a server you choose, such as Ollama.",
            provider_label(provider).to_owned(),
            true,
            &focus[AI_PROVIDER],
            {
                let settings = settings.clone();
                move |cx: &mut App| {
                    settings.update(cx, |me, cx| me.toggle_picker("cleanup.provider", cx))
                }
            },
        ));
    if provider_open {
        for (index, value) in ["local", "endpoint"].into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_ai_provider(value, cx))
            };
            panel = panel.child(picker_option(
                "cleanup.provider",
                value,
                provider_label(value).to_owned(),
                value == provider,
                &focus[AI_PROVIDER_OPTION + index],
                choose,
            ));
        }
    }
    if provider == "endpoint" {
        let endpoint = settings.read(cx).endpoint.clone();
        if let Some(form) = endpoint {
            for element in endpoint_rows(settings, &form, focus, cx) {
                panel = panel.child(element);
            }
        }
        return panel.child(div().h(px(space::XS)));
    }
    let Some(model) = model else {
        return panel.child(div().h(px(space::XS)));
    };
    let open_picker = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_picker("cleanup.aiModel", cx))
    };
    let size = crate::models::panel::format_size(model.bytes);
    panel = panel.child(picker_trigger(
        "cleanup.aiModel",
        IconName::Cpu,
        "AI model",
        &format!("{size} download. Chosen for this computer."),
        model.name.clone(),
        true,
        &focus[AI_MODEL],
        open_picker,
    ));
    if picker_open {
        for (index, entry) in offered.iter().enumerate() {
            let id = entry.id.clone();
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_ai_model(&id, cx))
            };
            panel = panel.child(picker_option(
                "cleanup.aiModel",
                &entry.id,
                entry.name.clone(),
                entry.id == model.id,
                &focus[AI_MODEL_OPTION + index.min(1)],
                choose,
            ));
        }
    }
    let text = ai_status_text(status, ai_on);
    let can_download = ai_on && matches!(status, Some(AiStatus::NotDownloaded | AiStatus::Failed));
    let download = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.download_ai_model(cx))
    };
    panel
        .child(ai_status_row(
            text,
            can_download.then_some((&focus[AI_DOWNLOAD], download)),
        ))
        .child(div().h(px(space::XS)))
}

fn ai_status_text(status: Option<AiStatus>, on: bool) -> String {
    match status {
        None => "No AI model is available.".into(),
        Some(AiStatus::NotDownloaded) if on => {
            "Not downloaded. Dictations use the rule text until it is ready.".into()
        }
        Some(AiStatus::NotDownloaded) => "Downloads when you turn AI cleanup on.".into(),
        Some(AiStatus::Downloading(Some(percent))) => format!("Downloading {percent}%"),
        Some(AiStatus::Downloading(None)) => "Downloading".into(),
        Some(AiStatus::Verifying) => "Checking the download".into(),
        Some(AiStatus::Failed) => "The file failed its check. Download it again.".into(),
        Some(AiStatus::Preparing) => "Preparing the AI model".into(),
        Some(AiStatus::Ready) if on => "Ready".into(),
        Some(AiStatus::Ready) => "Downloaded".into(),
    }
}

/// The words about the AI model's state, and a download button when the file is missing.
fn ai_status_row(
    text: String,
    download: Option<(&FocusHandle, impl Fn(&mut App) + 'static + Clone)>,
) -> impl IntoElement {
    let mut line = div()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(icon_tile(IconName::Info))
        .child(
            div()
                .id(hook::id("settings", "cleanup.aiStatus"))
                .test_support()
                .role(Role::Status)
                .aria_label(text.clone())
                .flex_1()
                .min_w_0()
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(color::TEXT_MUTED))
                .truncate()
                .child(text),
        );
    if let Some((focus, start)) = download {
        let on_click = {
            let start = start.clone();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| start(cx)
        };
        let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                start(cx);
                cx.stop_propagation();
            }
        };
        line = line.child(
            div()
                .id(hook::id("settings", "cleanup.aiDownload"))
                .test_support()
                .track_focus(focus)
                .role(Role::Button)
                .aria_label("Download the AI model")
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
                .text_color(theme::rgb_of(color::TEXT_PRIMARY))
                .cursor_pointer()
                .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
                .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
                .on_click(on_click)
                .on_key_down(on_key)
                .child("Download"),
        );
    }
    line
}

fn provider_label(provider: &str) -> &'static str {
    match provider {
        "endpoint" => "OpenAI-compatible endpoint",
        _ => "This computer",
    }
}

/// The rows for the endpoint: preset, address, model, key, and the connection test.
fn endpoint_rows(
    settings: &Entity<Settings>,
    form: &Entity<EndpointForm>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Vec<gpui_kit::AnyElement> {
    let preset_open = settings.read(cx).picker == Some("cleanup.endpointPreset");
    let (preset, key_set, test, text, url, model, key) = {
        let me = form.read(cx);
        (
            me.preset(),
            me.key_is_set(),
            me.test.clone(),
            me.test_text(),
            me.url.clone(),
            me.model.clone(),
            me.key.clone(),
        )
    };
    let mut rows = vec![picker_trigger(
        "cleanup.endpointPreset",
        IconName::Server,
        "Endpoint",
        "Ollama and LM Studio fill in the address for you.",
        preset.label().to_owned(),
        true,
        &focus[ENDPOINT_PRESET],
        {
            let settings = settings.clone();
            move |cx: &mut App| {
                settings.update(cx, |me, cx| me.toggle_picker("cleanup.endpointPreset", cx))
            }
        },
    )];
    if preset_open {
        for (index, choice) in Preset::ALL.into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_endpoint_preset(choice, cx))
            };
            rows.push(picker_option(
                "cleanup.endpointPreset",
                choice.key(),
                choice.label().to_owned(),
                choice == preset,
                &focus[ENDPOINT_PRESET_OPTION + index],
                choose,
            ));
        }
    }
    rows.push(field_row(
        "cleanup.endpointUrl",
        IconName::Globe,
        "Address",
        "Plain http works only for localhost. Anything else needs https.",
        &url,
        None,
    ));
    rows.push(field_row(
        "cleanup.endpointModel",
        IconName::Bot,
        "Model",
        "The model name the endpoint knows.",
        &model,
        None,
    ));
    let clear = key_set.then(|| {
        let form = form.clone();
        action_button(
            "cleanup.endpointKeyClear",
            "Remove",
            "Remove the API key",
            &focus[ENDPOINT_KEY_CLEAR],
            true,
            move |cx: &mut App| form.update(cx, |me, cx| me.clear_key(cx)),
        )
    });
    rows.push(field_row(
        "cleanup.endpointKey",
        IconName::EyeOff,
        "API key",
        if key_set {
            "Saved on this computer. Type a new key and press Enter to replace it."
        } else {
            "Optional. Press Enter to save it on this computer."
        },
        &key,
        clear,
    ));
    let running = test == Test::Running;
    let failed = matches!(test, Test::Failed(_));
    let run = {
        let form = form.clone();
        move |cx: &mut App| form.update(cx, |me, cx| me.test_connection(cx))
    };
    rows.push(test_row(text, failed, &focus[ENDPOINT_TEST], !running, run).into_any_element());
    rows
}

/// A row with a text field on the right.
fn field_row(
    id: &str,
    icon: IconName,
    title: &'static str,
    detail: &str,
    state: &Entity<InputState>,
    extra: Option<gpui_kit::AnyElement>,
) -> gpui_kit::AnyElement {
    let input = Input::new(state)
        .id(hook::id("settings", id))
        .aria_label(title);
    let trailing = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(div().w(px(220.0)).child(input))
        .children(extra);
    row(icon, title, detail.to_owned(), trailing).into_any_element()
}

fn action_button(
    id: &str,
    label: &'static str,
    aria: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    run: impl Fn(&mut App) + 'static + Clone,
) -> gpui_kit::AnyElement {
    let element_id = hook::id("settings", id);
    hook::mark_disabled(&element_id, !enabled);
    let on_click = {
        let run = run.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| run(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            run(cx);
            cx.stop_propagation();
        }
    };
    let mut button = div()
        .id(element_id)
        .test_support()
        .role(Role::Button)
        .aria_label(aria)
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
        .child(label);
    button = if enabled {
        button
            .track_focus(focus)
            .text_color(theme::rgb_of(color::TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key)
    } else {
        button.text_color(theme::rgb_of(color::TEXT_DISABLED))
    };
    button.into_any_element()
}

/// The words about the connection test, and the button that runs it.
fn test_row(
    text: String,
    failed: bool,
    focus: &FocusHandle,
    enabled: bool,
    run: impl Fn(&mut App) + 'static + Clone,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(space::MD))
        .px(px(space::LG))
        .py(px(space::MD))
        .border_t_1()
        .border_color(theme::rgb_of(color::DIVIDER))
        .child(icon_tile(if failed {
            IconName::CircleAlert
        } else {
            IconName::Info
        }))
        .child(
            div()
                .id(hook::id("settings", "cleanup.endpointStatus"))
                .test_support()
                .role(Role::Status)
                .aria_label(text.clone())
                .flex_1()
                .min_w_0()
                .text_token(BODY_SM)
                .text_color(theme::rgb_of(if failed {
                    color::STATUS_ERROR
                } else {
                    color::TEXT_MUTED
                }))
                .truncate()
                .child(text),
        )
        .child(action_button(
            "cleanup.endpointTest",
            "Test connection",
            "Test the connection to the endpoint",
            focus,
            enabled,
            run,
        ))
}

fn flow_bar(
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (enabled, idle, position, picker_open) = {
        let me = settings.read(cx);
        (
            me.stored_bool("overlay.enabled"),
            me.stored_bool("overlay.idleVisible"),
            me.stored_str("overlay.position"),
            me.picker == Some("flowbar.position"),
        )
    };
    let flip_enabled = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_flow_bar(cx))
    };
    let flip_idle = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_flow_bar_idle(cx))
    };
    let open_picker = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_picker("flowbar.position", cx))
    };
    let position_label = position_name(&position).to_owned();
    let mut panel = surface(hook::id("settings", "flowbar"))
        .child(div().h(px(space::XS)))
        .child(toggle_row(
            "flowbar.enabled",
            IconName::PanelBottom,
            "Show the flow bar",
            "The floating bar while you dictate.",
            enabled,
            true,
            &focus[FLOWBAR],
            flip_enabled,
        ))
        .child(toggle_row(
            "flowbar.idle",
            IconName::Clock,
            "Show when idle",
            "The bar stays on screen between dictations.",
            idle,
            true,
            &focus[FLOWBAR_IDLE],
            flip_idle,
        ))
        .child(picker_trigger(
            "flowbar.position",
            IconName::ArrowDownUp,
            "Position",
            "Where the bar sits. Dragging the bar overrides this until you pick again.",
            position_label,
            true,
            &focus[POSITION],
            open_picker,
        ));
    if picker_open {
        for (index, value) in ["bottom", "top"].into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_position(value, cx))
            };
            panel = panel.child(picker_option(
                "flowbar.position",
                value,
                position_name(value).to_owned(),
                position == value,
                &focus[POSITION_OPTION + index],
                choose,
            ));
        }
    }
    panel.child(div().h(px(space::XS)))
}

fn position_name(value: &str) -> &'static str {
    match value {
        "top" => "Top of the screen",
        _ => "Bottom of the screen",
    }
}

const RETENTION_OPTIONS: [(&str, &str); 3] = [
    ("never", "Never"),
    ("30d", "30 days"),
    ("forever", "Keep forever"),
];

fn retention_label(value: &str) -> &'static str {
    RETENTION_OPTIONS
        .iter()
        .find(|(key, _)| *key == value)
        .map(|(_, label)| *label)
        .unwrap_or("30 days")
}

fn storage_section(
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> impl IntoElement + use<> {
    let (folder, move_state, retention, picker_open, moving) = {
        let me = settings.read(cx);
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        (
            hushpen_core::paths::display(me.storage.data.root(), home.as_deref()),
            me.move_state.clone(),
            me.stored_str("history.audioRetention"),
            me.picker == Some("storage.retention"),
            me.mic.read(cx).state() == mic::CaptureState::Listening,
        )
    };
    let (change_label, change_enabled) = match &move_state {
        MoveState::Idle => ("Change…", !moving),
        MoveState::Choosing => ("Choosing…", false),
        MoveState::Failed(_) => ("Change…", !moving),
        MoveState::Restarting => ("Restarting…", false),
    };
    let change = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.change_data_folder(cx))
    };
    let open_picker = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_picker("storage.retention", cx))
    };
    let change_id = hook::id("settings", "storage.changeFolder");
    hook::mark_disabled(&change_id, !change_enabled);
    let on_click = {
        let change = change.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| change(cx)
    };
    let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
            change(cx);
            cx.stop_propagation();
        }
    };
    let mut change_button = div()
        .id(change_id)
        .test_support()
        .role(Role::Button)
        .aria_label("Change data folder")
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
        .text_color(theme::rgb_of(if change_enabled {
            color::TEXT_PRIMARY
        } else {
            color::TEXT_DISABLED
        }))
        .child(change_label);
    if change_enabled {
        change_button = change_button
            .track_focus(&focus[CHANGE_FOLDER])
            .cursor_pointer()
            .hover(|style| style.bg(theme::rgb_of(color::SURFACE_CONTROL_HOVER)))
            .focus_visible(|style| style.border_color(theme::rgb_of(color::FOCUS)))
            .on_click(on_click)
            .on_key_down(on_key);
    }
    let mut panel = surface(hook::id("settings", "storage"))
        .child(div().h(px(space::XS)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::MD))
                .px(px(space::LG))
                .py(px(space::MD))
                .border_t_1()
                .border_color(theme::rgb_of(color::DIVIDER))
                .child(icon_tile(IconName::HardDrive))
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
                                .child("Data folder"),
                        )
                        .child(
                            div()
                                .id(hook::id("settings", "storage.dataFolder"))
                                .test_support()
                                .aria_label(folder.clone())
                                .text_token(BODY_SM)
                                .text_color(theme::rgb_of(color::TEXT_MUTED))
                                .truncate()
                                .child(folder),
                        ),
                )
                .child(change_button),
        )
        .child(picker_trigger(
            "storage.retention",
            IconName::Clock,
            "Keep dictation audio",
            "How long the audio of a finished dictation stays.",
            retention_label(&retention).to_owned(),
            true,
            &focus[RETENTION],
            open_picker,
        ));
    if picker_open {
        for (index, (value, label)) in RETENTION_OPTIONS.into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_retention(value, cx))
            };
            panel = panel.child(picker_option(
                "storage.retention",
                value,
                label.to_owned(),
                retention == value,
                &focus[RETENTION_OPTION + index],
                choose,
            ));
        }
    }
    panel.child(div().h(px(space::XS)))
}

fn updates() -> impl IntoElement {
    surface(hook::id("settings", "updates"))
        .child(div().h(px(space::XS)))
        .child(muted_row(
            IconName::Download,
            "Automatic updates",
            "Update checks and installs land in a later release.",
        ))
        .child(div().h(px(space::XS)))
}
