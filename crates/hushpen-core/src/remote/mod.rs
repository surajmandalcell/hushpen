//! Device routes, availability, unload rules, and pairing without IO.

mod health;
mod pairing;

pub use health::{Health, Probe, Status};
pub use pairing::{PairEvent, PairFailure, Pairing};

use crate::os::Os;
use std::collections::{BTreeMap, BTreeSet};

pub const THIS_COMPUTER: &str = "this-computer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Phone,
    Server,
    ThisComputer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Serves {
    pub speech: bool,
    pub cleanup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub os: Os,
    pub serves: Serves,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Speech,
    Cleanup,
}

impl Serves {
    pub fn contains(self, kind: ModelKind) -> bool {
        match kind {
            ModelKind::Speech => self.speech,
            ModelKind::Cleanup => self.cleanup,
        }
    }
}

/// The local device can only be removed through the local-model switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    ids: Vec<String>,
}

impl Route {
    pub fn new(ids: Vec<String>, no_local_models: bool) -> Self {
        let mut seen = BTreeSet::new();
        let mut ids: Vec<_> = ids
            .into_iter()
            .filter(|id| !id.is_empty() && id != THIS_COMPUTER && seen.insert(id.clone()))
            .collect();
        if !no_local_models {
            ids.push(THIS_COMPUTER.to_owned());
        }
        Self { ids }
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Device(String),
    Refuse(Refusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NoReadyDevice,
}

impl Refusal {
    pub fn message(self) -> &'static str {
        "No device is ready. Models are disabled on this computer."
    }
}

pub fn pick(route: &Route, health: &BTreeMap<String, Health>, now: u64) -> Choice {
    route
        .ids
        .iter()
        .find(|id| {
            id.as_str() == THIS_COMPUTER || health.get(*id).is_some_and(|health| health.ready(now))
        })
        .map(|id| Choice::Device(id.clone()))
        .unwrap_or(Choice::Refuse(Refusal::NoReadyDevice))
}

/// Probe due devices before the first ready route, without waiting on a backed-off device.
pub fn next_probe<'a>(
    route: &'a Route,
    health: &BTreeMap<String, Health>,
    now: u64,
) -> Option<&'a str> {
    for id in route.ids() {
        if id == THIS_COMPUTER || health.get(id).is_some_and(|health| health.ready(now)) {
            return None;
        }
        if health.get(id).is_none_or(|health| health.probe_due(now)) {
            return Some(id);
        }
    }
    None
}

pub fn fallback_notice(device_name: &str) -> String {
    format!("Using this computer. {device_name} did not answer.")
}

pub fn working_notice(state: crate::flow_bar::BarState, notice: Option<&str>) -> Option<String> {
    matches!(
        state,
        crate::flow_bar::BarState::Transcribing | crate::flow_bar::BarState::Result
    )
    .then_some(notice)
    .flatten()
    .filter(|notice| !notice.is_empty())
    .map(str::to_owned)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnloadRule {
    KeepLoaded,
    AfterIdle(u32),
    WhenMemoryLow,
    WhileServerModeOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnloadKind {
    KeepLoaded,
    AfterIdle,
    WhenMemoryLow,
    WhileServerModeOpen,
}

impl UnloadRule {
    pub fn kind(self) -> UnloadKind {
        match self {
            Self::KeepLoaded => UnloadKind::KeepLoaded,
            Self::AfterIdle(_) => UnloadKind::AfterIdle,
            Self::WhenMemoryLow => UnloadKind::WhenMemoryLow,
            Self::WhileServerModeOpen => UnloadKind::WhileServerModeOpen,
        }
    }

    pub fn allowed_on(self, os: Os) -> bool {
        allowed_rules(os).contains(&self.kind())
            && !matches!(self, Self::AfterIdle(minutes) if !(1..=1440).contains(&minutes))
    }

    pub fn should_unload(self, idle_ms: u64, memory_low: bool, server_mode_open: bool) -> bool {
        match self {
            Self::KeepLoaded => false,
            Self::AfterIdle(minutes) => idle_ms >= u64::from(minutes) * 60_000,
            Self::WhenMemoryLow => memory_low,
            Self::WhileServerModeOpen => !server_mode_open,
        }
    }
}

pub fn allowed_rules(os: Os) -> &'static [UnloadKind] {
    use UnloadKind::*;
    match os {
        Os::Ios => &[WhileServerModeOpen],
        Os::Android | Os::Linux => &[KeepLoaded, AfterIdle, WhenMemoryLow, WhileServerModeOpen],
        Os::Mac | Os::Windows => &[KeepLoaded, AfterIdle],
    }
}

#[cfg(test)]
mod tests;
