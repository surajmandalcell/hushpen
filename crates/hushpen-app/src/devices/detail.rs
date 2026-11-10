//! The details of one paired device: when it unloads its models, what Hushpen knows about it,
//! its name, and Remove Device, which asks once in a row of its own.

use super::panel;
use super::sheet::{self, Page};
use super::{Devices, Picker, Request, focus};
use crate::hook;
use crate::theme::{Palette, size, space};
use crate::ui::{self, Kind};
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Window, div,
    px,
};
use hushpen_core::os::Os;
use hushpen_core::remote::{DeviceKind, UnloadKind, UnloadRule, allowed_rules};
use hushpen_store::settings::{RemoteDevice, Transport};

/// The idle times the pop-up offers, as in the Engine settings.
const IDLE_MINUTES: [u32; 4] = [5, 15, 30, 60];
/// The idle time of a device that starts to unload when idle.
const DEFAULT_IDLE: u32 = 15;

pub(super) fn render(
    palette: Palette,
    devices: &Entity<Devices>,
    id: &str,
    cx: &App,
) -> Option<Page> {
    let state = devices.read(cx);
    let device = state.device(id)?;
    let name = device.device.name.clone();
    let mut body = Vec::new();
    body.extend(sheet::notice(palette, state));
    body.push(unload_section(palette, devices, device, cx));
    body.push(facts(palette, devices, device, cx));
    body.push(rename(palette, devices, device, cx));
    if state.forgetting {
        body.push(sheet::section(
            palette,
            "forget.group",
            vec![forget_row(palette, devices, device, cx)],
            None,
        ));
    }
    let left = (!state.forgetting).then(|| {
        let devices = devices.clone();
        ui::button(
            palette,
            hook::id("devices", "remove"),
            "Remove Device…",
            Kind::Plain,
            Some(&state.focus.get("remove", cx)),
            !state.busy,
            move |window, cx| {
                devices.update(cx, |state, cx| {
                    state.forgetting = true;
                    window.focus(&state.focus.get("forget.cancel", cx), cx);
                    cx.notify();
                })
            },
        )
        .into_any_element()
    });
    Some(Page {
        title: name.clone(),
        subtitle: about(device),
        label: format!("{name} details"),
        body,
        left,
        right: vec![sheet::done(palette, devices, cx)],
    })
}

/// What the device is and how this computer reaches it, such as "Linux server · through SSH".
fn about(device: &RemoteDevice) -> String {
    let kind = match device.device.kind {
        DeviceKind::Phone => "phone",
        DeviceKind::Server | DeviceKind::ThisComputer => "server",
    };
    let way = match device.transport {
        Transport::Lan => "on this network",
        Transport::Tailscale => "on Tailscale",
        Transport::Ssh { .. } => "through SSH",
    };
    format!("{} {kind} · {way}", system(device.device.os))
}

fn system(os: Os) -> &'static str {
    match os {
        Os::Mac => "macOS",
        Os::Linux => "Linux",
        Os::Windows => "Windows",
        Os::Ios => "iOS",
        Os::Android => "Android",
    }
}

fn rule_label(kind: UnloadKind) -> &'static str {
    match kind {
        UnloadKind::KeepLoaded => "Keep in memory",
        UnloadKind::AfterIdle => "Unload when idle",
        UnloadKind::WhenMemoryLow => "Unload when memory is low",
        UnloadKind::WhileServerModeOpen => "Only while Server mode is open",
    }
}

fn minutes(count: u32) -> String {
    if count == 1 {
        "1 minute".to_owned()
    } else {
        format!("{count} minutes")
    }
}

