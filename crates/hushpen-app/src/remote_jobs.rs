//! Route jobs through paired workers, with the local worker as the final fallback.

use crate::controller::{CancelToken, Runner, monotonic_ms};
use crate::storage::Storage;
use hushpen_core::remote::{self, Choice, Health, ModelKind, Probe, THIS_COMPUTER};
use hushpen_engine::{
    EngineClient, EngineLog, Failure, JobOutcome, Link, LoadSpec, RemoteSpec, Timing,
    TranscribeSpec,
};
use hushpen_remote::{Identity, PairOffer, RemoteStream, WorkerKind};
use hushpen_store::secrets::Secrets;
use hushpen_store::settings::{RemoteDevice, RemoteSettings, Transport};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod live;
pub(crate) use live::{Capture, LiveJob};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const LOAD_TIMEOUT: Duration = Duration::from_secs(120);
const JOB_TIMEOUT: Duration = Duration::from_secs(180);

pub(crate) fn wait_load<T>(
    token: &CancelToken,
    mut receive: impl FnMut(Duration) -> Option<T>,
) -> Option<T> {
    let deadline = Instant::now() + LOAD_TIMEOUT;
    while !token.is_cancelled() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        if let Some(answer) = receive(left.min(Duration::from_millis(50))) {
            return Some(answer);
        }
    }
    None
}

/// A device added over SSH tries its direct address this long before the SSH tunnel.
const DIRECT_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub struct Router {
    pub resolver: hushpen_remote::Resolver,
    pub tunnels: crate::net::tunnel::Tunnels,
    health: Mutex<BTreeMap<String, Health>>,
    /// The route that last worked for each SSH device: true for the tunnel.
    tunneled: Mutex<BTreeMap<String, bool>>,
}

/// The order to try: a device added over SSH starts with the route that last worked, or the one
/// it paired on, and falls back to the other. Other devices have only their address.
fn ways(transport: &Transport, last: Option<bool>) -> Vec<bool> {
    match transport {
        Transport::Ssh { tunnel, .. } => {
            let first = last.unwrap_or(*tunnel);
            vec![first, !first]
        }
        Transport::Lan | Transport::Tailscale => vec![false],
    }
}

impl Router {
    /// Opens an authenticated session to a paired device, directly or through its SSH tunnel.
    pub fn connect(&self, identity: &Identity, device: &RemoteDevice) -> io::Result<RemoteStream> {
        let last = self
            .tunneled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&device.device.id)
            .copied();
        let ways = ways(&device.transport, last);
        let mut failure = None;
        for &tunnel in &ways {
            let address = if tunnel {
                let Transport::Ssh { host, .. } = &device.transport else {
                    continue;
                };
                let port = device
                    .address
                    .rsplit_once(':')
                    .and_then(|(_, port)| port.parse().ok())
                    .ok_or_else(|| io::Error::other("The saved address has no port."));
                match port.and_then(|port| self.tunnels.address(host, port)) {
                    Ok(address) => address,
                    Err(problem) => {
                        failure = Some(problem);
                        continue;
                    }
                }
            } else {
                device.address.clone()
            };
            let timeout = if ways.len() > 1 && !tunnel {
                DIRECT_TIMEOUT
            } else {
                CONNECT_TIMEOUT
            };
            match RemoteStream::connect(
                address,
                identity,
                device.static_public_key,
                None,
                timeout,
                &self.resolver,
            ) {
                Ok(session) => {
                    if ways.len() > 1 {
                        self.tunneled
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .insert(device.device.id.clone(), tunnel);
                    }
                    return Ok(session.stream);
                }
                Err(problem) => failure = Some(problem),
            }
        }
        Err(failure.unwrap_or_else(|| io::Error::other("The device has no route.")))
    }

    pub fn snapshot(&self) -> BTreeMap<String, Health> {
        let mut health = self.health.lock().unwrap_or_else(|p| p.into_inner());
        for entry in health.values_mut() {
            entry.tick(monotonic_ms());
        }
        health.clone()
    }

    pub fn probe(
        &self,
        identity: &Identity,
        devices: &[RemoteDevice],
        cancelled: &std::sync::atomic::AtomicBool,
    ) {
        for device in devices {
            if cancelled.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            let due = self
                .health
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(&device.device.id)
                .is_none_or(|health| health.probe_due(monotonic_ms()));
            if !due {
                continue;
            }
            let start = Instant::now();
            let probe = match self.connect(identity, device) {
                Ok(stream) => {
                    drop(stream);
                    Probe::Ready {
                        latency_ms: u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX),
                    }
                }
                Err(_) => Probe::Unreachable,
            };
            self.health
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .entry(device.device.id.clone())
                .or_default()
                .record(probe, monotonic_ms());
        }
    }
}

