//! The Devices page: the switch for models on other devices, the order of the devices that run
//! each kind of model, and the paired devices with their status. The page under the switch fades
//! and takes no input while models on other devices are off.

use super::{Devices, focus};
use crate::theme::{self, Palette, size, space};
use crate::ui::{self, Kind, Notice};
use crate::{hook, motion};
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontFeatures, InteractiveElement,
    IntoElement, ParentElement, Render, Rgba, Role, SharedString, StatefulInteractiveElement,
    Styled, Window, div, px, svg,
};
use hushpen_core::remote::{DeviceKind, ModelKind, Route, Status, THIS_COMPUTER};
use hushpen_store::settings::RemoteDevice;
use std::sync::Arc;

/// How much of the page under the switch shows while models on other devices are off.
const OFF_OPACITY: f32 = 0.35;
/// How much of a route row shows while it is dragged.
const DRAGGED_OPACITY: f32 = 0.5;

/// What a drag of a route row carries.
#[derive(Clone)]
struct RouteDrag {
    kind: ModelKind,
    id: String,
}

/// A drag draws nothing at the pointer: the rows reorder under it as it moves.
struct NoPreview;

impl Render for NoPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub fn render(devices: &Entity<Devices>, cx: &mut App) -> impl IntoElement {
    let palette = theme::palette(cx);
    let state = devices.read(cx);
    let live = state.settings.enabled;
    let mut page = div().flex().flex_col().gap(px(space::XXL)).w_full();
    if let Some(notice) = state.notice.clone().filter(|_| state.sheet.is_none()) {
        page = page.child(ui::group(
            hook::id("devices", "notice.group"),
            palette,
            vec![ui::notice_row(
                palette,
                hook::id("devices", "notice"),
                notice,
                Notice::Failure,
                None,
            )],
        ));
    }
    let general = ui::group(
        hook::id("devices", "general"),
        palette,
        vec![
            master_row(palette, devices, state, cx),
            no_local_row(palette, devices, state, cx),
        ],
    );
    let below = div()
        .flex()
        .flex_col()
        .gap(px(space::XXL))
        .w_full()
        .child(route_section(
            palette,
            devices,
            state,
            ModelKind::Speech,
            cx,
        ))
        .child(route_section(
            palette,
            devices,
            state,
            ModelKind::Cleanup,
            cx,
        ))
        .child(device_section(palette, devices, state, cx));
    page.child(general)
        .child(dimmed(hook::id("devices", "dim"), live, below))
}

/// `element` faded while `live` is off, easing to its new look as the switch turns.
fn dimmed(id: SharedString, live: bool, element: gpui_kit::Div) -> impl IntoElement {
    motion::moving(id, if live { 1.0 } else { 0.0 }, motion::BASE, move |at| {
        if at >= 1.0 {
            element
        } else {
            element.opacity(OFF_OPACITY + (1.0 - OFF_OPACITY) * at)
        }
    })
}

fn master_row(
    palette: Palette,
    devices: &Entity<Devices>,
    state: &Devices,
    cx: &App,
) -> AnyElement {
    let on = state.settings.enabled;
    let devices = devices.clone();
    ui::toggle_row(
        palette,
        hook::id("devices", "enabled"),
        "Use models on other devices",
        "Phones and servers run speech and cleanup for this computer.",
        on,
        &state.focus.get("enabled", cx),
        move |cx| {
            devices.update(cx, |state, cx| {
                let _ = state.set_enabled(!on, cx);
            })
        },
    )
}

