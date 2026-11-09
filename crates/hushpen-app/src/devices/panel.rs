//! Plain controls expose Devices state for the screen designer.

use super::{AddMethod, Devices, Request, Sheet};
use gpui_kit::TestSupportExt as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::{
    App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Window, div,
};
use hushpen_core::remote::{
    ModelKind, Pairing, Route, Status, THIS_COMPUTER, UnloadKind, UnloadRule, allowed_rules,
};

pub struct PlaceholderDevices;

impl PlaceholderDevices {
    pub fn render(devices: &Entity<Devices>, cx: &mut App) -> impl IntoElement + use<> {
        let state = devices.read(cx);
        let enabled = state.settings.enabled;
        let no_local = state.settings.no_local_models;
        let mut page = div()
            .flex()
            .flex_col()
            .child(button(
                devices,
                "devices.enabled",
                format!("Use models on other devices: {enabled}"),
                move |state, _, cx| {
                    let _ = state.set_enabled(!enabled, cx);
                },
            ))
            .child(button(
                devices,
                "devices.noLocal",
                format!("Do not load models on this computer: {no_local}"),
                move |state, _, cx| {
                    let _ = state.set_no_local(!no_local, cx);
                },
            ))
            .child(button(
                devices,
                "devices.add",
                "Add device…",
                |state, _, cx| state.open_add(cx),
            ))
            .child(button(
                devices,
                "devices.probe",
                "Check devices",
                |state, _, cx| state.request(Request::Probe, cx),
            ));
        for (kind, title, ids) in [
            (
                ModelKind::Speech,
                "Speech to Text",
                &state.settings.routes.speech,
            ),
            (
                ModelKind::Cleanup,
                "Text cleanup",
                &state.settings.routes.cleanup,
            ),
        ] {
            let mut group = div().flex().flex_col().child(title);
            let route = Route::new(ids.clone(), state.settings.no_local_models);
            for id in route.ids() {
                if id == THIS_COMPUTER {
                    group = group.child("This computer · Ready");
                    continue;
                }
                let Some(device) = state
                    .settings
                    .devices
                    .iter()
                    .find(|device| device.device.id == *id)
                else {
                    continue;
                };
                let health = state.health.get(id).map(|health| {
                    let mut health = health.clone();
                    health.tick(crate::controller::monotonic_ms());
                    health
                });
                let health = health.as_ref();
                let status = match health.map(|health| health.status).unwrap_or_default() {
                    Status::Unknown => "Unknown",
                    Status::Ready => "Ready",
                    Status::Asleep => "Asleep",
                    Status::Unreachable => "Unreachable",
                    Status::ServerModeOff => "Server mode off",
                };
                let latency = health
                    .and_then(|health| health.latency_ms)
                    .map(|ms| format!(" · {ms} ms"))
                    .unwrap_or_default();
                let mut row = div().child(format!("{} · {status}{latency}", device.device.name));
                for (up, label) in [(true, "Move up"), (false, "Move down")] {
                    let mut moved = route.clone();
                    let can_move = moved.move_device(id, up);
                    let id = id.clone();
                    row = row.child(
                        button(
                            devices,
                            format!("devices.{kind:?}.{id}.{up}"),
                            label,
                            move |state, _, cx| {
                                let _ = state.move_device(kind, &id, up, cx);
                            },
                        )
                        .disabled(!can_move),
                    );
                }
                let id = id.clone();
                group = group.child(row.child(button(
                    devices,
                    format!("devices.{kind:?}.{id}.detail"),
                    "Details…",
                    move |state, window, cx| state.open_detail(&id, window, cx),
                )));
            }
            if route.ids().is_empty() {
                group = group
                    .child("No device is ready. Add a device or allow models on this computer.");
            }
            page = page.child(group);
        }
        if let Some(notice) = &state.notice {
            page = page.child(notice.clone());
        }
        match &state.sheet {
            Some(Sheet::Add(method)) => {
                let mut sheet = div().flex().flex_col().child("Add device");
                for (method, label) in [
                    (AddMethod::Phone, "Phone on this network"),
                    (AddMethod::Ssh, "Server over SSH"),
                    (AddMethod::Tailscale, "Tailscale address"),
                ] {
                    sheet = sheet.child(button(
                        devices,
                        format!("devices.method.{method:?}"),
                        label,
                        move |state, _, cx| state.select_method(method, cx),
                    ));
                }
                match method {
                    AddMethod::Phone => {
                        sheet = sheet.child(button(
                            devices,
                            "devices.discover",
                            "Find phones",
                            |state, _, cx| state.request(Request::Discover, cx),
                        ));
                        if state.discovered.is_empty() {
                            sheet = sheet.child(
                                "No phones found. Open server mode on the phone and try again.",
                            );
                        }
                        for found in &state.discovered {
                            let device = found.device.clone();
                            sheet = sheet.child(button(
                                devices,
                                format!("devices.found.{}", device.id),
                                device.name.clone(),
                                move |state, _, cx| {
                                    state.selected_device = Some(device.clone());
                                    cx.notify();
                                },
                            ));
                        }
                        sheet = sheet
                            .child(
                                Input::new(&state.host)
                                    .id("devices.host")
                                    .aria_label("This computer's network address"),
                            )
                            .child(button(
                                devices,
                                "devices.offer",
                                "Show pairing QR",
                                |state, _, cx| {
                                    let host = state.host.read(cx).value().to_string();
                                    state.request(Request::Offer { host }, cx);
                                },
                            ));
                        if let Pairing::Offered { expires_at, .. } = state.pairing {
                            sheet = sheet.child(format!(
                                "Pairing code expires at {expires_at}. Scan it from the phone."
                            ));
                            if !state.qr.is_empty() {
                                sheet = sheet.child(
                                    div()
                                        .id("devices.qr")
                                        .test_support()
                                        .child(qr(state.qr.clone())),
                                );
                            }
                        }
                        sheet = sheet
                            .child(
                                Input::new(&state.uri)
                                    .id("devices.uri")
                                    .aria_label("Pairing link"),
                            )
                            .child(button(
                                devices,
                                "devices.pair",
                                "Pair device",
                                |state, _, cx| state.pair_input(cx),
                            ));
                    }
                    AddMethod::Ssh => {
                        if state.hosts.is_empty() {
                            sheet = sheet.child("No hosts found in your SSH config.");
                        }
                        for host in &state.hosts {
                            let host = host.clone();
                            sheet = sheet.child(button(
                                devices,
                                format!("devices.ssh.{host}"),
                                host.clone(),
                                move |state, _, cx| {
                                    state.selected_host = Some(host.clone());
                                    cx.notify();
                                },
                            ));
                        }
                        if let Some(host) = &state.selected_host {
                            sheet = sheet.child(format!("Selected: {host}"));
                        }
                        sheet = sheet.child(
                            button(devices, "devices.setupSsh", "Add server", |state, _, cx| {
                                if let Some(host) = state.selected_host.clone() {
                                    state.request(Request::SetupSsh { host }, cx);
                                }
                            })
                            .disabled(state.busy || state.selected_host.is_none()),
                        );
                    }
                    AddMethod::Tailscale => {
                        sheet = sheet.child(format!("Device system: {:?}", state.tailscale_os));
                        for (os, label) in [
                            (hushpen_core::os::Os::Linux, "Linux"),
                            (hushpen_core::os::Os::Android, "Android"),
                            (hushpen_core::os::Os::Ios, "iOS"),
                            (hushpen_core::os::Os::Mac, "macOS"),
                            (hushpen_core::os::Os::Windows, "Windows"),
                        ] {
                            sheet = sheet.child(button(
                                devices,
                                format!("devices.os.{os:?}"),
                                label,
                                move |state, _, cx| {
                                    state.tailscale_os = os;
                                    cx.notify();
                                },
                            ));
                        }
                        sheet = sheet
                            .child(
                                Input::new(&state.address)
                                    .id("devices.address")
                                    .aria_label("Tailscale address and port"),
                            )
                            .child(
                                Input::new(&state.uri)
                                    .id("devices.uri")
                                    .aria_label("Pairing link"),
                            )
                            .child(button(
                                devices,
                                "devices.pairTailscale",
                                "Pair device",
                                |state, _, cx| state.pair_input(cx),
                            ));
                    }
                }
                for row in &state.progress {
                    sheet = sheet.child(row.clone());
                }
                sheet = sheet.child(format!("Working: {}", state.busy));
                page = page.child(sheet.child(button(
                    devices,
                    "devices.close",
                    "Done",
                    |state, _, cx| state.close(cx),
                )));
            }
            Some(Sheet::Detail(id)) => {
                if let Some(device) = state
                    .settings
                    .devices
                    .iter()
                    .find(|device| device.device.id == *id)
                {
                    let mut sheet = div()
                        .flex()
                        .flex_col()
                        .child(device.device.name.clone())
                        .child(format!("Device: {}", device.device.id))
                        .child(format!("System: {:?}", device.device.os))
                        .child(format!("Kind: {:?}", device.device.kind))
                        .child(format!("Transport: {:?}", device.transport))
                        .child(format!("Address: {}", device.address))
                        .child(format!(
                            "Health: {:?}",
                            state
                                .health
                                .get(id)
                                .map(|health| (health.status, health.latency_ms))
                        ))
                        .child(
                            Input::new(&state.name)
                                .id("devices.name")
                                .aria_label("Device name"),
                        );
                    let rename_id = id.clone();
                    sheet = sheet.child(button(
                        devices,
                        "devices.rename",
                        "Rename",
                        move |state, _, cx| {
                            let name = state.name.read(cx).value().to_string();
                            let _ = state.rename(&rename_id, &name, cx);
                        },
                    ));
                    sheet = sheet.child(format!("When idle: {:?}", device.unload_rule));
                    for kind in allowed_rules(device.device.os) {
                        let rules: Vec<_> = match kind {
                            UnloadKind::KeepLoaded => {
                                vec![(UnloadRule::KeepLoaded, "Keep loaded".to_owned())]
                            }
                            UnloadKind::AfterIdle => [5, 15, 30, 60]
                                .into_iter()
                                .map(|minutes| {
                                    (
                                        UnloadRule::AfterIdle(minutes),
                                        format!("Unload after {minutes} minutes"),
                                    )
                                })
                                .collect(),
                            UnloadKind::WhenMemoryLow => {
                                vec![(UnloadRule::WhenMemoryLow, "When memory is low".into())]
                            }
                            UnloadKind::WhileServerModeOpen => vec![(
                                UnloadRule::WhileServerModeOpen,
                                "While server mode is open".into(),
                            )],
                        };
                        for (rule, label) in rules {
                            let id = id.clone();
                            sheet = sheet.child(button(
                                devices,
                                format!("devices.unload.{rule:?}"),
                                label,
                                move |state, _, cx| {
                                    let _ = state.unload(&id, rule, cx);
                                },
                            ));
                        }
                    }
                    let id = id.clone();
                    sheet = sheet.child(button(
                        devices,
                        "devices.forget",
                        "Forget device",
                        move |state, _, cx| {
                            let _ = state.forget(&id, cx);
                        },
                    ));
                    page = page.child(sheet.child(button(
                        devices,
                        "devices.close",
                        "Done",
                        |state, _, cx| state.close(cx),
                    )));
                }
            }
            None => {}
        }
        page
    }
}