/// A job keeps the settings it started with, even if another session changes them.
pub struct Routing {
    router: Arc<Router>,
    pub settings: RemoteSettings,
    private_key: Option<[u8; 32]>,
}

pub fn local_models_allowed(values: &Map<String, Value>) -> bool {
    RemoteSettings::from_values(values).local_models_allowed()
}

pub fn identity(secrets: &Secrets) -> io::Result<Identity> {
    if let Some(key) = secrets.noise_private_key() {
        return Identity::from_private(key);
    }
    let identity = Identity::generate()?;
    secrets
        .set_noise_private_key(&identity.private())
        .map_err(io::Error::other)?;
    Ok(identity)
}

impl Routing {
    pub fn from_storage(storage: &Storage) -> Self {
        let settings = storage.settings.remote();
        let private_key = if settings.enabled {
            match identity(&storage.secrets) {
                Ok(identity) => Some(identity.private()),
                Err(error) => {
                    log::warn!("REMOTE_KEY_UNAVAILABLE {error}");
                    None
                }
            }
        } else {
            None
        };
        Self {
            router: Arc::clone(&storage.remote),
            settings,
            private_key,
        }
    }

    fn connect(&self, device: &RemoteDevice) -> io::Result<RemoteStream> {
        let key = self
            .private_key
            .ok_or_else(|| io::Error::other("device key unavailable"))?;
        let identity = Identity::from_private(key)?;
        self.router.connect(&identity, device)
    }

    /// A failed attempt backs off that device and asks core for the next route.
    pub fn run<T>(
        &self,
        kind: ModelKind,
        token: &CancelToken,
        mut remote_job: impl FnMut(&RemoteDevice, RemoteStream) -> Option<T>,
        local_job: impl FnOnce() -> T,
        mut notice: impl FnMut(String),
    ) -> Result<T, &'static str> {
        let configured = self.settings.route(kind);
        let health = &self.router.health;
        let mut attempted = BTreeSet::new();
        let mut connected = BTreeMap::new();
        let mut failed_name = None;
        loop {
            if token.is_cancelled() {
                return Err("The job was cancelled.");
            }
            let now = monotonic_ms();
            let route = remote::Route::new(
                configured
                    .ids()
                    .iter()
                    .filter(|id| !attempted.contains(*id))
                    .cloned()
                    .collect(),
                !self.settings.local_models_allowed(),
            );
            let next = {
                let mut health = health.lock().unwrap_or_else(|p| p.into_inner());
                for state in health.values_mut() {
                    state.tick(now);
                }
                remote::next_probe(&route, &health, now).map(str::to_owned)
            };
            if let Some(id) = next {
                let Some(device) = self
                    .settings
                    .devices
                    .iter()
                    .find(|device| device.device.id == id)
                else {
                    return Err("A route names an unknown device.");
                };
                let start = Instant::now();
                let probe = match self.connect(device) {
                    Ok(stream) => {
                        connected.insert(id.clone(), stream);
                        Probe::Ready {
                            latency_ms: u32::try_from(start.elapsed().as_millis())
                                .unwrap_or(u32::MAX),
                        }
                    }
                    Err(_) => {
                        attempted.insert(id.clone());
                        failed_name = Some(device.device.name.clone());
                        Probe::Unreachable
                    }
                };
                health
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .entry(id)
                    .or_default()
                    .record(probe, monotonic_ms());
                continue;
            }
            let choice = remote::pick(
                &route,
                &health.lock().unwrap_or_else(|p| p.into_inner()),
                now,
            );
            match choice {
                Choice::Refuse(reason) => return Err(reason.message()),
                Choice::Device(id) if id == THIS_COMPUTER => {
                    let name = failed_name.or_else(|| {
                        configured.ids().iter().find_map(|id| {
                            self.settings
                                .devices
                                .iter()
                                .find(|device| device.device.id == *id)
                                .map(|device| device.device.name.clone())
                        })
                    });
                    if let Some(name) = name {
                        notice(remote::fallback_notice(&name));
                    }
                    return Ok(local_job());
                }
                Choice::Device(id) => {
                    let Some(device) = self
                        .settings
                        .devices
                        .iter()
                        .find(|device| device.device.id == id)
                    else {
                        return Err("A route names an unknown device.");
                    };
                    if !attempted.insert(id.clone()) {
                        return Err("No device finished the job.");
                    }
                    let stream = connected
                        .remove(&id)
                        .map(Ok)
                        .unwrap_or_else(|| self.connect(device));
                    if let Ok(stream) = stream
                        && let Some(done) = remote_job(device, stream)
                    {
                        return Ok(done);
                    }
                    failed_name = Some(device.device.name.clone());
                    health
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .entry(id)
                        .or_default()
                        .record(Probe::Unreachable, monotonic_ms());
                }
            }
        }
    }

    pub(crate) fn speech(
        &self,
        load: LoadSpec,
        spec: TranscribeSpec,
        token: Arc<CancelToken>,
        local: Runner,
        live: Option<&Capture>,
        notice: impl FnMut(String),
    ) -> JobOutcome {
        let outcome = self.run(
            ModelKind::Speech,
            &token,
            |device, stream| {
                let client = EngineClient::start_link(
                    Link::Remote(
                        RemoteSpec::new(stream, WorkerKind::Speech)
                            .with_unload_rule(device.unload_rule),
                    ),
                    Timing::default(),
                    EngineLog::discard(),
                );
                let loading = client.load_model(load.clone());
                let loaded = wait_load(&token, |wait| loading.wait_timeout(wait));
                let outcome = if matches!(loaded, Some(Ok(_))) && !token.is_cancelled() {
                    let result = match live {
                        Some(capture) => {
                            capture.decode(&client, &spec, &token, client.status().remote_live)
                        }
                        None => live::batch(&client, &spec, &token),
                    };
                    match result {
                        Some(JobOutcome::Failed(failure))
                            if failure.code == hushpen_core::error::ENGINE_NO_SPEECH =>
                        {
                            Some(JobOutcome::Failed(failure))
                        }
                        Some(JobOutcome::Failed(_)) | None => None,
                        Some(done) => Some(done),
                    }
                } else {
                    None
                };
                client.shutdown();
                outcome
            },
            || local(spec.clone(), Arc::clone(&token)),
            notice,
        );
        match outcome {
            Ok(done) => done,
            Err(_) if token.is_cancelled() => JobOutcome::Cancelled,
            Err(reason) => JobOutcome::Failed(Failure::new(
                hushpen_core::error::ENGINE_UNAVAILABLE,
                reason,
            )),
        }
    }
}