/// Turning local models off means something only while other devices are in use.
fn no_local_row(
    palette: Palette,
    devices: &Entity<Devices>,
    state: &Devices,
    cx: &App,
) -> AnyElement {
    const TITLE: &str = "Do not load models on this computer";
    let (live, on) = (state.settings.enabled, state.settings.no_local_models);
    let devices = devices.clone();
    let switch = ui::switch(
        palette,
        hook::id("devices", "noLocal"),
        TITLE,
        on,
        live,
        &state.focus.get("noLocal", cx),
        move |_, cx| {
            devices.update(cx, |state, cx| {
                let _ = state.set_no_local(!on, cx);
            })
        },
    );
    let row = ui::labeled_row(
        palette,
        hook::id("devices", "noLocal.row"),
        TITLE,
        Some("If no device answers, the dictation stops with a notice.".into()),
        switch,
    );
    dimmed(
        hook::id("devices", "noLocal.dim"),
        live,
        div().flex().flex_col().w_full().child(row),
    )
    .into_any_element()
}

/// One kind of model: the devices that run it, in the order Hushpen tries them.
fn route_section(
    palette: Palette,
    devices: &Entity<Devices>,
    state: &Devices,
    kind: ModelKind,
    cx: &App,
) -> AnyElement {
    let (key, title, ids) = match kind {
        ModelKind::Speech => ("speech", "Speech to Text", &state.settings.routes.speech),
        ModelKind::Cleanup => ("cleanup", "Text cleanup", &state.settings.routes.cleanup),
    };
    let route = Route::new(ids.clone(), state.settings.no_local_models);
    let mut rows: Vec<AnyElement> = route
        .ids()
        .iter()
        .filter_map(|id| route_row(palette, devices, state, kind, key, &route, id, cx))
        .collect();
    if rows.is_empty() {
        rows.push(ui::notice_row(
            palette,
            hook::id("devices", &format!("{key}.empty")),
            "No device runs this. Add one, or allow models on this computer.".into(),
            Notice::Info,
            None,
        ));
    }
    let footer = (kind == ModelKind::Cleanup).then(|| {
        (
            hook::id("devices", "routes.help"),
            "Hushpen uses the first device in each list that answers.".to_owned(),
        )
    });
    ui::titled_section(
        palette,
        hook::id("devices", &format!("heading.{key}")),
        title,
        ui::group(hook::id("devices", &format!("route.{key}")), palette, rows),
        footer,
    )
}

