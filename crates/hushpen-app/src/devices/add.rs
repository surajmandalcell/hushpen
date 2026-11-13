//! Add a Device: Nearby | Remote. Nearby lists the phones found on this network; picking one
//! shows the pairing number both screens must match. Under the list, a QR and an eight-digit
//! code pair a phone that is not listed. Remote lists the SSH and Tailscale machines and sets
//! one up over SSH, showing each step.

use super::sheet::{self, Page};
use super::{AddMethod, Devices, Request, focus};
use crate::device_ssh::Step;
use crate::hook;
use crate::theme::{Palette, radius, size, space};
use crate::ui::{self, Kind, Notice};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, Bounds, Entity, FontFeatures, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Point, Role, Size, StatefulInteractiveElement, Styled, canvas, div, fill, px,
    rgb, svg,
};
use hushpen_core::remote::{PairFailure, Pairing, RemoteHost};
use hushpen_store::settings::Transport;
use std::sync::Arc;

const METHODS: [(AddMethod, &str, &str); 2] = [
    (AddMethod::Nearby, "nearby", "Nearby"),
    (AddMethod::Remote, "remote", "Remote"),
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

/// The modules of white a scanner needs around a QR code.
const QUIET_ZONE: usize = 4;

const KEY_HELP: &str = "Pairing keeps a key on both sides, so every frame is encrypted.";

pub(super) fn render(
    palette: Palette,
    devices: &Entity<Devices>,
    method: AddMethod,
    cx: &App,
) -> Page {
    let state = devices.read(cx);
    let mut body = vec![methods(palette, devices, method, cx)];
    let comparing =
        method == AddMethod::Nearby && matches!(state.pairing, Pairing::Confirming { .. });
    let failed = matches!(state.pairing, Pairing::Failed(_));
    // A failed offer shows its reason beside the button that starts a new one.
    if !(method == AddMethod::Nearby && failed) {
        body.extend(sheet::notice(palette, state));
    }
    match method {
        AddMethod::Nearby if comparing => body.extend(compare(palette, devices, cx)),
        AddMethod::Nearby => body.extend(nearby(palette, devices, cx)),
        AddMethod::Remote => body.push(remote(palette, devices, cx)),
    }
    let right = if comparing {
        vec![cancel(palette, devices, cx), pair(palette, devices, cx)]
    } else {
        vec![sheet::done(palette, devices, cx)]
    };
    Page {
        title: "Add a Device".into(),
        subtitle: "Pick where to look. Hushpen sets up what it finds.".into(),
        label: "Add a Device".into(),
        body,
        left: None,
        right,
    }
}

/// Nearby and Remote, centered over the groups. Picking the open tab looks again.
fn methods(palette: Palette, devices: &Entity<Devices>, method: AddMethod, cx: &App) -> AnyElement {
    let chosen = METHODS
        .iter()
        .find(|(each, ..)| *each == method)
        .map_or("nearby", |(_, key, _)| *key);
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
                let Some(&(next, ..)) = METHODS.iter().find(|(_, each, _)| *each == key) else {
                    return;
                };
                devices.update(cx, |state, cx| {
                    if next == method {
                        let again = match next {
                            AddMethod::Nearby => Request::Discover,
                            AddMethod::Remote => Request::Hosts,
                        };
                        state.request(again, cx);
                    } else {
                        state.select_method(next, cx);
                    }
                });
            },
        ))
        .into_any_element()
}

