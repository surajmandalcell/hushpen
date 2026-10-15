//! The Settings view: a row of section tabs on top, then one grouped section. Every control has
//! a stable hook id: `settings.<section>.<name>`.

use super::endpoint::{EndpointForm, Test};
use super::{AiStatus, MoveState, Section, Settings, slots};
use crate::hook;
use crate::mic;
use crate::shortcuts::{self, Shortcuts};
use crate::theme::{self, BODY, Palette, StyledType, radius, size, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    AnyElement, App, ClickEvent, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Role, StatefulInteractiveElement, Styled, Window, div, px,
    transparent_black,
};
use hushpen_core::endpoint::Preset;

use slots::{
    AI_CLEANUP, AI_DOWNLOAD, AI_MODEL, AI_MODEL_OPTION, AI_PROVIDER, AI_PROVIDER_OPTION,
    CHANGE_FOLDER, ENDPOINT_KEY_CLEAR, ENDPOINT_PRESET, ENDPOINT_PRESET_OPTION, ENDPOINT_TEST,
    FLOWBAR, FLOWBAR_IDLE, HIDDEN, LOGIN, MIC, POSITION, POSITION_OPTION, RETENTION,
    RETENTION_OPTION, RULES, SOUNDS, UPDATES_AUTO, UPDATES_CHECK, UPDATES_NOTES, UPDATES_NOW,
};

/// The rows of a section and the one line of help under them.
struct Body {
    rows: Vec<AnyElement>,
    footer: Option<&'static str>,
}

