//! Add a Device: the three places Hushpen looks for one. Nearby lists the phones that announce
//! themselves on this network and shows a pairing code; SSH sets up a server from
//! `~/.ssh/config` and shows each step; Tailscale pairs with an address.

use super::sheet::{self, Page};
use super::{AddMethod, Devices, Picker, Request, focus};
use crate::device_ssh::Step;
use crate::hook;
use crate::theme::{Palette, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Bounds, Entity, InteractiveElement, IntoElement, ParentElement, Point, Role,
    Size, StatefulInteractiveElement, Styled, canvas, div, fill, px, rgb, svg,
};
use hushpen_core::os::Os;
use hushpen_core::remote::Pairing;
use hushpen_store::settings::Transport;

const METHODS: [(AddMethod, &str, &str); 3] = [
    (AddMethod::Phone, "phone", "Nearby"),
    (AddMethod::Ssh, "ssh", "SSH"),
    (AddMethod::Tailscale, "tailscale", "Tailscale"),
];

/// The steps of a server setup, in the order the service reports them.
const STEPS: [Step; 6] = [
    Step::Connecting,
    Step::Downloading,
    Step::Verifying,
    Step::Installing,
    Step::Starting,
    Step::Pairing,
];

/// The systems a device on Tailscale can run, as the System pop-up names them.
const SYSTEMS: [(Os, &str, &str); 5] = [
    (Os::Linux, "linux", "Linux"),
    (Os::Android, "android", "Android"),
    (Os::Ios, "ios", "iOS"),
    (Os::Mac, "macos", "macOS"),
    (Os::Windows, "windows", "Windows"),
];

/// The modules of white a scanner needs around a QR code.
const QUIET_ZONE: usize = 4;

pub(super) fn render(
    palette: Palette,
    devices: &Entity<Devices>,
    method: AddMethod,
    cx: &App,
) -> Page {
    let state = devices.read(cx);
    let mut body = vec![methods(palette, devices, method, cx)];
    body.extend(sheet::notice(palette, state));
    match method {
        AddMethod::Phone => body.extend(nearby(palette, devices, cx)),
        AddMethod::Ssh => body.push(ssh(palette, devices, cx)),
        AddMethod::Tailscale => body.push(tailscale(palette, devices, cx)),
    }
    Page {
        title: "Add a Device".into(),
        subtitle: "Pick where to look. Hushpen sets up what it finds.".into(),
        label: "Add a Device".into(),
        body,
        left: None,
        right: vec![sheet::done(palette, devices, cx)],
    }
}

/// Nearby, SSH, and Tailscale, centered over the groups. Nearby looks for phones again.
fn methods(palette: Palette, devices: &Entity<Devices>, method: AddMethod, cx: &App) -> AnyElement {
    let chosen = METHODS
        .iter()
        .find(|(each, ..)| *each == method)
        .map_or("phone", |(_, key, _)| *key);
    let choices = METHODS.map(|(_, key, label)| (key, label));
    let devices = devices.clone();
    let focus = devices.read(cx).focus.get("method", cx);
    div()
        .flex()
        .justify_center()
        .child(ui::segmented(
            palette,
            hook::id("devices", "method"),
            "Where to look",
            &choices,
            size::SEGMENT_WIDTH,
            chosen,
            &focus,
            move |key, _, cx| {
                let Some(&(method, ..)) = METHODS.iter().find(|(_, each, _)| *each == key) else {
                    return;
                };
                devices.update(cx, |state, cx| {
                    state.select_method(method, cx);
                    if method == AddMethod::Phone {
                        state.request(Request::Discover, cx);
                    }
                });
            },
        ))
        .into_any_element()
}