pub fn host_address(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// The caller saves the public pin only after authentication succeeds.
pub fn authenticate_pair(
    identity: &Identity,
    bootstrap: &str,
    resolver: &hushpen_remote::Resolver,
) -> io::Result<(String, [u8; 32])> {
    let offer = PairOffer::from_bootstrap(bootstrap)?;
    let address = host_address(&offer.host, offer.port);
    let session = RemoteStream::connect(
        address.clone(),
        identity,
        offer.public_key,
        Some(&offer),
        CONNECT_TIMEOUT,
        resolver,
    )?;
    let key = session.peer_key;
    drop(session);
    Ok((address, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ssh_devices_try_the_route_that_last_worked_then_the_other() {
        let ssh = |tunnel| Transport::Ssh {
            host: "oci2".into(),
            tunnel,
        };
        assert_eq!(ways(&ssh(false), None), [false, true]);
        assert_eq!(ways(&ssh(true), None), [true, false]);
        assert_eq!(ways(&ssh(false), Some(true)), [true, false]);
        assert_eq!(ways(&ssh(true), Some(false)), [false, true]);
        assert_eq!(ways(&Transport::Lan, Some(true)), [false]);
        assert_eq!(ways(&Transport::Tailscale, None), [false]);
    }

    #[test]
    fn remote_off_keeps_local_models_even_if_the_saved_switch_is_on() {
        let mut values = Map::new();
        values.insert("remote.noLocalModels".into(), json!(true));
        assert!(local_models_allowed(&values));
        values.insert("remote.enabled".into(), json!(true));
        assert!(!local_models_allowed(&values));
    }

    #[test]
    fn no_local_models_refuses_without_calling_the_local_runner() {
        let routing = Routing {
            router: Arc::default(),
            settings: RemoteSettings {
                enabled: true,
                no_local_models: true,
                ..Default::default()
            },
            private_key: None,
        };
        let result = routing.run(
            ModelKind::Speech,
            &CancelToken::default(),
            |_, _| Some(()),
            || panic!("local model must stay off"),
            |_| {},
        );
        assert!(result.is_err());
    }

    #[test]
    fn cancelling_a_model_load_stops_waiting_before_the_next_poll() {
        let token = CancelToken::default();
        let mut polls = 0;
        let answer = wait_load::<()>(&token, |wait| {
            assert!(wait <= Duration::from_millis(50));
            polls += 1;
            token.cancel();
            None
        });
        assert_eq!(answer, None);
        assert_eq!(polls, 1);
    }
}