/// The phones found on this network, then the QR and the code for a phone that is not listed.
fn nearby(palette: Palette, devices: &Entity<Devices>, cx: &App) -> Vec<AnyElement> {
    let state = devices.read(cx);
    let mut found = Vec::new();
    for phone in &state.discovered {
        let key = format!("add.found.{}", phone.device.id);
        let selected = state
            .selected_device
            .as_ref()
            .is_some_and(|device| device.id == phone.device.id);
        let hostname = phone.hostname.trim_end_matches('.');
        let (trailing, detail) = if selected {
            (
                ui::value(palette).child("Waiting…").into_any_element(),
                "Pair on the phone to show the number".to_owned(),
            )
        } else {
            let control = format!("{key}.pair");
            let (devices, device) = (devices.clone(), phone.device.clone());
            (
                ui::button(
                    palette,
                    hook::id("devices", &control),
                    "Pair…",
                    Kind::Plain,
                    Some(&state.focus.get(&control, cx)),
                    !state.busy,
                    move |_, cx| {
                        devices.update(cx, |state, cx| {
                            state.selected_device = Some(device.clone());
                            cx.notify();
                        })
                    },
                )
                .into_any_element(),
                format!("Hushpen found · {hostname}"),
            )
        };
        found.push(device_line(
            palette,
            &key,
            IconName::Smartphone,
            phone.device.name.clone(),
            detail,
            trailing,
        ));
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
            "No phones found.".into(),
            Notice::Info,
            Some(again.into_any_element()),
        ));
    }
    vec![
        sheet::section(
            palette,
            "add.found",
            found,
            Some("Open Server mode on a phone. It shows here.".into()),
        ),
        fallback(palette, devices, cx),
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

/// The QR and the code, always under the list. A failed or finished offer says why and offers
/// a new one.
fn fallback(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let rows = match &state.pairing {
        Pairing::Offered { .. } if !state.qr.is_empty() => {
            vec![qr_row(palette, devices, cx), code_row(palette, devices, cx)]
        }
        Pairing::Failed(failure) => {
            let text = state.notice.clone().unwrap_or_else(|| match failure {
                PairFailure::Expired => "The code expired. Show a new one.".into(),
                _ => "The code stopped working. Show a new one.".into(),
            });
            let devices = devices.clone();
            let again = ui::button(
                palette,
                hook::id("devices", "offer"),
                "Show New Code",
                Kind::Plain,
                Some(&state.focus.get("offer", cx)),
                !state.busy,
                move |window, cx| {
                    devices.update(cx, |state, cx| {
                        state.request(
                            Request::Offer {
                                host: String::new(),
                            },
                            cx,
                        );
                        window.focus(&state.focus.get(focus::DONE, cx), cx);
                    })
                },
            );
            vec![ui::notice_row(
                palette,
                hook::id("devices", "add.code.failed"),
                text,
                Notice::Failure,
                Some(again.into_any_element()),
            )]
        }
        _ => vec![ui::notice_row(
            palette,
            hook::id("devices", "add.code.wait"),
            "Getting the code ready…".into(),
            Notice::Info,
            None,
        )],
    };
    sheet::section(palette, "add.code", rows, Some(KEY_HELP.into()))
}

/// The QR beside where to scan it, and how long and how often it works.
fn qr_row(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let minutes = state.offer_minutes().unwrap_or(1).max(1);
    let expiry = format!(
        "It works once, for {minutes} {}.",
        if minutes == 1 { "minute" } else { "minutes" }
    );
    let tries = format!(
        "{} {} left.",
        state.tries_left,
        if state.tries_left == 1 {
            "try"
        } else {
            "tries"
        }
    );
    ui::row(palette)
        .id(hook::id("devices", "add.code.row"))
        .test_support()
        .gap(px(space::LG))
        .py(px(space::MD))
        .child(qr(state.qr.clone()))
        .child(
            ui::text_column()
                .gap(px(space::XS))
                .child(ui::title(palette).child("Phone not listed? Scan this."))
                .child(ui::secondary(palette).child("On the phone: Devices, Pair a Desktop."))
                .child(
                    ui::secondary(palette)
                        .id(hook::id("devices", "add.code.expiry"))
                        .test_support()
                        .aria_label(expiry.clone())
                        .child(expiry),
                )
                .child(
                    ui::secondary(palette)
                        .id(hook::id("devices", "add.code.tries"))
                        .test_support()
                        .aria_label(tries.clone())
                        .child(tries),
                ),
        )
        .into_any_element()
}

/// The eight digits to type on the phone, in two groups of four.
fn code_row(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let code = devices.read(cx).code.clone();
    let shown = match code.len() {
        8 => format!("{} {}", &code[..4], &code[4..]),
        _ => code,
    };
    ui::labeled_row(
        palette,
        hook::id("devices", "add.offer"),
        "Or type the code",
        Some("On the phone: Pair a Desktop, Type Code".into()),
        ui::value(palette)
            .id(hook::id("devices", "code"))
            .test_support()
            .aria_label(shown.clone())
            .text_color(palette.label)
            .font_weight(FontWeight::SEMIBOLD)
            .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
            .child(shown),
    )
}

/// The number both screens show, under the phone that is pairing.
fn compare(palette: Palette, devices: &Entity<Devices>, cx: &App) -> Vec<AnyElement> {
    let state = devices.read(cx);
    let Pairing::Confirming {
        number,
        local_confirmed,
        ..
    } = &state.pairing
    else {
        return Vec::new();
    };
    let shown = if number.len() == 6 {
        format!("{} {}", &number[..3], &number[3..])
    } else {
        number.clone()
    };
    let name = state
        .selected_device
        .as_ref()
        .map_or_else(|| "Phone".to_owned(), |device| device.name.clone());
    let detail = if *local_confirmed {
        "Waiting for you to tap Pair on the phone"
    } else {
        "Compare the numbers, then tap Pair on both"
    }
    .to_owned();
    let line = device_line(
        palette,
        "add.pairing",
        IconName::Smartphone,
        name,
        detail,
        ui::value(palette).child("Pairing…").into_any_element(),
    );
    let big = ui::row(palette)
        .id(hook::id("devices", "add.number.row"))
        .test_support()
        .flex_col()
        .gap(px(space::SM))
        .py(px(space::LG))
        .child(
            div()
                .id(hook::id("devices", "number"))
                .test_support()
                .aria_label(shown.clone())
                .text_size(px(size::PAIRING_NUMBER))
                .font_weight(FontWeight::SEMIBOLD)
                .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
                .text_color(palette.label)
                .child(shown),
        )
        .child(ui::secondary(palette).child("Must match the number on the phone."))
        .into_any_element();
    vec![
        sheet::section(palette, "add.pairing.group", vec![line], None),
        sheet::section(palette, "add.number", vec![big], Some(KEY_HELP.into())),
    ]
}

fn cancel(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let devices = devices.clone();
    ui::button(
        palette,
        hook::id("devices", "cancelPairing"),
        "Cancel",
        Kind::Plain,
        Some(&state.focus.get("cancelPairing", cx)),
        true,
        move |_, cx| devices.update(cx, |state, cx| state.confirm(false, cx)),
    )
    .into_any_element()
}

/// Pair on this computer. It turns off after the tap, while the phone's answer is awaited.
fn pair(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    let waiting = matches!(
        state.pairing,
        Pairing::Confirming {
            local_confirmed: true,
            ..
        }
    );
    let devices = devices.clone();
    ui::button(
        palette,
        hook::id("devices", "pair"),
        "Pair",
        Kind::Default,
        Some(&state.focus.get(focus::DONE, cx)),
        !waiting,
        move |_, cx| devices.update(cx, |state, cx| state.confirm(true, cx)),
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

/// The SSH target of a machine: its alias, else its address.
fn target(host: &RemoteHost) -> String {
    host.ssh_alias
        .clone()
        .unwrap_or_else(|| host.address.clone())
}

/// Every machine Hushpen can reach, and a field for any other, or the steps of the setup once
/// one runs.
fn remote(palette: Palette, devices: &Entity<Devices>, cx: &App) -> AnyElement {
    let state = devices.read(cx);
    if state.busy || !state.progress.is_empty() {
        return steps(palette, state);
    }
    let setup = |control: String, host: String| {
        let devices = devices.clone();
        ui::button(
            palette,
            hook::id("devices", &control),
            "Set Up…",
            Kind::Plain,
            Some(&state.focus.get(&control, cx)),
            !state.busy && !host.is_empty(),
            move |window, cx| {
                devices.update(cx, |state, cx| {
                    state.request(Request::SetupSsh { host: host.clone() }, cx);
                    window.focus(&state.focus.get(focus::DONE, cx), cx);
                })
            },
        )
        .into_any_element()
    };
    let saved = |host: &str| {
        state.settings.devices.iter().any(
            |device| matches!(&device.transport, Transport::Ssh { host: saved } if saved == host),
        )
    };
    let mut rows: Vec<AnyElement> = state
        .hosts
        .iter()
        .map(|host| {
            let target = target(host);
            let trailing = if saved(&target) {
                ui::value(palette).child("Added").into_any_element()
            } else {
                setup(format!("add.ssh.{}", host.name), target)
            };
            device_line(
                palette,
                &format!("add.host.{}", host.name),
                IconName::Server,
                host.name.clone(),
                host.secondary_line(),
                trailing,
            )
        })
        .collect();
    let typed = RemoteHost::typed(state.host.read(cx).value().as_ref()).map(|host| host.address);
    // The label and its line need room, so the field is narrower than a field row's.
    rows.push(ui::labeled_row(
        palette,
        hook::id("devices", "host.row"),
        "Another machine",
        Some("Host name or address".into()),
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(space::SM))
            .child(div().w(px(size::HOST_FIELD_WIDTH)).child(ui::field(
                palette,
                hook::id("devices", "host"),
                "Another machine",
                &state.host,
            )))
            .child(setup("add.ssh.typed".into(), typed.unwrap_or_default())),
    ));
    sheet::section(
        palette,
        "add.hosts",
        rows,
        Some("From ~/.ssh/config and tailscale status. A machine in both shows once.".into()),
    )
}

/// The steps of a server setup, with the failed one marked.
fn steps(palette: Palette, state: &Devices) -> AnyElement {
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
    sheet::section(
        palette,
        "add.steps",
        rows,
        Some("The host listens on port 47321. Audio uses the pairing key, not SSH.".into()),
    )
}