/// The phones found on this network, then the pairing code a phone scans.
fn nearby(palette: Palette, devices: &Entity<Devices>, cx: &App) -> Vec<AnyElement> {
    let state = devices.read(cx);
    let mut found = Vec::new();
    for phone in &state.discovered {
        let key = format!("add.found.{}", phone.device.id);
        let selected = state
            .selected_device
            .as_ref()
            .is_some_and(|device| device.id == phone.device.id);
        let trailing = if selected {
            ui::value(palette).child("Selected").into_any_element()
        } else {
            let control = format!("{key}.pair");
            let (devices, device) = (devices.clone(), phone.device.clone());
            ui::button(
                palette,
                hook::id("devices", &control),
                "Pair…",
                Kind::Plain,
                Some(&state.focus.get(&control, cx)),
                !state.busy,
                move |window, cx| {
                    devices.update(cx, |state, cx| {
                        state.selected_device = Some(device.clone());
                        state.uri.update(cx, |input, cx| input.focus(window, cx));
                        cx.notify();
                    })
                },
            )
            .into_any_element()
        };
        found.push(device_line(
            palette,
            &key,
            IconName::Smartphone,
            phone.device.name.clone(),
            format!("Hushpen found · {}", phone.hostname.trim_end_matches('.')),
            trailing,
        ));
        if selected {
            found.push(link_row(palette, devices, "pair", cx));
        }
    }
    if found.is_empty() {
        let devices = devices.clone();
        let again = ui::button(
            palette,
            hook::id("devices", "discover"),
            "Search Again",
            Kind::Plain,
            Some(&state.focus.get("discover", cx)),
            !state.busy,
            move |_, cx| devices.update(cx, |state, cx| state.request(Request::Discover, cx)),
        );
        found.push(ui::notice_row(
            palette,
            hook::id("devices", "add.found.empty"),
            "No phones found. Open Server mode on the phone.".into(),
            Notice::Info,
            Some(again.into_any_element()),
        ));
    }
    let code = if matches!(state.pairing, Pairing::Offered { .. }) && !state.qr.is_empty() {
        vec![code_row(palette, devices, cx)]
    } else {
        vec![
            offer_row(palette, devices, cx),
            ui::field_row(
                palette,
                hook::id("devices", "host"),
                "Address of this computer",
                "Leave it on Automatic",
                &state.host,
                None,
            ),
        ]
    };
    vec![
        sheet::section(
            palette,
            "add.found",
            found,
            Some("Phones with Server mode open show here.".into()),
        ),
        sheet::section(
            palette,
            "add.code",
            code,
            Some("Pairing keeps a key on both sides, so every frame is encrypted.".into()),
        ),
    ]
}

/// A found phone or a server: its icon, its name over where it was found, and an action.
fn device_line(
    palette: Palette,
    key: &str,
    icon: IconName,
    name: String,
    detail: String,
    trailing: AnyElement,
) -> AnyElement {
    ui::row(palette)
        .id(hook::id("devices", key))
        .test_support()
        .child(ui::icon(palette, icon))
        .child(ui::named_lines(
            palette,
            hook::id("devices", &format!("{key}.title")),
            name,
            Some((hook::id("devices", &format!("{key}.detail")), detail)),
        ))
        .child(trailing)
        .into_any_element()
}

/// The pairing link the device shows in Server mode, and Pair, which takes the button `control`.
fn link_row(palette: Palette, devices: &Entity<Devices>, control: &str, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let pair = {
        let devices = devices.clone();
        ui::button(
            palette,
            hook::id("devices", control),
            "Pair",
            Kind::Default,
            Some(&state.focus.get(control, cx)),
            !state.busy,
            move |_, cx| devices.update(cx, |state, cx| state.pair_input(cx)),
        )
    };
    ui::field_row(
        palette,
        hook::id("devices", "uri"),
        "Pairing link",
        "From Server mode",
        &state.uri,
        Some(pair.into_any_element()),
    )
}

fn offer_row(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let show = {
        let devices = devices.clone();
        move |window: &mut gpui_kit::Window, cx: &mut App| {
            devices.update(cx, |state, cx| {
                let host = state.host.read(cx).value().to_string();
                state.request(Request::Offer { host }, cx);
                window.focus(&state.focus.get(focus::DONE, cx), cx);
            })
        }
    };
    ui::labeled_row(
        palette,
        hook::id("devices", "add.offer"),
        "Pairing code",
        Some("A phone scans it to pair".into()),
        ui::button(
            palette,
            hook::id("devices", "offer"),
            "Show Code",
            Kind::Plain,
            Some(&state.focus.get("offer", cx)),
            !state.busy,
            show,
        ),
    )
}

