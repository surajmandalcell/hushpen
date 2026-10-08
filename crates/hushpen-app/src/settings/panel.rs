//! The Settings view: a section bar on top, then one grouped panel per
//! section. Every control has a stable hook id: `settings.<section>.<name>`.

use super::{MoveState, Section, Settings};
use crate::hook;
use crate::mic;
use crate::shortcuts::{self, Shortcuts};
use crate::theme::{
    self, BODY_MD, BODY_SM, LABEL_MD, ROW_TITLE, StyledType, color, radius, size, space,
};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px, svg,
    transparent_black,
};

// Focus slots in `Settings::control_focus`.
const LOGIN: usize = 0;
const HIDDEN: usize = 1;
const MIC: usize = 2;
const SOUNDS: usize = 3;
const RULES: usize = 4;
const FLOWBAR: usize = 5;
const FLOWBAR_IDLE: usize = 6;
const POSITION: usize = 7;
const CHANGE_FOLDER: usize = 8;
const RETENTION: usize = 9;
const RETENTION_OPTION: usize = 10; // ..=12
const POSITION_OPTION: usize = 13; // ..=14

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
    let mut page = div()
        .flex()
        .flex_col()
        .gap(px(space::LG))
        .w_full()
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
            .px(px(10.0 - size::FOCUS_RING))
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
    let rules_on = settings.read(cx).stored_bool("cleanup.rules");
    let flip_rules = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_rules(cx))
    };
    surface(hook::id("settings", "cleanup"))
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
        .child(muted_row(
            IconName::WandSparkles,
            "AI cleanup",
            "Rewrite with a local model.",
        ))
        .child(div().h(px(space::XS)))
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
        .child(row(
            IconName::HardDrive,
            "Data folder",
            folder,
            change_button,
        ))
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
