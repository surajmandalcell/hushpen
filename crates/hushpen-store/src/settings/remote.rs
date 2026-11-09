//! Stored remote devices contain public pins. Private keys belong to `Secrets`.

use super::SettingsStore;
use crate::{Error, Result};
use hushpen_core::os::Os;
use hushpen_core::remote::{
    Device, DeviceKind, ModelKind, Route, Serves, THIS_COMPUTER, UnloadRule,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    Lan,
    Tailscale,
    Ssh { host: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDevice {
    pub device: Device,
    pub address: String,
    pub static_public_key: [u8; 32],
    pub transport: Transport,
    pub unload_rule: UnloadRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routes {
    pub speech: Vec<String>,
    pub cleanup: Vec<String>,
}

impl Default for Routes {
    fn default() -> Self {
        Self {
            speech: vec![THIS_COMPUTER.into()],
            cleanup: vec![THIS_COMPUTER.into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteSettings {
    pub enabled: bool,
    pub no_local_models: bool,
    pub devices: Vec<RemoteDevice>,
    pub routes: Routes,
}

impl RemoteSettings {
    /// Reads a snapshot from the settings registry. Invalid entries never become destinations.
    pub fn from_values(values: &Map<String, Value>) -> Self {
        let no_local_models = values
            .get("remote.noLocalModels")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let devices = values
            .get("remote.devices")
            .and_then(parse_devices)
            .unwrap_or_default();
        let routes = values
            .get("remote.routes")
            .and_then(parse_routes)
            .unwrap_or_default();
        let normalize = |ids: Vec<String>, kind| {
            Route::new(
                ids.into_iter()
                    .filter(|id| {
                        id == THIS_COMPUTER
                            || devices.iter().any(|device: &RemoteDevice| {
                                device.device.id == *id && device.device.serves.contains(kind)
                            })
                    })
                    .collect(),
                no_local_models,
            )
            .ids()
            .to_vec()
        };
        Self {
            enabled: values
                .get("remote.enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            no_local_models,
            routes: Routes {
                speech: normalize(routes.speech, ModelKind::Speech),
                cleanup: normalize(routes.cleanup, ModelKind::Cleanup),
            },
            devices,
        }
    }

    pub fn route(&self, kind: ModelKind) -> Route {
        if !self.enabled {
            return Route::new(Vec::new(), false);
        }
        let ids = match kind {
            ModelKind::Speech => &self.routes.speech,
            ModelKind::Cleanup => &self.routes.cleanup,
        };
        Route::new(ids.clone(), self.no_local_models)
    }

    pub fn local_models_allowed(&self) -> bool {
        !self.enabled || !self.no_local_models
    }
}

impl SettingsStore {
    pub fn remote(&self) -> RemoteSettings {
        RemoteSettings::from_values(&self.lock().values)
    }

    /// Saves a device change and both routes together, so a restart never sees half a pairing.
    pub fn set_remote(&self, remote: &RemoteSettings) -> Result<()> {
        let devices = Value::Array(remote.devices.iter().map(RemoteDevice::to_json).collect());
        let devices = sanitize_devices(&devices)
            .ok_or_else(|| Error::InvalidSetting("remote.devices".into()))?;
        let routes = routes_json(&remote.routes);
        sanitize_routes(&routes).ok_or_else(|| Error::InvalidSetting("remote.routes".into()))?;
        let mut state = self.lock();
        let mut next = state.clone();
        next.values
            .insert("remote.enabled".into(), json!(remote.enabled));
        next.values
            .insert("remote.noLocalModels".into(), json!(remote.no_local_models));
        next.values.insert("remote.devices".into(), devices);
        next.values.insert("remote.routes".into(), routes);
        normalize_routes(&mut next.values);
        self.persist(&next)?;
        *state = next;
        Ok(())
    }
}

impl RemoteDevice {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.device.id,
            "name": self.device.name,
            "kind": match self.device.kind { DeviceKind::Phone => "phone", DeviceKind::Server => "server", DeviceKind::ThisComputer => "thisComputer" },
            "os": match self.device.os { Os::Mac => "macos", Os::Linux => "linux", Os::Windows => "windows", Os::Ios => "ios", Os::Android => "android" },
            "serves": { "speech": self.device.serves.speech, "cleanup": self.device.serves.cleanup },
            "address": self.address,
            "staticPublicKey": self.static_public_key.to_vec(),
            "transport": match &self.transport { Transport::Lan => json!({"kind":"lan"}), Transport::Tailscale => json!({"kind":"tailscale"}), Transport::Ssh{host} => json!({"kind":"ssh","host":host}) },
            "unloadRule": match self.unload_rule { UnloadRule::KeepLoaded => json!({"kind":"keepLoaded"}), UnloadRule::AfterIdle(minutes) => json!({"kind":"afterIdle","minutes":minutes}), UnloadRule::WhenMemoryLow => json!({"kind":"whenMemoryLow"}), UnloadRule::WhileServerModeOpen => json!({"kind":"whileServerModeOpen"}) },
        })
    }

    fn from_json(value: &Value) -> Option<Self> {
        let fields = value.as_object()?;
        let text = |field| fields.get(field)?.as_str();
        let id = text("id")?;
        if !valid_id(id) || id == THIS_COMPUTER {
            return None;
        }
        let name = text("name")?;
        if name.trim().is_empty()
            || name.chars().count() > 128
            || name.chars().any(char::is_control)
        {
            return None;
        }
        let kind = match text("kind")? {
            "phone" => DeviceKind::Phone,
            "server" => DeviceKind::Server,
            _ => return None,
        };
        let os = match text("os")? {
            "macos" => Os::Mac,
            "linux" => Os::Linux,
            "windows" => Os::Windows,
            "ios" => Os::Ios,
            "android" => Os::Android,
            _ => return None,
        };
        let serves = fields.get("serves")?.as_object()?;
        let serves = Serves {
            speech: serves.get("speech")?.as_bool()?,
            cleanup: serves.get("cleanup")?.as_bool()?,
        };
        if !serves.speech && !serves.cleanup {
            return None;
        }
        let address = text("address")?;
        if !valid_address(address) {
            return None;
        }
        let key = fields.get("staticPublicKey")?.as_array()?;
        let static_public_key: [u8; 32] = key
            .iter()
            .map(|byte| u8::try_from(byte.as_u64()?).ok())
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?;
        if static_public_key == [0; 32] {
            return None;
        }
        let transport = fields.get("transport")?;
        let transport = match transport.get("kind")?.as_str()? {
            "lan" => Transport::Lan,
            "tailscale" => Transport::Tailscale,
            "ssh" => {
                let host = transport.get("host")?.as_str()?;
                if !valid_id(host) || host.starts_with('-') {
                    return None;
                }
                Transport::Ssh {
                    host: host.to_owned(),
                }
            }
            _ => return None,
        };
        let unload = fields.get("unloadRule")?;
        let unload_rule = match unload.get("kind")?.as_str()? {
            "keepLoaded" => UnloadRule::KeepLoaded,
            "afterIdle" => {
                UnloadRule::AfterIdle(u32::try_from(unload.get("minutes")?.as_u64()?).ok()?)
            }
            "whenMemoryLow" => UnloadRule::WhenMemoryLow,
            "whileServerModeOpen" => UnloadRule::WhileServerModeOpen,
            _ => return None,
        };
        if !unload_rule.allowed_on(os) {
            return None;
        }
        Some(Self {
            device: Device {
                id: id.to_owned(),
                name: name.to_owned(),
                kind,
                os,
                serves,
            },
            address: address.to_owned(),
            static_public_key,
            transport,
            unload_rule,
        })
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_address(address: &str) -> bool {
    if address.len() > 320 {
        return false;
    }
    let Some((host, port)) = address.rsplit_once(':') else {
        return false;
    };
    if port.parse::<u16>().ok().is_none_or(|port| port == 0) {
        return false;
    }
    if host.starts_with('[') && host.ends_with(']') {
        return host[1..host.len() - 1]
            .parse::<std::net::Ipv6Addr>()
            .is_ok();
    }
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
}

fn parse_devices(value: &Value) -> Option<Vec<RemoteDevice>> {
    let items = value.as_array()?;
    if items.len() > 128 {
        return None;
    }
    let mut seen = BTreeSet::new();
    let devices = items
        .iter()
        .map(RemoteDevice::from_json)
        .collect::<Option<Vec<_>>>()?;
    devices
        .iter()
        .all(|device| seen.insert(device.device.id.clone()))
        .then_some(devices)
}

pub(super) fn sanitize_devices(value: &Value) -> Option<Value> {
    Some(Value::Array(
        parse_devices(value)?
            .iter()
            .map(RemoteDevice::to_json)
            .collect(),
    ))
}

fn parse_routes(value: &Value) -> Option<Routes> {
    let parse = |key| {
        let items = value.get(key)?.as_array()?;
        if items.len() > 129 {
            return None;
        }
        items
            .iter()
            .map(|item| item.as_str().filter(|id| valid_id(id)).map(str::to_owned))
            .collect::<Option<Vec<_>>>()
    };
    Some(Routes {
        speech: parse("speech")?,
        cleanup: parse("cleanup")?,
    })
}

fn routes_json(routes: &Routes) -> Value {
    json!({"speech":routes.speech,"cleanup":routes.cleanup})
}

pub(super) fn sanitize_routes(value: &Value) -> Option<Value> {
    Some(routes_json(&parse_routes(value)?))
}

pub(super) fn normalize_routes(values: &mut Map<String, Value>) {
    let remote = RemoteSettings::from_values(values);
    values.insert("remote.routes".into(), routes_json(&remote.routes));
}

#[cfg(test)]
mod tests;