/// The code next to the steps that pair a phone with it, and how long it works.
fn code_row(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let minutes = state.offer_minutes().unwrap_or(1).max(1);
    let expiry = if minutes == 1 {
        "It works once, for 1 minute.".to_owned()
    } else {
        format!("It works once, for {minutes} minutes.")
    };
    let cancel = {
        let devices = devices.clone();
        move |window: &mut gpui_kit::Window, cx: &mut App| {
            devices.update(cx, |state, cx| {
                state.request(Request::CancelOffer, cx);
                window.focus(&state.focus.get("offer", cx), cx);
            })
        }
    };
    let steps = [
        "1. Open Hushpen on the phone.",
        "2. Open Server mode.",
        "3. Scan this code.",
    ];
    ui::row(palette)
        .id(hook::id("devices", "add.code.row"))
        .test_support()
        .gap(px(space::LG))
        .py(px(space::MD))
        .child(qr(state.qr.clone()))
        .child(
            ui::text_column()
                .gap(px(space::XS))
                .children(steps.map(|step| ui::title(palette).child(step)))
                .child(ui::secondary(palette).child(expiry))
                .child(div().flex().pt(px(space::XS)).child(ui::button(
                    palette,
                    hook::id("devices", "cancelOffer"),
                    "Cancel",
                    Kind::Plain,
                    Some(&state.focus.get("cancelOffer", cx)),
                    true,
                    cancel,
                ))),
        )
        .into_any_element()
}

/// The QR code of a pairing offer, in whole pixels per module, about `PAIRING_CODE` on a side.
fn qr(modules: Vec<Vec<bool>>) -> impl IntoElement {
    let side = modules.len() + 2 * QUIET_ZONE;
    let cell = (size::PAIRING_CODE / side as f32).floor().max(2.0);
    // Scanners read dark modules on white only, so the code keeps these colors in both
    // appearances.
    div()
        .id(hook::id("devices", "qr"))
        .test_support()
        .role(Role::Image)
        .aria_label("Pairing code")
        .flex_none()
        .size(px(cell * side as f32))
        .rounded(px(radius::BUTTON))
        .bg(rgb(0xffffff))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    for (y, row) in modules.iter().enumerate() {
                        for (x, dark) in row.iter().enumerate() {
                            if *dark {
                                let origin = Point::new(
                                    bounds.origin.x + px((x + QUIET_ZONE) as f32 * cell),
                                    bounds.origin.y + px((y + QUIET_ZONE) as f32 * cell),
                                );
                                window.paint_quad(fill(
                                    Bounds::new(origin, Size::new(px(cell), px(cell))),
                                    rgb(0x000000),
                                ));
                            }
                        }
                    }
                },
            )
            .size_full(),
        )
}