fn button(
    devices: &Entity<Devices>,
    id: impl Into<gpui_kit::SharedString>,
    label: impl Into<gpui_kit::SharedString>,
    action: impl Fn(&mut Devices, &mut Window, &mut Context<Devices>) + 'static,
) -> Button {
    let devices = devices.clone();
    Button::new(id.into())
        .label(label)
        .on_click(move |_, window, cx| devices.update(cx, |state, cx| action(state, window, cx)))
}

fn qr(modules: Vec<Vec<bool>>) -> impl IntoElement {
    use gpui_kit::{Bounds, Point, Size, canvas, fill, px, rgb};
    let side = modules.len() + 8;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let cell = f32::from(bounds.size.width) / side as f32;
            window.paint_quad(fill(bounds, rgb(0xffffff)));
            for (y, row) in modules.iter().enumerate() {
                for (x, dark) in row.iter().enumerate() {
                    if *dark {
                        let origin = Point::new(
                            bounds.origin.x + px((x + 4) as f32 * cell),
                            bounds.origin.y + px((y + 4) as f32 * cell),
                        );
                        window.paint_quad(fill(
                            Bounds::new(origin, Size::new(px(cell), px(cell))),
                            rgb(0),
                        ));
                    }
                }
            }
        },
    )
    .size(px(side as f32 * 3.0))
}