pub fn render(
    settings: &Entity<Settings>,
    shortcuts: Option<&Entity<Shortcuts>>,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let palette = theme::palette(cx);
    let (section, notice, focus, section_focus) = {
        let me = settings.read(cx);
        (
            me.section,
            me.notice.clone(),
            me.control_focus.clone(),
            me.section_focus.clone(),
        )
    };
    let body: AnyElement = match section {
        Section::Shortcuts => match shortcuts {
            Some(shortcuts) => shortcuts::panel::render(shortcuts, cx).into_any_element(),
            None => section_of(
                palette,
                section,
                Body {
                    rows: vec![later_row(palette, "shortcuts", "Shortcuts", "Later.")],
                    footer: None,
                },
            ),
        },
        Section::General => section_of(palette, section, general(palette, settings, &focus, cx)),
        Section::Audio => section_of(palette, section, audio(palette, settings, &focus, cx)),
        Section::Cleanup => section_of(palette, section, cleanup(palette, settings, &focus, cx)),
        Section::FlowBar => section_of(palette, section, flow_bar(palette, settings, &focus, cx)),
        Section::Storage => section_of(palette, section, storage(palette, settings, &focus, cx)),
        Section::Updates => section_of(palette, section, updates(palette, settings, &focus, cx)),
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
        .child(section_bar(palette, settings, section, &section_focus));
    if let Some(message) = notice {
        page = page.child(ui::plain_group(
            hook::id("settings", "notice.group"),
            palette,
            vec![ui::note_row(
                palette,
                hook::id("settings", "notice"),
                message,
                true,
            )],
        ));
    }
    page.child(body)
}

fn section_of(palette: Palette, section: Section, body: Body) -> AnyElement {
    ui::section(
        palette,
        ui::plain_group(hook::id("settings", section.key()), palette, body.rows),
        body.footer.map(|text| {
            (
                hook::id("settings", &format!("{}.help", section.key())),
                text.to_owned(),
            )
        }),
    )
    .into_any_element()
}

fn section_bar(
    palette: Palette,
    settings: &Entity<Settings>,
    active: Section,
    focus: &[FocusHandle],
) -> impl IntoElement + use<> {
    let tabs = Section::ALL.iter().enumerate().map(|(index, section)| {
        let select = {
            let settings = settings.clone();
            let section = *section;
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                settings.update(cx, |me, cx| me.select_section(section, cx));
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
            .h(px(size::NAV_ITEM_HEIGHT))
            .px(px(size::NAV_ITEM_PADDING - size::FOCUS_RING))
            .rounded(px(radius::SELECTION))
            .border_2()
            .border_color(transparent_black())
            .text_token(BODY)
            .text_color(if selected {
                palette.label
            } else {
                palette.label_secondary
            })
            .cursor_pointer()
            .hover(move |style| style.bg(palette.hover()))
            .focus_visible(move |style| style.border_color(palette.accent))
            .on_click(select)
            .child(section.title());
        if selected {
            tab = tab.bg(palette.selection);
        }
        tab
    });
    div()
        .id(hook::id("settings", "sections"))
        .test_support()
        .flex()
        .flex_wrap()
        .gap(px(space::XXS))
        .w_full()
        .children(tabs)
}

/// A row for a control that does nothing yet.
fn later_row(palette: Palette, id: &str, title: &'static str, detail: &str) -> AnyElement {
    ui::labeled_row(
        palette,
        hook::id("settings", &format!("{id}.row")),
        title,
        Some(detail.to_owned()),
        ui::value(palette)
            .text_color(palette.label_tertiary)
            .child("Later"),
    )
}

/// A row with a switch on the right. `toggle` runs when the switch is pressed.
fn toggle_row(
    palette: Palette,
    id: &str,
    title: &'static str,
    detail: &str,
    on: bool,
    focus: &FocusHandle,
    toggle: impl Fn(&mut App) + 'static,
) -> AnyElement {
    ui::labeled_row(
        palette,
        hook::id("settings", &format!("{id}.row")),
        title,
        Some(detail.to_owned()),
        ui::switch(
            palette,
            hook::id("settings", id),
            title,
            on,
            true,
            focus,
            move |_, cx| toggle(cx),
        ),
    )
}

/// A row with a pop-up button on the right that shows the current value.
fn picker_row(
    palette: Palette,
    id: &str,
    title: &'static str,
    detail: &str,
    current: String,
    focus: &FocusHandle,
    toggle: impl Fn(&mut App) + 'static,
) -> AnyElement {
    ui::labeled_row(
        palette,
        hook::id("settings", &format!("{id}.row")),
        title,
        Some(detail.to_owned()),
        ui::menu_button(
            palette,
            hook::id("settings", id),
            title,
            current,
            true,
            focus,
            move |_, cx| toggle(cx),
        ),
    )
}

/// One option of an open picker.
fn picker_option(
    palette: Palette,
    id: &str,
    value: &str,
    label: String,
    selected: bool,
    focus: &FocusHandle,
    choose: impl Fn(&mut App) + 'static,
) -> AnyElement {
    ui::option_row(
        palette,
        hook::id("settings", &format!("{id}.option.{value}")),
        label,
        selected,
        focus,
        move |_, cx| choose(cx),
    )
}

/// A row with a text field on the right.
fn field_row(
    palette: Palette,
    id: &str,
    title: &'static str,
    detail: &str,
    state: &Entity<InputState>,
    extra: Option<AnyElement>,
) -> AnyElement {
    let trailing = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(div().w(px(FIELD_WIDTH)).child(ui::field(
            palette,
            hook::id("settings", id),
            title,
            state,
        )))
        .children(extra);
    ui::labeled_row(
        palette,
        hook::id("settings", &format!("{id}.row")),
        title,
        Some(detail.to_owned()),
        trailing,
    )
}

const FIELD_WIDTH: f32 = 220.0;

// ---- Sections ----

fn general(
    palette: Palette,
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Body {
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
    Body {
        rows: vec![
            toggle_row(
                palette,
                "general.launchAtLogin",
                "Launch at login",
                "Hushpen starts when you log in.",
                launch_on,
                &focus[LOGIN],
                flip_login,
            ),
            toggle_row(
                palette,
                "general.startHidden",
                "Start hidden",
                "A start at login shows only the tray icon.",
                hidden_on,
                &focus[HIDDEN],
                flip_hidden,
            ),
        ],
        footer: None,
    }
}

fn audio(
    palette: Palette,
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Body {
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
    let mut rows = vec![picker_row(
        palette,
        "audio.mic",
        "Microphone",
        "The microphone dictation listens on.",
        current,
        &focus[MIC],
        open_picker,
    )];
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
            rows.push(picker_option(
                palette,
                "audio.mic",
                &index.to_string(),
                label,
                selected,
                &handle,
                choose,
            ));
        }
    }
    rows.push(toggle_row(
        palette,
        "audio.sounds",
        "Sounds",
        "A short cue marks the start and end of a dictation.",
        sounds_on,
        &focus[SOUNDS],
        flip_sounds,
    ));
    Body { rows, footer: None }
}

fn cleanup(
    palette: Palette,
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Body {
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
    let mut rows = vec![
        toggle_row(
            palette,
            "cleanup.rules",
            "Clean up text",
            "Fillers and stutters are removed before the text is pasted.",
            rules_on,
            &focus[RULES],
            flip_rules,
        ),
        toggle_row(
            palette,
            "cleanup.ai",
            "AI cleanup",
            "A language model fixes self-corrections.",
            ai_on,
            &focus[AI_CLEANUP],
            flip_ai,
        ),
        picker_row(
            palette,
            "cleanup.provider",
            "AI source",
            "Where the model runs.",
            provider_label(provider).to_owned(),
            &focus[AI_PROVIDER],
            {
                let settings = settings.clone();
                move |cx: &mut App| {
                    settings.update(cx, |me, cx| me.toggle_picker("cleanup.provider", cx))
                }
            },
        ),
    ];
    if provider_open {
        for (index, value) in ["local", "endpoint"].into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_ai_provider(value, cx))
            };
            rows.push(picker_option(
                palette,
                "cleanup.provider",
                value,
                provider_label(value).to_owned(),
                value == provider,
                &focus[AI_PROVIDER_OPTION + index],
                choose,
            ));
        }
    }
    let footer = Some(
        "AI cleanup stays off until you turn it on. An endpoint is a server you choose, such as Ollama.",
    );
    if provider == "endpoint" {
        let endpoint = settings.read(cx).endpoint.clone();
        if let Some(form) = endpoint {
            rows.extend(endpoint_rows(palette, settings, &form, focus, cx));
        }
        return Body { rows, footer };
    }
    let Some(model) = model else {
        return Body { rows, footer };
    };
    let open_picker = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_picker("cleanup.aiModel", cx))
    };
    let size = crate::models::panel::format_size(model.bytes);
    rows.push(picker_row(
        palette,
        "cleanup.aiModel",
        "AI model",
        &format!("{size} download. Chosen for this computer."),
        model.name.clone(),
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
            rows.push(picker_option(
                palette,
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
    let download = can_download.then(|| {
        let settings = settings.clone();
        ui::button(
            palette,
            hook::id("settings", "cleanup.aiDownload"),
            "Download",
            Kind::Plain,
            Some(&focus[AI_DOWNLOAD]),
            true,
            move |_, cx| settings.update(cx, |me, cx| me.download_ai_model(cx)),
        )
        .into_any_element()
    });
    rows.push(ui::status_row(
        palette,
        hook::id("settings", "cleanup.aiStatus"),
        text,
        matches!(status, Some(AiStatus::Failed)),
        download,
    ));
    Body { rows, footer }
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

fn provider_label(provider: &str) -> &'static str {
    match provider {
        "endpoint" => "OpenAI-compatible endpoint",
        _ => "This computer",
    }
}

/// The rows for the endpoint: preset, address, model, key, and the connection test.
fn endpoint_rows(
    palette: Palette,
    settings: &Entity<Settings>,
    form: &Entity<EndpointForm>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Vec<AnyElement> {
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
    let mut rows = vec![picker_row(
        palette,
        "cleanup.endpointPreset",
        "Endpoint",
        "Ollama and LM Studio fill in the address for you.",
        preset.label().to_owned(),
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
                palette,
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
        palette,
        "cleanup.endpointUrl",
        "Address",
        "Use https. Plain http works only for localhost.",
        &url,
        None,
    ));
    rows.push(field_row(
        palette,
        "cleanup.endpointModel",
        "Model",
        "The model name the endpoint knows.",
        &model,
        None,
    ));
    let clear = key_set.then(|| {
        let form = form.clone();
        ui::button(
            palette,
            hook::id("settings", "cleanup.endpointKeyClear"),
            "Remove",
            Kind::Plain,
            Some(&focus[ENDPOINT_KEY_CLEAR]),
            true,
            move |_, cx| form.update(cx, |me, cx| me.clear_key(cx)),
        )
        .into_any_element()
    });
    rows.push(field_row(
        palette,
        "cleanup.endpointKey",
        "API key",
        if key_set {
            "Saved on this computer. Press Enter to replace it."
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
        move |_: &mut Window, cx: &mut App| form.update(cx, |me, cx| me.test_connection(cx))
    };
    rows.push(ui::status_row(
        palette,
        hook::id("settings", "cleanup.endpointStatus"),
        text,
        failed,
        Some(
            ui::button(
                palette,
                hook::id("settings", "cleanup.endpointTest"),
                "Test connection",
                Kind::Plain,
                Some(&focus[ENDPOINT_TEST]),
                !running,
                run,
            )
            .into_any_element(),
        ),
    ));
    rows
}

fn flow_bar(
    palette: Palette,
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Body {
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
    let mut rows = vec![
        toggle_row(
            palette,
            "flowbar.enabled",
            "Show the flow bar",
            "The floating bar while you dictate.",
            enabled,
            &focus[FLOWBAR],
            flip_enabled,
        ),
        toggle_row(
            palette,
            "flowbar.idle",
            "Show when idle",
            "The bar stays on screen between dictations.",
            idle,
            &focus[FLOWBAR_IDLE],
            flip_idle,
        ),
        picker_row(
            palette,
            "flowbar.position",
            "Position",
            "Where the bar sits on the screen.",
            position_name(&position).to_owned(),
            &focus[POSITION],
            open_picker,
        ),
    ];
    if picker_open {
        for (index, value) in ["bottom", "top"].into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_position(value, cx))
            };
            rows.push(picker_option(
                palette,
                "flowbar.position",
                value,
                position_name(value).to_owned(),
                position == value,
                &focus[POSITION_OPTION + index],
                choose,
            ));
        }
    }
    Body {
        rows,
        footer: Some("Dragging the bar overrides the position until you pick again."),
    }
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

fn storage(
    palette: Palette,
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Body {
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
        move |_: &mut Window, cx: &mut App| settings.update(cx, |me, cx| me.change_data_folder(cx))
    };
    let open_picker = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_picker("storage.retention", cx))
    };
    let folder_row = ui::row(palette)
        .id(hook::id("settings", "storage.folder.row"))
        .test_support()
        .child(
            ui::text_column()
                .child(ui::title(palette).child("Data folder"))
                .child(
                    ui::secondary(palette)
                        .id(hook::id("settings", "storage.dataFolder"))
                        .test_support()
                        .aria_label(folder.clone())
                        .child(folder),
                ),
        )
        .child(ui::button(
            palette,
            hook::id("settings", "storage.changeFolder"),
            change_label,
            Kind::Plain,
            Some(&focus[CHANGE_FOLDER]),
            change_enabled,
            change,
        ))
        .into_any_element();
    let mut rows = vec![
        folder_row,
        picker_row(
            palette,
            "storage.retention",
            "Keep dictation audio",
            "How long the audio of a finished dictation stays.",
            retention_label(&retention).to_owned(),
            &focus[RETENTION],
            open_picker,
        ),
    ];
    if picker_open {
        for (index, (value, label)) in RETENTION_OPTIONS.into_iter().enumerate() {
            let choose = {
                let settings = settings.clone();
                move |cx: &mut App| settings.update(cx, |me, cx| me.set_retention(value, cx))
            };
            rows.push(picker_option(
                palette,
                "storage.retention",
                value,
                label.to_owned(),
                retention == value,
                &focus[RETENTION_OPTION + index],
                choose,
            ));
        }
    }
    Body { rows, footer: None }
}

fn updates(
    palette: Palette,
    settings: &Entity<Settings>,
    focus: &[FocusHandle],
    cx: &mut App,
) -> Body {
    let (check_on, auto_on, updater) = {
        let me = settings.read(cx);
        (
            me.stored_bool("updates.check"),
            me.stored_bool("updates.autoInstall"),
            me.updater.clone(),
        )
    };
    let flip_check = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_update_check(cx))
    };
    let flip_auto = {
        let settings = settings.clone();
        move |cx: &mut App| settings.update(cx, |me, cx| me.toggle_auto_install(cx))
    };
    let mut rows = vec![
        toggle_row(
            palette,
            "updates.check",
            "Check for updates",
            "Asks GitHub for a newer version at most once a day. Nothing identifies you.",
            check_on,
            &focus[UPDATES_CHECK],
            flip_check,
        ),
        toggle_row(
            palette,
            "updates.autoInstall",
            "Install updates automatically",
            "Downloads a new version for you. It starts after you restart.",
            auto_on,
            &focus[UPDATES_AUTO],
            flip_auto,
        ),
    ];
    if let Some(updater) = updater {
        rows.extend(crate::updater::rows(
            &updater,
            &focus[UPDATES_NOW],
            &focus[UPDATES_NOTES],
            cx,
        ));
    }
    Body { rows, footer: None }
}