/// When idle, with only the rules the system of the device allows, then Idle time while the
/// device unloads when idle.
fn unload_section(
    palette: Palette,
    devices: &Entity<Devices>,
    device: &RemoteDevice,
    cx: &App,
) -> AnyElement {
    let state = devices.read(cx);
    let rule = device.unload_rule;
    let allowed = allowed_rules(device.device.os);
    let id = device.device.id.clone();
    let menu = (state.menu == Some(Picker::Rule)).then(|| {
        allowed.iter().fold(
            ui::Menu::new(hook::id("devices", "rule.list"), sheet::close_menu(devices)),
            |menu, &kind| {
                let option = format!("rule.{kind:?}");
                let next = match kind {
                    UnloadKind::KeepLoaded => UnloadRule::KeepLoaded,
                    UnloadKind::AfterIdle => match rule {
                        UnloadRule::AfterIdle(minutes) => UnloadRule::AfterIdle(minutes),
                        _ => UnloadRule::AfterIdle(DEFAULT_IDLE),
                    },
                    UnloadKind::WhenMemoryLow => UnloadRule::WhenMemoryLow,
                    UnloadKind::WhileServerModeOpen => UnloadRule::WhileServerModeOpen,
                };
                let (devices, id) = (devices.clone(), id.clone());
                menu.option(
                    hook::id("devices", &option),
                    rule_label(kind),
                    None,
                    kind == rule.kind(),
                    &state.focus.get(&option, cx),
                    move |_, cx| {
                        devices.update(cx, |state, cx| {
                            let _ = state.unload(&id, next, cx);
                        })
                    },
                )
            },
        )
    });
    let mut rows = vec![ui::menu_row(
        palette,
        "What the device does with loaded models",
        ui::PopUp {
            id: hook::id("devices", "rule"),
            label: "When idle",
            current: rule_label(rule.kind()).to_owned(),
            enabled: allowed.len() > 1 && !state.busy,
            focus: &state.focus.get("rule", cx),
            menu,
        },
        sheet::toggle(devices, Picker::Rule),
    )];
    if let UnloadRule::AfterIdle(current) = rule {
        let mut choices = IDLE_MINUTES.to_vec();
        if !choices.contains(&current) {
            choices.push(current);
            choices.sort_unstable();
        }
        let menu = (state.menu == Some(Picker::IdleTime)).then(|| {
            choices.iter().fold(
                ui::Menu::new(
                    hook::id("devices", "idleTime.list"),
                    sheet::close_menu(devices),
                ),
                |menu, &count| {
                    let option = format!("idleTime.{count}");
                    let (devices, id) = (devices.clone(), id.clone());
                    menu.option(
                        hook::id("devices", &option),
                        minutes(count),
                        None,
                        count == current,
                        &state.focus.get(&option, cx),
                        move |_, cx| {
                            devices.update(cx, |state, cx| {
                                let _ = state.unload(&id, UnloadRule::AfterIdle(count), cx);
                            })
                        },
                    )
                },
            )
        });
        rows.push(ui::menu_row(
            palette,
            "How long it waits before it unloads",
            ui::PopUp {
                id: hook::id("devices", "idleTime"),
                label: "Idle time",
                current: minutes(current),
                enabled: !state.busy,
                focus: &state.focus.get("idleTime", cx),
                menu,
            },
            sheet::toggle(devices, Picker::IdleTime),
        ));
    }
    sheet::section(
        palette,
        "detail.unload",
        rows,
        Some("Asleep on the lists means this rule unloaded the models.".into()),
    )
}

/// The status with Check Now, how this computer reaches the device, and what it runs.
fn facts(
    palette: Palette,
    devices: &Entity<Devices>,
    device: &RemoteDevice,
    cx: &App,
) -> AnyElement {
    let state = devices.read(cx);
    let reading = panel::reading(palette, state, &device.device.id);
    let check = {
        let devices = devices.clone();
        ui::button(
            palette,
            hook::id("devices", "probe"),
            "Check Now",
            Kind::Plain,
            Some(&state.focus.get("probe", cx)),
            state.settings.enabled && !state.busy,
            move |_, cx| devices.update(cx, |state, cx| state.request(Request::Probe, cx)),
        )
    };
    let status = ui::labeled_row(
        palette,
        hook::id("devices", "detail.status.row"),
        "Status",
        None,
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .child(panel::latency_cell(palette, "detail", reading.latency))
            .child(panel::status_cell(palette, "detail", &reading))
            .child(check),
    );
    let (title, address) = match &device.transport {
        Transport::Ssh { host } => ("SSH host", host.clone()),
        Transport::Lan | Transport::Tailscale => ("Address", device.address.clone()),
    };
    let serves = device.device.serves;
    let runs = match (serves.speech, serves.cleanup) {
        (true, true) => "Speech to Text and Text cleanup",
        (true, false) => "Speech to Text",
        (false, true) => "Text cleanup",
        (false, false) => "Nothing yet",
    };
    sheet::section(
        palette,
        "detail.facts",
        vec![
            status,
            ui::fact_row(
                palette,
                hook::id("devices", "detail.address.row"),
                hook::id("devices", "detail.address"),
                title,
                Some(address),
                None,
            ),
            ui::fact_row(
                palette,
                hook::id("devices", "detail.runs.row"),
                hook::id("devices", "detail.runs"),
                "Runs",
                Some(runs.to_owned()),
                None,
            ),
        ],
        None,
    )
}