/// A device in a route: a grip, its kind, name, model, latency, and status, then Move up and
/// Move down. A drag of the row moves the device as the pointer passes the middle of another
/// row. This computer stays last, so it has no grip and its arrows are off.
#[expect(clippy::too_many_arguments, reason = "the parts of one row")]
fn route_row(
    palette: Palette,
    devices: &Entity<Devices>,
    state: &Devices,
    kind: ModelKind,
    key: &str,
    route: &Route,
    id: &str,
    cx: &App,
) -> Option<AnyElement> {
    let device = if id == THIS_COMPUTER {
        None
    } else {
        Some(state.device(id)?)
    };
    let (name, icon) = match device {
        None => ("This computer".to_owned(), IconName::Laptop),
        Some(device) => (device.device.name.clone(), kind_icon(device.device.kind)),
    };
    let model = state.model(kind, device).unwrap_or_default();
    let row = format!("{key}.{id}");
    let reading = reading(palette, state, id);
    let (live, movable) = (state.settings.enabled, device.is_some());
    let arrows = [(true, "up", "Move up"), (false, "down", "Move down")].map(|(up, way, tip)| {
        let can_move = state.settings.enabled && route.clone().move_device(id, up);
        let control = format!("{row}.{way}");
        let devices = devices.clone();
        let id = id.to_owned();
        ui::icon_button_enabled(
            palette,
            hook::id("devices", &control),
            if up {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            },
            format!("Move {name} {way}"),
            tip,
            &state.focus.get(&control, cx),
            can_move,
            move |_, cx| {
                devices.update(cx, |state, cx| {
                    let _ = state.move_device(kind, &id, up, cx);
                })
            },
        )
    });
    let mut grip = div()
        .id(hook::id("devices", &format!("{row}.grip")))
        .test_support()
        .flex_none()
        .w(px(size::NAV_ICON))
        .mr(px(-space::XS));
    if movable {
        grip = grip.child(
            svg()
                .path(IconName::GripVertical.path())
                .size(px(size::NAV_ICON))
                .text_color(palette.label_tertiary),
        );
        if live {
            grip = grip.cursor_grab();
        }
    }
    let dragged = cx.has_active_drag()
        && state
            .dragging
            .as_ref()
            .is_some_and(|(from, dragged)| *from == kind && dragged == id);
    let mut element = ui::row(palette)
        .id(hook::id("devices", &row))
        .test_support()
        .child(grip)
        .child(ui::icon(palette, icon))
        .child(ui::named_lines(
            palette,
            hook::id("devices", &format!("{row}.title")),
            name.clone(),
            None,
        ))
        .child(model_cell(palette, &row, model))
        .child(latency_cell(palette, &row, reading.latency))
        .child(status_cell(palette, &row, &reading).w(px(size::DEVICE_STATUS_WIDTH)))
        .child(
            div()
                .flex()
                .flex_none()
                .gap(px(size::MODEL_ICON_GAP))
                .children(arrows),
        );
    if dragged {
        element = element.opacity(DRAGGED_OPACITY);
    }
    if live {
        let (target, over) = (devices.clone(), id.to_owned());
        // Every row of every route hears each move of a drag, so a row acts only on a drag of its
        // own route that is over it.
        element = element.on_drag_move::<RouteDrag>(move |event, _, cx| {
            let pointer = event.event.position;
            let drag = event.drag(cx).clone();
            if drag.kind != kind || drag.id == over || !event.bounds.contains(&pointer) {
                return;
            }
            let below = pointer.y > event.bounds.center().y;
            target.update(cx, |state, cx| {
                let _ = state.drag_over(kind, &drag.id, &over, below, cx);
            });
        });
    }
    if live && movable {
        let source = devices.clone();
        let value = RouteDrag {
            kind,
            id: id.to_owned(),
        };
        element = element.on_drag(value, move |drag, _, _, cx| {
            source.update(cx, |state, cx| {
                state.dragging = Some((drag.kind, drag.id.clone()));
                cx.notify();
            });
            cx.new(|_| NoPreview)
        });
    }
    Some(element.into_any_element())
}

/// The model the device of the row `row` runs, at `<row>.model`. A long name shows whole in the
/// tooltip.
fn model_cell(palette: Palette, row: &str, model: String) -> impl IntoElement {
    let cell = ui::value(palette)
        .id(hook::id("devices", &format!("{row}.model")))
        .test_support()
        .aria_label(model.clone())
        .w(px(size::DEVICE_MODEL_WIDTH))
        .text_right()
        .truncate()
        .child(model.clone());
    ui::tooltip(cell, model)
}

/// Every paired device, with Add Device at the end of the heading and Details on each row.
fn device_section(
    palette: Palette,
    devices: &Entity<Devices>,
    state: &Devices,
    cx: &App,
) -> AnyElement {
    let live = state.settings.enabled;
    let add = {
        let devices = devices.clone();
        move |_: &mut Window, cx: &mut App| {
            devices.update(cx, |state, cx| {
                state.focus.opened_by(focus::ADD);
                state.open_add(cx);
            })
        }
    };
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .pr(px(size::ROW_PADDING))
        .child(ui::heading(
            palette,
            hook::id("devices", "heading.devices"),
            "Your devices",
        ))
        .child(ui::button(
            palette,
            hook::id("devices", "add"),
            "Add Device…",
            Kind::Plain,
            Some(&state.focus.get(focus::ADD, cx)),
            live,
            add,
        ));
    let mut rows: Vec<AnyElement> = state
        .settings
        .devices
        .iter()
        .map(|device| device_row(palette, devices, state, device, cx))
        .collect();
    if rows.is_empty() {
        rows.push(ui::notice_row(
            palette,
            hook::id("devices", "empty"),
            "No devices yet. Add a phone or a server.".into(),
            Notice::Info,
            None,
        ));
    }
    div()
        .flex()
        .flex_col()
        .gap(px(space::XS + space::XXS))
        .w_full()
        .child(header)
        .child(ui::group(hook::id("devices", "list"), palette, rows))
        .into_any_element()
}

