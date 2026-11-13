//! Devices state and intents. Network work runs in the device service.

mod add;
mod detail;
mod focus;
pub mod panel;
pub mod sheet;

use crate::storage::Storage;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext as _, Context, Entity, EventEmitter, Window};
use hushpen_core::os::Os;
use hushpen_core::remote::{Device, Health, ModelKind, PairEvent, Pairing, Route, UnloadRule};
use hushpen_remote::discovery::FoundDevice;
use hushpen_store::settings::{RemoteDevice, RemoteSettings, Transport};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub enum Request {
    Discover,
    Offer {
        host: String,
    },
    CancelOffer,
    Pair {
        bootstrap: String,
        device: Device,
        transport: Transport,
    },
    SetupSsh {
        host: String,
    },
    Probe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMethod {
    Phone,
    Ssh,
    Tailscale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sheet {
    Add(AddMethod),
    Detail(String),
}

/// A pop-up button of the sheets whose menu is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picker {
    System,
    Rule,
    IdleTime,
}

pub struct Devices {
    storage: Rc<Storage>,
    pub settings: RemoteSettings,
    pub health: BTreeMap<String, Health>,
    pub discovered: Vec<FoundDevice>,
    pub hosts: Vec<String>,
    pub selected_host: Option<String>,
    pub selected_device: Option<Device>,
    pub tailscale_os: Os,
    pub sheet: Option<Sheet>,
    pub pairing: Pairing,
    pub qr: Vec<Vec<bool>>,
    pub progress: Vec<String>,
    pub busy: bool,
    pub notice: Option<String>,
    pub menu: Option<Picker>,
    /// The details ask once before they forget the device.
    pub forgetting: bool,
    /// The route row that the last drag picked up. It counts only while a drag runs.
    pub dragging: Option<(ModelKind, String)>,
    pub focus: focus::Focus,
    pub host: Entity<InputState>,
    pub address: Entity<InputState>,
    pub name: Entity<InputState>,
}

impl EventEmitter<Request> for Devices {}

impl Devices {
    pub fn new(storage: Rc<Storage>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            settings: storage.settings.remote(),
            storage,
            health: BTreeMap::new(),
            discovered: Vec::new(),
            hosts: Vec::new(),
            selected_host: None,
            selected_device: None,
            tailscale_os: Os::Linux,
            sheet: None,
            pairing: Pairing::Idle,
            qr: Vec::new(),
            progress: Vec::new(),
            busy: false,
            notice: None,
            menu: None,
            forgetting: false,
            dragging: None,
            focus: focus::Focus::default(),
            host: cx.new(|cx| InputState::new(window, cx).placeholder("Automatic")),
            address: cx
                .new(|cx| InputState::new(window, cx).placeholder("Tailscale address and port")),
            name: cx.new(|cx| InputState::new(window, cx).placeholder("Device name")),
        }
    }

    fn save(&mut self, next: RemoteSettings, cx: &mut Context<Self>) -> Result<(), String> {
        self.storage.settings.set_remote(&next).map_err(|error| {
            let message = format!("Could not save devices: {error}");
            self.notice = Some(message.clone());
            cx.notify();
            message
        })?;
        self.settings = self.storage.settings.remote();
        self.notice = None;
        cx.notify();
        Ok(())
    }

    pub fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) -> Result<(), String> {
        let mut next = self.settings.clone();
        next.enabled = enabled;
        self.save(next, cx)
    }

    pub fn set_no_local(&mut self, disabled: bool, cx: &mut Context<Self>) -> Result<(), String> {
        let mut next = self.settings.clone();
        next.no_local_models = disabled;
        self.save(next, cx)
    }

    pub fn move_device(
        &mut self,
        kind: ModelKind,
        id: &str,
        up: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.reorder(kind, cx, |route| route.move_device(id, up))
    }

    /// Puts `id` at `index` of the route of `kind`, as a drag does.
    pub fn move_device_to(
        &mut self,
        kind: ModelKind,
        id: &str,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.reorder(kind, cx, |route| route.move_to(id, index))
    }

    /// Moves the dragged device `id` next to `over` in the route of `kind`: before it while the
    /// pointer is in the top half of the row of `over`, and after it below that.
    pub fn drag_over(
        &mut self,
        kind: ModelKind,
        id: &str,
        over: &str,
        below: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let ids = match kind {
            ModelKind::Speech => &self.settings.routes.speech,
            ModelKind::Cleanup => &self.settings.routes.cleanup,
        };
        let route = Route::new(ids.clone(), self.settings.no_local_models);
        let place = |id: &str| route.ids().iter().position(|item| item == id);
        let (Some(from), Some(at)) = (place(id), place(over)) else {
            return Ok(());
        };
        // Taking the device out first shifts the rows after it up by one.
        let index = match (below, from < at) {
            (true, true) | (false, false) => at,
            (true, false) => at + 1,
            (false, true) => at - 1,
        };
        self.move_device_to(kind, id, index, cx)
    }

    fn reorder(
        &mut self,
        kind: ModelKind,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Route) -> bool,
    ) -> Result<(), String> {
        let mut next = self.settings.clone();
        let ids = match kind {
            ModelKind::Speech => &mut next.routes.speech,
            ModelKind::Cleanup => &mut next.routes.cleanup,
        };
        let mut route = Route::new(ids.clone(), next.no_local_models);
        if change(&mut route) {
            *ids = route.ids().to_vec();
            self.save(next, cx)?;
        }
        Ok(())
    }

    /// The name of the model that `device` (this computer for `None`) runs for `kind`, without
    /// its maker, so `S1-mini by Superwhisper` fits its column as `S1-mini`. Every device loads
    /// the model chosen on this computer, except that a phone cleans up text with the model of
    /// its system.
    pub fn model(&self, kind: ModelKind, device: Option<&RemoteDevice>) -> Option<String> {
        let chosen = |key: &str| {
            self.storage
                .settings
                .get(key)
                .and_then(|value| value.as_str().map(|id| id.trim().to_owned()))
                .unwrap_or_default()
        };
        let catalog = hushpen_core::catalog::embedded();
        let entry = match (kind, device.map(|device| device.device.os)) {
            (ModelKind::Cleanup, Some(Os::Android)) => return Some("Gemini Nano".into()),
            (ModelKind::Cleanup, Some(Os::Ios)) => return Some("Apple Intelligence".into()),
            (ModelKind::Speech, _) => catalog.chosen_speech(&chosen("dictation.modelId")),
            (ModelKind::Cleanup, _) => catalog.chosen_llm(&chosen("cleanup.llm.modelId")),
        };
        entry.map(|entry| {
            let name = entry.name.as_str();
            name.split_once(" by ")
                .map_or(name, |(model, _)| model)
                .to_owned()
        })
    }

    pub fn opened(&mut self, cx: &mut Context<Self>) {
        self.settings = self.storage.settings.remote();
        if self.settings.enabled {
            cx.emit(Request::Probe);
        }
        cx.notify();
    }

    pub fn open_add(&mut self, cx: &mut Context<Self>) {
        self.close(cx);
        self.sheet = Some(Sheet::Add(AddMethod::Phone));
        cx.emit(Request::Discover);
        cx.notify();
    }

    pub fn select_method(&mut self, method: AddMethod, cx: &mut Context<Self>) {
        self.close(cx);
        self.sheet = Some(Sheet::Add(method));
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(Request::CancelOffer);
        self.sheet = None;
        self.pairing.apply(PairEvent::Cancel, now());
        self.qr.clear();
        self.notice = None;
        self.busy = false;
        self.selected_device = None;
        self.progress.clear();
        self.menu = None;
        self.forgetting = false;
        cx.notify();
    }

    pub fn device(&self, id: &str) -> Option<&RemoteDevice> {
        self.settings
            .devices
            .iter()
            .find(|device| device.device.id == id)
    }

    pub fn open_detail(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(device) = self.device(id) {
            let name = device.device.name.clone();
            self.name
                .update(cx, |input, cx| input.set_value(name, window, cx));
            self.sheet = Some(Sheet::Detail(id.into()));
            self.notice = None;
            self.menu = None;
            self.forgetting = false;
            cx.notify();
        }
    }

    pub fn rename(&mut self, id: &str, name: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let mut next = self.settings.clone();
        let device = next
            .devices
            .iter_mut()
            .find(|device| device.device.id == id)
            .ok_or("Device was forgotten.")?;
        device.device.name = name.trim().into();
        self.save(next, cx)
    }

    pub fn unload(
        &mut self,
        id: &str,
        rule: UnloadRule,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let mut next = self.settings.clone();
        let device = next
            .devices
            .iter_mut()
            .find(|device| device.device.id == id)
            .ok_or("Device was forgotten.")?;
        if !rule.allowed_on(device.device.os) {
            let message = "This device does not support that unload rule.".to_owned();
            self.failed(message.clone(), cx);
            return Err(message);
        }
        device.unload_rule = rule;
        self.save(next, cx)
    }

    pub fn forget(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let mut next = self.settings.clone();
        next.devices.retain(|device| device.device.id != id);
        for route in [&mut next.routes.speech, &mut next.routes.cleanup] {
            route.retain(|item| item != id);
        }
        self.save(next, cx)?;
        self.health.remove(id);
        self.close(cx);
        Ok(())
    }

    pub fn request(&mut self, request: Request, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.pairing.apply(PairEvent::Cancel, now());
        self.qr.clear();
        self.notice = None;
        self.busy = !matches!(
            request,
            Request::Discover | Request::Probe | Request::CancelOffer
        );
        self.progress.clear();
        cx.emit(request);
        cx.notify();
    }

    pub fn set_discovered(&mut self, found: Vec<FoundDevice>, cx: &mut Context<Self>) {
        self.discovered = found;
        cx.notify();
    }
    pub fn set_hosts(&mut self, hosts: Vec<String>, cx: &mut Context<Self>) {
        self.selected_host = hosts.first().cloned();
        self.hosts = hosts;
        cx.notify();
    }
    pub fn set_offer(
        &mut self,
        payload: String,
        qr: Vec<Vec<bool>>,
        expires: u64,
        cx: &mut Context<Self>,
    ) {
        self.pairing.apply(
            PairEvent::Offer {
                payload,
                expires_at: expires,
            },
            now(),
        );
        self.qr = qr;
        self.busy = false;
        cx.notify();
    }
    pub fn set_progress(&mut self, rows: Vec<String>, cx: &mut Context<Self>) {
        self.progress = rows;
        cx.notify();
    }
    pub fn set_health(&mut self, health: BTreeMap<String, Health>, cx: &mut Context<Self>) {
        self.health = health;
        cx.notify();
    }
    pub fn failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.pairing.apply(PairEvent::Reject, now());
        self.qr.clear();
        self.notice = Some(message);
        cx.notify();
    }
    pub fn add_paired(
        &mut self,
        device: RemoteDevice,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let mut next = self.storage.settings.remote();
        let id = &device.device.id;
        if next.devices.iter().any(|other| {
            other.device.id == *id && other.static_public_key != device.static_public_key
        }) {
            let message = "Forget the saved device before replacing its key.".to_owned();
            self.failed(message.clone(), cx);
            return Err(message);
        }
        next.devices.retain(|other| other.device.id != *id);
        for (kind, route) in [
            (ModelKind::Speech, &mut next.routes.speech),
            (ModelKind::Cleanup, &mut next.routes.cleanup),
        ] {
            route.retain(|item| item != id);
            if device.device.serves.contains(kind) {
                route.insert(0, id.clone());
            }
        }
        next.devices.push(device);
        self.save(next, cx)?;
        self.busy = false;
        self.close(cx);
        Ok(())
    }

    /// Whole minutes until the pairing code expires, while one is offered.
    pub fn offer_minutes(&self) -> Option<u64> {
        match self.pairing {
            Pairing::Offered { expires_at, .. } => {
                Some(expires_at.saturating_sub(now()).div_ceil(60))
            }
            _ => None,
        }
    }

    pub fn snapshot(&self) -> Value {
        let health = self
            .health
            .iter()
            .map(|(id, health)| {
                let mut health = health.clone();
                health.tick(crate::controller::monotonic_ms());
                (
                    id.clone(),
                    json!({"status":format!("{:?}",health.status),"latencyMs":health.latency_ms}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let models = |kind, ids: &[String]| {
            ids.iter()
                .map(|id| (id.clone(), json!(self.model(kind, self.device(id)))))
                .collect::<serde_json::Map<_, _>>()
        };
        let routes = &self.settings.routes;
        json!({"enabled":self.settings.enabled,"noLocalModels":self.settings.no_local_models,
            "devices":self.settings.devices.iter().map(RemoteDevice::to_json).collect::<Vec<_>>(),
            "routes":{"speech":routes.speech,"cleanup":routes.cleanup},
            "models":{"speech":models(ModelKind::Speech,&routes.speech),"cleanup":models(ModelKind::Cleanup,&routes.cleanup)},
            "sheet":match &self.sheet {None=>"closed",Some(Sheet::Add(_))=>"add",Some(Sheet::Detail(_))=>"detail"},
            "busy":self.busy,"notice":self.notice,"progress":self.progress,"health":health,
            "discovered":self.discovered.iter().map(|found| json!({"id":found.device.id,"name":found.device.name})).collect::<Vec<_>>(),
            "hosts":self.hosts,"pairing":match self.pairing {Pairing::Idle=>"idle",Pairing::Offered{..}=>"offered",Pairing::Confirming{..}=>"confirming",Pairing::Paired{..}=>"paired",Pairing::Failed(_)=>"failed"}})
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