/// The servers of `~/.ssh/config` with Set Up, or the steps of the setup once one runs.
fn ssh(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    if state.busy || !state.progress.is_empty() {
        let current = state.progress.len().saturating_sub(1);
        let failed = !state.busy && state.notice.is_some();
        let rows = STEPS
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let waiting = index > current || (index == current && !state.busy && !failed);
                let (icon, tone) = match index.cmp(&current) {
                    std::cmp::Ordering::Less => (IconName::Check, palette.label_secondary),
                    std::cmp::Ordering::Equal if state.busy => {
                        (IconName::LoaderCircle, palette.label_secondary)
                    }
                    std::cmp::Ordering::Equal if failed => {
                        (IconName::CircleAlert, palette.label_secondary)
                    }
                    _ => (IconName::Circle, palette.label_tertiary),
                };
                let key = format!("add.step.{index}");
                let label = step.label();
                ui::row(palette)
                    .id(hook::id("devices", &key))
                    .test_support()
                    .child(
                        svg()
                            .path(icon.path())
                            .size(px(size::NAV_ICON))
                            .flex_none()
                            .text_color(tone),
                    )
                    .child(
                        ui::text_column().child(
                            ui::title(palette)
                                .id(hook::id("devices", &format!("{key}.title")))
                                .test_support()
                                .aria_label(label)
                                .text_color(if waiting {
                                    palette.label_tertiary
                                } else {
                                    palette.label
                                })
                                .child(label),
                        ),
                    )
                    .children((index == current && failed).then(|| {
                        ui::value(palette)
                            .id(hook::id("devices", &format!("{key}.status")))
                            .test_support()
                            .role(Role::Status)
                            .aria_label("Failed")
                            .text_color(palette.error)
                            .child("Failed")
                    }))
                    .into_any_element()
            })
            .collect();
        return sheet::section(
            palette,
            "add.steps",
            rows,
            Some("Audio goes through the SSH connection. No port opens on the server.".into()),
        );
    }
    let mut rows: Vec<AnyElement> = state
        .hosts
        .iter()
        .map(|host| {
            let added = state.settings.devices.iter().any(
                |device| matches!(&device.transport, Transport::Ssh { host: saved } if saved == host),
            );
            let trailing = if added {
                ui::value(palette).child("Added").into_any_element()
            } else {
                let control = format!("add.ssh.{host}");
                let (devices, host) = (devices.clone(), host.clone());
                ui::button(
                    palette,
                    hook::id("devices", &control),
                    "Set Up…",
                    Kind::Plain,
                    Some(&state.focus.get(&control, cx)),
                    !state.busy,
                    move |window, cx| {
                        devices.update(cx, |state, cx| {
                            state.selected_host = Some(host.clone());
                            state.request(Request::SetupSsh { host: host.clone() }, cx);
                            window.focus(&state.focus.get(focus::DONE, cx), cx);
                        })
                    },
                )
                .into_any_element()
            };
            device_line(
                palette,
                &format!("add.host.{host}"),
                IconName::Server,
                host.clone(),
                "From ~/.ssh/config".into(),
                trailing,
            )
        })
        .collect();
    if rows.is_empty() {
        rows.push(ui::notice_row(
            palette,
            hook::id("devices", "add.hosts.empty"),
            "No servers in ~/.ssh/config.".into(),
            Notice::Info,
            None,
        ));
    }
    sheet::section(
        palette,
        "add.hosts",
        rows,
        Some("Hushpen installs a small host in your home folder there. No sudo needed.".into()),
    )
}

/// The system, the address, and the pairing link of a device on Tailscale.
fn tailscale(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let current = SYSTEMS
        .iter()
        .find(|(os, ..)| *os == state.tailscale_os)
        .map_or("Linux", |(_, _, label)| *label);
    let menu = (state.menu == Some(Picker::System)).then(|| {
        SYSTEMS.iter().fold(
            ui::Menu::new(hook::id("devices", "os.list"), sheet::close_menu(devices)),
            |menu, &(os, key, label)| {
                let option = format!("os.{key}");
                let devices = devices.clone();
                menu.option(
                    hook::id("devices", &option),
                    label,
                    None,
                    os == state.tailscale_os,
                    &state.focus.get(&option, cx),
                    move |_, cx| {
                        devices.update(cx, |state, cx| {
                            state.tailscale_os = os;
                            cx.notify();
                        })
                    },
                )
            },
        )
    });
    let system = ui::menu_row(
        palette,
        "What the device runs",
        ui::PopUp {
            id: hook::id("devices", "os"),
            label: "System",
            current: current.to_owned(),
            enabled: !state.busy,
            focus: &state.focus.get("os", cx),
            menu,
        },
        sheet::toggle(devices, Picker::System),
    );
    let rows = vec![
        system,
        ui::field_row(
            palette,
            hook::id("devices", "address"),
            "Address",
            "Tailscale name or IP, and port",
            &state.address,
            None,
        ),
        link_row(palette, devices, "pairTailscale", cx),
    ];
    sheet::section(
        palette,
        "add.tailscale",
        rows,
        Some("Both devices must be in the same Tailscale network.".into()),
    )
}