fn device_row(
    palette: Palette,
    devices: &Entity<Devices>,
    state: &Devices,
    device: &RemoteDevice,
    cx: &App,
) -> AnyElement {
    let id = device.device.id.clone();
    let row = format!("device.{id}");
    let control = format!("{row}.info");
    let reading = reading(palette, state, &id);
    let open = {
        let (devices, control) = (devices.clone(), control.clone());
        move |window: &mut Window, cx: &mut App| {
            devices.update(cx, |state, cx| {
                state.focus.opened_by(&control);
                state.open_detail(&id, window, cx);
            })
        }
    };
    ui::row(palette)
        .id(hook::id("devices", &row))
        .test_support()
        .child(ui::icon(palette, kind_icon(device.device.kind)))
        .child(ui::named_lines(
            palette,
            hook::id("devices", &format!("{row}.title")),
            device.device.name.clone(),
            None,
        ))
        .child(latency_cell(palette, &row, reading.latency))
        .child(status_cell(palette, &row, &reading).w(px(size::DEVICE_STATUS_WIDTH)))
        .child(ui::icon_button_enabled(
            palette,
            hook::id("devices", &control),
            IconName::Info,
            format!("{} details", device.device.name),
            "Details",
            &state.focus.get(&control, cx),
            state.settings.enabled,
            open,
        ))
        .into_any_element()
}

pub(super) fn kind_icon(kind: DeviceKind) -> IconName {
    match kind {
        DeviceKind::Phone => IconName::Smartphone,
        DeviceKind::Server => IconName::Server,
        DeviceKind::ThisComputer => IconName::Laptop,
    }
}

/// What a row says about a device: its status word, the color of the word, and the time of the
/// last answer.
pub(super) struct Reading {
    pub word: &'static str,
    pub tone: Rgba,
    pub latency: Option<u32>,
}

pub(super) fn reading(palette: Palette, state: &Devices, id: &str) -> Reading {
    if id == THIS_COMPUTER {
        return Reading {
            word: "Fallback",
            tone: palette.label_secondary,
            latency: None,
        };
    }
    let mut health = state.health.get(id).cloned().unwrap_or_default();
    health.tick(crate::controller::monotonic_ms());
    let (word, tone) = match health.status {
        Status::Ready => ("Ready", palette.success),
        Status::Asleep => ("Asleep", palette.label_secondary),
        Status::ServerModeOff => ("Server mode off", palette.label_secondary),
        Status::Unreachable => ("Not reachable", palette.error),
        Status::Unknown if state.settings.enabled => ("Checking", palette.label_secondary),
        Status::Unknown => ("Not checked", palette.label_tertiary),
    };
    Reading {
        word,
        tone,
        latency: health.latency_ms,
    }
}

/// The status word of the row `row`, at `<row>.status`.
pub(super) fn status_cell(
    palette: Palette,
    row: &str,
    reading: &Reading,
) -> impl IntoElement + Styled + use<> {
    ui::value(palette)
        .id(hook::id("devices", &format!("{row}.status")))
        .test_support()
        .role(Role::Status)
        .aria_label(reading.word)
        .text_right()
        .text_color(reading.tone)
        .child(reading.word)
}

/// The latency of the row `row` in tabular digits, at `<row>.latency`, so the column holds still.
pub(super) fn latency_cell(palette: Palette, row: &str, latency: Option<u32>) -> impl IntoElement {
    let text = latency.map(|ms| format!("{ms} ms")).unwrap_or_default();
    ui::value(palette)
        .id(hook::id("devices", &format!("{row}.latency")))
        .test_support()
        .aria_label(text.clone())
        .w(px(size::DEVICE_LATENCY_WIDTH))
        .text_right()
        .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
        .child(text)
}