/// The name of the device with Rename after the field.
fn rename(
    palette: Palette,
    devices: &Entity<Devices>,
    device: &RemoteDevice,
    cx: &App,
) -> AnyElement {
    let state = devices.read(cx);
    let save = {
        let (devices, id) = (devices.clone(), device.device.id.clone());
        ui::button(
            palette,
            hook::id("devices", "rename"),
            "Rename",
            Kind::Plain,
            Some(&state.focus.get("rename", cx)),
            !state.busy,
            move |_, cx| {
                devices.update(cx, |state, cx| {
                    let name = state.name.read(cx).value().trim().to_owned();
                    if !name.is_empty() {
                        let _ = state.rename(&id, &name, cx);
                    }
                })
            },
        )
    };
    let trailing = div()
        .flex()
        .items_center()
        .gap(px(space::SM))
        .child(div().w(px(size::FIELD_WIDTH)).child(ui::field(
            palette,
            hook::id("devices", "name"),
            "Name",
            &state.name,
        )))
        .child(save);
    sheet::section(
        palette,
        "detail.name",
        vec![ui::labeled_row(
            palette,
            hook::id("devices", "name.row"),
            "Name",
            None,
            trailing,
        )],
        None,
    )
}

/// The question Remove Device asks, with Cancel and Remove.
fn forget_row(
    palette: Palette,
    devices: &Entity<Devices>,
    device: &RemoteDevice,
    cx: &App,
) -> AnyElement {
    let state = devices.read(cx);
    let cancel = {
        let devices = devices.clone();
        move |window: &mut Window, cx: &mut App| {
            devices.update(cx, |state, cx| {
                state.forgetting = false;
                window.focus(&state.focus.get("remove", cx), cx);
                cx.notify();
            })
        }
    };
    let forget = {
        let (devices, id) = (devices.clone(), device.device.id.clone());
        move |window: &mut Window, cx: &mut App| {
            devices.update(cx, |state, cx| {
                // The row that opened the sheet goes with the device.
                let _ = state.focus.opener(cx);
                if state.forget(&id, cx).is_ok() {
                    window.focus(&state.focus.get(focus::ADD, cx), cx);
                }
            })
        }
    };
    ui::row(palette)
        .id(hook::id("devices", "forget.ask"))
        .test_support()
        .child(ui::named_lines(
            palette,
            hook::id("devices", "forget.ask.title"),
            format!("Remove \u{201c}{}\u{201d}?", device.device.name),
            Some((
                hook::id("devices", "forget.ask.detail"),
                "Pair it again to use it.".to_owned(),
            )),
        ))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::SM))
                .child(ui::button(
                    palette,
                    hook::id("devices", "forget.cancel"),
                    "Cancel",
                    Kind::Plain,
                    Some(&state.focus.get("forget.cancel", cx)),
                    true,
                    cancel,
                ))
                .child(ui::button(
                    palette,
                    hook::id("devices", "forget"),
                    "Remove",
                    Kind::Default,
                    Some(&state.focus.get("forget", cx)),
                    !state.busy,
                    forget,
                )),
        )
        .into_any_element()
}
