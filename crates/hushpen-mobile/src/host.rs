mod cleanup;
mod pairing;

pub use pairing::{PairedDesktop, PairingState};

use crate::{Engine, MobileError, SystemCleanup, lock};
use hushpen_core::os::Os;
use hushpen_core::remote::{Device, DeviceKind, Serves, UnloadRule};
use hushpen_engine::{EngineLog, InProcess, Link, LlmClient, Timing};
use hushpen_host::Workers;
use hushpen_remote::{Identity, QrPayload, RemoteControl, RemoteStream, Resolver, Session};
use hushpen_store::secrets::Secrets;
use std::collections::HashMap;
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

#[uniffi::export(with_foreign)]
pub trait JobActivity: Send + Sync {
    fn changed(&self, active: bool);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum HostUnloadRule {
    KeepLoaded,
    AfterIdle { minutes: u32 },
    WhenMemoryLow,
    WhileServerModeOpen,
}

impl HostUnloadRule {
    fn core(self, os: Os) -> Result<UnloadRule, MobileError> {
        let rule = match self {
            Self::KeepLoaded => UnloadRule::KeepLoaded,
            Self::AfterIdle { minutes } if (1..=1440).contains(&minutes) => {
                UnloadRule::AfterIdle(minutes)
            }
            Self::AfterIdle { .. } => {
                return Err(MobileError::invalid("idle minutes must be 1 to 1440"));
            }
            Self::WhenMemoryLow => UnloadRule::WhenMemoryLow,
            Self::WhileServerModeOpen => UnloadRule::WhileServerModeOpen,
        };
        if !rule.allowed_on(os) {
            return Err(MobileError::invalid(
                "unload rule is unavailable on this system",
            ));
        }
        Ok(rule)
    }
}

struct Running {
    stop: Arc<AtomicBool>,
    connections: Arc<Mutex<HashMap<u64, RemoteControl>>>,
    thread: JoinHandle<()>,
    workers: Arc<Workers>,
    port: u16,
    #[cfg(unix)]
    wake: UnixStream,
    rule: UnloadRule,
    _advertisement: hushpen_remote::discovery::Advertisement,
}

#[derive(uniffi::Object)]
pub struct Host {
    engine: Arc<Engine>,
    system: Arc<SystemCleanup>,
    name: String,
    os: Os,
    identity: Arc<Identity>,
    secrets: Arc<Secrets>,
    pairing: Arc<pairing::Pairing>,
    running: Mutex<Option<Running>>,
    foreground: AtomicBool,
    activity: Mutex<Option<Arc<dyn JobActivity>>>,
}

#[uniffi::export]
impl Host {
    pub fn validate_unload_rule(&self, rule: HostUnloadRule) -> Result<(), MobileError> {
        rule.core(self.os).map(|_| ())
    }

    pub fn allowed_unload_rules(&self) -> Vec<HostUnloadRule> {
        use hushpen_core::remote::UnloadKind;
        hushpen_core::remote::allowed_rules(self.os)
            .iter()
            .map(|kind| match kind {
                UnloadKind::KeepLoaded => HostUnloadRule::KeepLoaded,
                UnloadKind::AfterIdle => HostUnloadRule::AfterIdle { minutes: 30 },
                UnloadKind::WhenMemoryLow => HostUnloadRule::WhenMemoryLow,
                UnloadKind::WhileServerModeOpen => HostUnloadRule::WhileServerModeOpen,
            })
            .collect()
    }

    #[uniffi::constructor]
    pub fn new(
        engine: Arc<Engine>,
        name: String,
        system: Arc<SystemCleanup>,
    ) -> Result<Arc<Self>, MobileError> {
        if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
            return Err(MobileError::invalid(
                "phone name must be 1 to 128 characters",
            ));
        }
        let secrets = Secrets::open(&engine.data.join("config")).map_err(MobileError::failed)?;
        let identity = match secrets.noise_private_key() {
            Some(private) => Identity::from_private(private),
            None => {
                let identity = Identity::generate().map_err(MobileError::failed)?;
                secrets
                    .set_noise_private_key(&identity.private())
                    .map_err(MobileError::failed)?;
                Ok(identity)
            }
        }
        .map_err(MobileError::failed)?;
        let pairing = Arc::new(pairing::Pairing::new(engine.data.join("config")));
        Ok(Arc::new(Self {
            engine,
            system,
            name,
            os: Os::current(),
            identity: Arc::new(identity),
            secrets: Arc::new(secrets),
            pairing,
            running: Mutex::new(None),
            foreground: AtomicBool::new(true),
            activity: Mutex::new(None),
        }))
    }

    pub fn set_job_activity(&self, activity: Option<Arc<dyn JobActivity>>) {
        let running = lock(&self.running);
        *lock(&self.activity) = activity.clone();
        if let Some(running) = running.as_ref() {
            running
                .workers
                .set_job_activity(activity_callback(activity));
        }
    }

    pub fn start(&self, listen: String, rule: HostUnloadRule) -> Result<u16, MobileError> {
        let os = self.os;
        let rule = rule.core(os)?;
        if (os == Os::Ios || rule == UnloadRule::WhileServerModeOpen)
            && !self.foreground.load(Ordering::Acquire)
        {
            return Err(MobileError::failed("keep Server mode open to serve models"));
        }
        let mut running = lock(&self.running);
        if let Some(running) = running.as_ref() {
            return Ok(running.port);
        }
        let address = listen
            .parse::<std::net::SocketAddr>()
            .map_err(MobileError::invalid)?;
        let listener = TcpListener::bind(address).map_err(MobileError::failed)?;
        listener
            .set_nonblocking(true)
            .map_err(MobileError::failed)?;
        let port = listener.local_addr().map_err(MobileError::failed)?.port();
        #[cfg(unix)]
        let (wake, wait) = UnixStream::pair().map_err(MobileError::failed)?;
        let id = key_id(&self.identity.public());
        let device = Device {
            id: id.clone(),
            name: self.name.clone(),
            kind: DeviceKind::Phone,
            os,
            serves: Serves {
                speech: true,
                cleanup: true,
            },
        };
        let advertisement = hushpen_remote::discovery::Advertisement::new(
            &device,
            &format!("hushpen-{}.local.", &id[..32]),
            port,
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(MobileError::failed)?;
        let system = Arc::clone(&self.system);
        let cleanup = LlmClient::start_link(
            Link::InProcess(InProcess::new(move |input, output| {
                cleanup::serve(input, output, Arc::clone(&system))
            })),
            Timing::llm(),
            EngineLog::open_scoped(&self.engine.data.join("logs/llm.log"), "llm")
                .map_err(MobileError::failed)?,
        );
        let workers = Workers::from_clients_with_speech_session(
            self.engine.client.clone(),
            cleanup,
            os,
            true,
            Arc::clone(&self.engine.operation),
        );
        workers.set_job_activity(activity_callback(lock(&self.activity).clone()));
        workers.set_unload_rule(rule);
        let stop = Arc::new(AtomicBool::new(false));
        let connections = Arc::new(Mutex::new(HashMap::new()));
        let thread = accept(
            listener,
            #[cfg(unix)]
            wait,
            Arc::clone(&self.identity),
            Arc::clone(&self.secrets),
            Arc::clone(&self.pairing),
            Arc::clone(&workers),
            self.engine.data.clone(),
            rule,
            Arc::clone(&stop),
            Arc::clone(&connections),
        )
        .map_err(MobileError::failed)?;
        *running = Some(Running {
            stop,
            connections,
            thread,
            workers,
            port,
            #[cfg(unix)]
            wake,
            rule,
            _advertisement: advertisement,
        });
        Ok(port)
    }

    /// Opens the Nearby window: a computer can list this phone and pick it. Closes after
    /// two minutes, after three tries, and with `close_pairing`.
    pub fn open_pairing(&self) -> Result<(), MobileError> {
        let port = self.serving_port()?;
        self.pairing.open(&self.name, port, self.identity.public())
    }

    pub fn close_pairing(&self) {
        self.pairing.close();
    }

    pub fn pairing_state(&self) -> PairingState {
        self.pairing.state()
    }

    /// Pair or Cancel on the number comparison.
    pub fn answer_pairing(&self, approve: bool) {
        self.pairing.answer(approve);
    }

    /// Scans the computer's QR. Returns the computer's name.
    pub fn pair(&self, payload: String) -> Result<String, MobileError> {
        let port = self.serving_port()?;
        let offer = QrPayload::parse(&payload).map_err(MobileError::invalid)?;
        let session = RemoteStream::connect_qr(
            match offer.host.parse::<std::net::IpAddr>() {
                Ok(ip) => std::net::SocketAddr::new(ip, offer.port).to_string(),
                Err(_) => format!("{}:{}", offer.host, offer.port),
            },
            &self.identity,
            &offer,
            Duration::from_secs(10),
            &Resolver::default(),
        )
        .map_err(MobileError::failed)?;
        self.enroll(session, port)
    }

    /// Types the computer's 8-digit code. An empty address looks for computers showing a
    /// code through Bonjour; otherwise it is `host:port`. Returns the computer's name.
    pub fn pair_code(&self, code: String, address: String) -> Result<String, MobileError> {
        let port = self.serving_port()?;
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        if code.len() != 8 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(MobileError::invalid("Enter the 8-digit code."));
        }
        let address = address.trim();
        let addresses = if address.is_empty() {
            computers_nearby()?
        } else if address.contains(':') {
            vec![address.to_owned()]
        } else {
            return Err(MobileError::invalid("Enter the address as host:port."));
        };
        let mut failure = MobileError::failed(
            "No computer is showing a code. Open Add a Device on your computer.",
        );
        for address in addresses {
            match RemoteStream::connect_code(
                address,
                &self.identity,
                &code,
                Duration::from_secs(10),
                &Resolver::default(),
            ) {
                Ok(session) => return self.enroll(session, port),
                Err(problem) => failure = MobileError::failed(problem),
            }
        }
        Err(failure)
    }

    /// A computer paired by Nearby never sends its name, so it keeps the key-based label.
    pub fn paired_clients(&self) -> Vec<PairedDesktop> {
        let names = self.pairing.names();
        self.secrets
            .paired_clients()
            .iter()
            .map(key_id)
            .map(|id| PairedDesktop {
                name: names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| format!("Desktop {}", &id[..8])),
                id,
            })
            .collect()
    }

    pub fn forget(&self, public_key: String) -> Result<(), MobileError> {
        let keys = self.secrets.paired_clients();
        if !keys.iter().any(|key| key_id(key) == public_key) {
            return Err(MobileError::invalid("paired desktop was not found"));
        }
        self.stop();
        self.secrets
            .set_paired_clients(
                &keys
                    .into_iter()
                    .filter(|key| key_id(key) != public_key)
                    .collect::<Vec<_>>(),
            )
            .map_err(MobileError::failed)?;
        self.pairing.set_name(&public_key, None)
    }

    pub fn set_foreground(&self, foreground: bool) {
        self.foreground.store(foreground, Ordering::Release);
        if !foreground
            && (self.os == Os::Ios
                || lock(&self.running)
                    .as_ref()
                    .is_some_and(|running| running.rule == UnloadRule::WhileServerModeOpen))
        {
            self.stop();
        }
    }

    pub fn memory_low(&self) {
        if let Some(running) = lock(&self.running).as_ref() {
            running.workers.memory_low();
        }
    }

    /// A local flow releases speech without unloading a serving phone.
    pub fn release_local_model(&self) -> Result<(), MobileError> {
        let running = lock(&self.running);
        if running.is_none() {
            self.engine.unload()?;
        }
        Ok(())
    }

    pub fn stop(&self) {
        self.pairing.close();
        let Some(running) = lock(&self.running).take() else {
            return;
        };
        running.stop.store(true, Ordering::Release);
        for control in lock(&running.connections).values() {
            let _ = control.shutdown();
        }
        #[cfg(unix)]
        drop(running.wake);
        let _ = running.thread.join();
        running.workers.set_job_activity(None);
        running.workers.cleanup.shutdown();
        if running.rule == UnloadRule::WhileServerModeOpen {
            self.engine.client.unload();
        }
    }
}

impl Host {
    fn serving_port(&self) -> Result<u16, MobileError> {
        lock(&self.running)
            .as_ref()
            .map(|running| running.port)
            .ok_or_else(|| MobileError::failed("start Server mode before pairing"))
    }

    /// Trusts the computer that just proved itself and tells it where this phone serves.
    fn enroll(&self, mut session: Session, port: u16) -> Result<String, MobileError> {
        let name = session.computer_name.clone().unwrap_or_default();
        let mut pins = self.secrets.paired_clients();
        if !pins.contains(&session.peer_key) {
            pins.push(session.peer_key);
        }
        self.secrets
            .set_paired_clients(&pins)
            .map_err(MobileError::failed)?;
        self.pairing
            .set_name(&key_id(&session.peer_key), Some(&name))?;
        hushpen_remote::enrollment::Enrollment {
            device: Device {
                id: key_id(&self.identity.public()),
                name: self.name.clone(),
                kind: DeviceKind::Phone,
                os: self.os,
                serves: Serves {
                    speech: true,
                    cleanup: true,
                },
            },
            port,
        }
        .send(&mut session.stream)
        .map_err(MobileError::failed)?;
        Ok(name)
    }
}

fn activity_callback(
    activity: Option<Arc<dyn JobActivity>>,
) -> Option<Arc<dyn Fn(bool) + Send + Sync>> {
    activity.map(|activity| Arc::new(move |active| activity.changed(active)) as Arc<_>)
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop();
    }
}

fn key_id(key: &[u8; 32]) -> String {
    key.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[allow(clippy::too_many_arguments)]
fn accept(
    listener: TcpListener,
    #[cfg(unix)] wake: UnixStream,
    identity: Arc<Identity>,
    secrets: Arc<Secrets>,
    pairing: Arc<pairing::Pairing>,
    workers: Arc<Workers>,
    data: std::path::PathBuf,
    rule: UnloadRule,
    stop: Arc<AtomicBool>,
    connections: Arc<Mutex<HashMap<u64, RemoteControl>>>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("phone-host".into())
        .spawn(move || {
            let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let mut next_id = 0_u64;
            while !stop.load(Ordering::Acquire) {
                #[cfg(unix)]
                match wait_for_connection(&listener, &wake) {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(problem) => {
                        log::debug!("PHONE_LISTENER_ENDED {problem}");
                        break;
                    }
                }
                let socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(problem) if problem.kind() == std::io::ErrorKind::WouldBlock => {
                        #[cfg(not(unix))]
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    Err(problem) if problem.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                if stop.load(Ordering::Acquire) {
                    break;
                }
                if active.load(Ordering::Acquire) >= 8 {
                    continue;
                }
                let id = next_id;
                next_id = next_id.wrapping_add(1);
                let (identity, secrets, pairing) = (
                    Arc::clone(&identity),
                    Arc::clone(&secrets),
                    Arc::clone(&pairing),
                );
                let workers = Arc::clone(&workers);
                let data = data.clone();
                let active = Arc::clone(&active);
                let stop = Arc::clone(&stop);
                let connections = Arc::clone(&connections);
                active.fetch_add(1, Ordering::AcqRel);
                // A Nearby comparison waits for a person, so each connection has its own thread.
                std::thread::spawn(move || {
                    if let Some(session) = handshake(socket, &identity, &secrets, &pairing) {
                        let mut controls = lock(&connections);
                        controls.insert(id, session.stream.control());
                        if stop.load(Ordering::Acquire) {
                            for control in controls.values() {
                                let _ = control.shutdown();
                            }
                        } else {
                            drop(controls);
                            if let Err(problem) =
                                hushpen_host::serve_session(session.stream, workers, &data, rule)
                            {
                                log::debug!("PHONE_SESSION_ENDED {problem}");
                            }
                        }
                        lock(&connections).remove(&id);
                    }
                    active.fetch_sub(1, Ordering::AcqRel);
                });
            }
        })
}

/// A paired desktop gets its session. While the Nearby window is open, a new desktop may
/// compare numbers on this connection and be saved; it gets no session.
fn handshake(
    socket: std::net::TcpStream,
    identity: &Identity,
    secrets: &Secrets,
    pairing: &pairing::Pairing,
) -> Option<Session> {
    let (mut offer, generation) = match pairing.take() {
        Some((offer, generation)) => (Some(offer), generation),
        None => (None, 0),
    };
    let mut compared = false;
    let timeout = Duration::from_secs(if offer.is_some() { 120 } else { 2 });
    let result = RemoteStream::accept_pairing_with(
        socket,
        identity,
        &secrets.paired_clients(),
        offer.as_mut(),
        timeout,
        |number, _| {
            compared = true;
            pairing.compare(number)
        },
        |key| {
            let mut pins = secrets.paired_clients();
            if !pins.contains(&key) {
                pins.push(key);
            }
            secrets
                .set_paired_clients(&pins)
                .map_err(std::io::Error::other)
        },
    );
    match (result, offer) {
        (Ok(session), _) if session.paired => {
            pairing.finish(generation, Ok(()));
            None
        }
        (Ok(session), offer) => {
            if let Some(offer) = offer {
                pairing.restore(offer, generation);
            }
            Some(session)
        }
        (Err(_), Some(_)) if compared => {
            pairing.finish(
                generation,
                Err("Pairing was cancelled or the numbers did not match.".into()),
            );
            None
        }
        (Err(_), offer) => {
            if let Some(offer) = offer {
                pairing.restore(offer, generation);
            }
            None
        }
    }
}

/// Computers showing a pairing code now, found through Bonjour.
fn computers_nearby() -> Result<Vec<String>, MobileError> {
    let discovery = hushpen_remote::discovery::Discovery::new().map_err(MobileError::failed)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut found = Vec::new();
    while std::time::Instant::now() < deadline {
        if let Some(device) = discovery
            .next(Duration::from_millis(100))
            .map_err(MobileError::failed)?
            && device.device.kind != DeviceKind::Phone
        {
            found.extend(device.addresses.iter().map(|address| address.to_string()));
        }
    }
    Ok(found)
}

#[cfg(unix)]
fn wait_for_connection(listener: &TcpListener, wake: &UnixStream) -> std::io::Result<bool> {
    use rustix::event::{PollFd, PollFlags, poll};
    let mut events = [
        PollFd::new(listener, PollFlags::IN),
        PollFd::new(wake, PollFlags::IN),
    ];
    loop {
        match poll(&mut events, None) {
            Err(rustix::io::Errno::INTR) => continue,
            Err(problem) => return Err(problem.into()),
            Ok(_) => {}
        }
        if !events[1].revents().is_empty() {
            return Ok(false);
        }
        if events[0].revents().contains(PollFlags::IN) {
            return Ok(true);
        }
        return Err(std::io::Error::other("phone listener closed"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MobileError, SystemLlm};
    use hushpen_engine::{LlmOutcome, LlmSpec, LlmState, LlmWork, RemoteSpec};
    use hushpen_remote::WorkerKind;

    struct System(bool);
    impl SystemLlm for System {
        fn available(&self) -> bool {
            self.0
        }
        fn cleanup(&self, text: String, _: String) -> Result<String, MobileError> {
            Ok(format!("{}!", text.trim_end_matches('.')))
        }
    }

    #[cfg(unix)]
    #[test]
    fn idle_listener_cancellation_needs_no_network_connection() {
        for _ in 0..3 {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let (wake, wait) = UnixStream::pair().unwrap();
            let (sent, received) = std::sync::mpsc::channel();
            let thread = std::thread::spawn(move || {
                sent.send(wait_for_connection(&listener, &wait)).unwrap();
            });
            assert!(matches!(
                received.recv_timeout(Duration::from_millis(20)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            drop(wake);
            assert!(
                !received
                    .recv_timeout(Duration::from_secs(1))
                    .unwrap()
                    .unwrap()
            );
            thread.join().unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn listener_cancellation_before_wait_does_not_lose_the_wake() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let (wake, wait) = UnixStream::pair().unwrap();
        drop(wake);
        let (sent, received) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            sent.send(wait_for_connection(&listener, &wait)).unwrap();
        });
        assert!(
            !received
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
        );
        thread.join().unwrap();
    }

    fn host(data: &std::path::Path, os: Os) -> Arc<Host> {
        let engine = Engine::new(data.to_string_lossy().into_owned()).unwrap();
        let mut host = Host::new(
            engine,
            "Test phone".into(),
            SystemCleanup::new(Arc::new(System(true))),
        )
        .unwrap();
        Arc::get_mut(&mut host).unwrap().os = os;
        host
    }

    fn pair_desktop(host: &Host) -> Identity {
        let desktop = Identity::generate().unwrap();
        let identity = Identity::from_private(desktop.private()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut offer = hushpen_remote::PairOffer::new(
            "127.0.0.1".into(),
            listener.local_addr().unwrap().port(),
            desktop.public(),
            hushpen_store::time::now_unix_ms() / 1000 + 120,
        )
        .unwrap();
        offer.open_window("Test desktop").unwrap();
        let payload = offer.qr_payload();
        let peer = std::thread::spawn(move || {
            let mut offer = offer;
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(problem)
                        if problem.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(problem) => panic!("pairing listener failed: {problem}"),
                }
            };
            let mut session = RemoteStream::accept(
                socket,
                &identity,
                &[],
                Some(&mut offer),
                Duration::from_secs(3),
            )
            .unwrap();
            let enrollment = hushpen_remote::enrollment::Enrollment::receive(
                &mut session.stream,
                &session.peer_key,
            )
            .unwrap();
            assert_eq!(enrollment.device.kind, DeviceKind::Phone);
            assert!(enrollment.device.serves.cleanup);
            enrollment.port
        });
        host.pair(payload).unwrap();
        assert_eq!(
            peer.join().unwrap(),
            lock(&host.running).as_ref().unwrap().port
        );
        desktop
    }

    fn connect(host: &Host, desktop: &Identity) -> RemoteStream {
        let port = lock(&host.running).as_ref().unwrap().port;
        RemoteStream::connect(
            format!("127.0.0.1:{port}"),
            desktop,
            host.identity.public(),
            None,
            Duration::from_secs(3),
            &Resolver::default(),
        )
        .unwrap()
        .stream
    }

    #[test]
    fn paired_cleanup_serves_over_noise_and_stopping_keeps_local_speech_ready() {
        let directory = tempfile::tempdir().unwrap();
        let host = host(directory.path(), Os::Android);
        let port = host
            .start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        let desktop = pair_desktop(&host);
        let client = LlmClient::start_link(
            Link::Remote(RemoteSpec::new(
                connect(&host, &desktop),
                WorkerKind::Cleanup,
            )),
            Timing::default(),
            EngineLog::discard(),
        );
        assert!(
            client.wait_for(Duration::from_secs(3), |status| status.state
                == LlmState::Ready)
        );
        assert!(
            client
                .load_model(LlmSpec::new("Qwen3.5-2B-Q4_K_M.gguf".into()))
                .wait_timeout(Duration::from_secs(3))
                .unwrap()
                .is_ok()
        );
        let result = client
            .run(LlmWork::Cleanup {
                system: "Clean this text".into(),
                user: "Hello there.".into(),
                max_tokens: 32,
            })
            .wait_timeout(Duration::from_secs(3))
            .unwrap();
        assert!(matches!(result, LlmOutcome::Done(done) if done.text == "Hello there!"));
        host.stop();
        client.shutdown();
        assert!(
            host.engine
                .client
                .wait_for(Duration::from_secs(2), |status| status.state
                    == hushpen_engine::EngineState::Ready)
        );
        assert!(host.engine.client.status().pid.is_none());
        let probe = TcpListener::bind(("127.0.0.1", port)).unwrap();
        drop(probe);
        host.start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        host.forget(key_id(&desktop.public())).unwrap();
        assert!(host.paired_clients().is_empty());
        let port = host
            .start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        assert!(
            RemoteStream::connect(
                format!("127.0.0.1:{port}"),
                &desktop,
                host.identity.public(),
                None,
                Duration::from_secs(2),
                &Resolver::default()
            )
            .is_err()
        );
        host.stop();
        host.engine.shutdown();
    }

    fn wait_for(host: &Host, done: impl Fn(&PairingState) -> bool) -> PairingState {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let state = host.pairing_state();
            if done(&state) {
                return state;
            }
            assert!(std::time::Instant::now() < deadline, "stuck in {state:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn nearby_pairing_needs_the_window_and_both_taps_and_saves_one_desktop() {
        let directory = tempfile::tempdir().unwrap();
        let host = host(directory.path(), Os::Android);
        assert!(host.open_pairing().is_err());
        let port = host
            .start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        assert_eq!(host.pairing_state(), PairingState::Closed);
        let desktop = Identity::generate().unwrap();
        let mut address = format!("127.0.0.1:{port}");
        let nearby = |approve: bool, address: &str| {
            let (address, desktop) = (
                address.to_owned(),
                Identity::from_private(desktop.private()).unwrap(),
            );
            std::thread::spawn(move || {
                let mut shown = String::new();
                let result = RemoteStream::connect_nearby(
                    address,
                    &desktop,
                    Duration::from_secs(10),
                    &Resolver::default(),
                    |number, name| {
                        assert_eq!(name, "Test phone");
                        shown = number.into();
                        Ok(approve)
                    },
                );
                (result.is_ok(), shown)
            })
        };
        // A closed window refuses the desktop.
        assert!(!nearby(true, &address).join().unwrap().0);
        host.open_pairing().unwrap();
        assert_eq!(host.pairing_state(), PairingState::Listening);
        let peer = nearby(true, &address);
        let PairingState::Comparing { number } =
            wait_for(&host, |s| matches!(s, PairingState::Comparing { .. }))
        else {
            unreachable!()
        };
        assert!(host.paired_clients().is_empty());
        host.answer_pairing(true);
        let (paired, shown) = peer.join().unwrap();
        assert!(paired);
        assert_eq!(shown, number);
        assert_eq!(number.len(), 6);
        wait_for(&host, |s| *s == PairingState::Paired);
        let clients = host.paired_clients();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].id, key_id(&desktop.public()));
        assert!(clients[0].name.starts_with("Desktop "));
        // Cancel on the phone saves nothing, and a fresh window works again.
        host.forget(clients[0].id.clone()).unwrap();
        let port = host
            .start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        address = format!("127.0.0.1:{port}");
        host.open_pairing().unwrap();
        let peer = nearby(true, &address);
        wait_for(&host, |s| matches!(s, PairingState::Comparing { .. }));
        host.answer_pairing(false);
        assert!(!peer.join().unwrap().0);
        assert!(matches!(
            wait_for(&host, |s| matches!(s, PairingState::Failed { .. })),
            PairingState::Failed { .. }
        ));
        assert!(host.paired_clients().is_empty());
        // Closing the sheet closes the window and any comparison under way.
        host.open_pairing().unwrap();
        let peer = nearby(true, &address);
        wait_for(&host, |s| matches!(s, PairingState::Comparing { .. }));
        host.close_pairing();
        assert!(!peer.join().unwrap().0);
        assert_eq!(host.pairing_state(), PairingState::Closed);
        assert!(host.paired_clients().is_empty());
        host.stop();
        host.engine.shutdown();
    }

    #[test]
    fn a_typed_code_pairs_with_a_named_computer_and_wrong_codes_do_not() {
        let directory = tempfile::tempdir().unwrap();
        let host = host(directory.path(), Os::Android);
        host.start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        let desktop = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let mut offer = hushpen_remote::PairOffer::new(
            "127.0.0.1".into(),
            listener.local_addr().unwrap().port(),
            desktop.public(),
            hushpen_store::time::now_unix_ms() / 1000 + 120,
        )
        .unwrap();
        offer.open_window("Studio Mac").unwrap();
        let code = offer.code().to_owned();
        let peer = std::thread::spawn(move || {
            for _ in 0..2 {
                let (socket, _) = listener.accept().unwrap();
                let session = RemoteStream::accept(
                    socket,
                    &desktop,
                    &[],
                    Some(&mut offer),
                    Duration::from_secs(5),
                );
                if let Ok(mut session) = session {
                    hushpen_remote::enrollment::Enrollment::receive(
                        &mut session.stream,
                        &session.peer_key,
                    )
                    .unwrap();
                }
            }
        });
        let wrong = if code == "00000000" {
            "11111111"
        } else {
            "00000000"
        };
        assert!(host.pair_code(wrong.into(), address.clone()).is_err());
        assert!(host.pair_code("12".into(), address.clone()).is_err());
        assert!(host.pair_code(code.clone(), "no-port".into()).is_err());
        assert_eq!(
            host.pair_code(format!("{} {}", &code[..4], &code[4..]), address)
                .unwrap(),
            "Studio Mac"
        );
        peer.join().unwrap();
        assert_eq!(host.paired_clients()[0].name, "Studio Mac");
        host.stop();
        host.engine.shutdown();
    }

    #[test]
    fn ios_background_transition_stops_and_refuses_server_mode() {
        let directory = tempfile::tempdir().unwrap();
        let host = host(directory.path(), Os::Ios);
        assert_eq!(
            host.allowed_unload_rules(),
            [HostUnloadRule::WhileServerModeOpen]
        );
        host.start("127.0.0.1:0".into(), HostUnloadRule::WhileServerModeOpen)
            .unwrap();
        host.set_foreground(false);
        assert!(lock(&host.running).is_none());
        assert!(
            host.start("127.0.0.1:0".into(), HostUnloadRule::WhileServerModeOpen)
                .is_err()
        );
        host.set_foreground(true);
        host.start("127.0.0.1:0".into(), HostUnloadRule::WhileServerModeOpen)
            .unwrap();
        host.stop();
        host.engine.shutdown();
    }

    #[test]
    #[ignore = "requires HUSHPEN_MOBILE_TEST_MODELS and HUSHPEN_MOBILE_TEST_WAV"]
    fn real_speech_runs_locally_and_through_the_paired_phone_host() {
        let directory = tempfile::tempdir().unwrap();
        let host = host(directory.path(), Os::Android);
        let model = crate::models::phone_model("base-q5_1").unwrap();
        let pin = &model.files[0];
        let source = std::path::PathBuf::from(
            std::env::var_os("HUSHPEN_MOBILE_TEST_MODELS")
                .expect("set the verified speech model folder"),
        )
        .join(&pin.file);
        let destination = directory.path().join("models/whisper").join(&pin.file);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::hard_link(&source, &destination)
            .or_else(|_| std::fs::copy(&source, &destination).map(|_| ()))
            .unwrap();
        assert!(matches!(
            hushpen_store::model_files::verify(&destination, pin.bytes, &pin.sha256, |_| true)
                .unwrap(),
            hushpen_store::model_files::Verdict::Ready { .. }
        ));
        let wav = std::path::PathBuf::from(
            std::env::var_os("HUSHPEN_MOBILE_TEST_WAV").expect("set the spoken WAV fixture"),
        );
        host.engine.load(model.id.clone()).unwrap();
        let local = host
            .engine
            .transcribe(wav.to_string_lossy().into_owned(), Some("en".into()))
            .unwrap();
        assert!(!local.text.trim().is_empty());
        host.start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        let desktop = pair_desktop(&host);
        let client = hushpen_engine::EngineClient::start_link(
            Link::Remote(RemoteSpec::new(
                connect(&host, &desktop),
                WorkerKind::Speech,
            )),
            Timing::default(),
            EngineLog::discard(),
        );
        assert!(
            client
                .load_model(hushpen_engine::LoadSpec {
                    path: destination,
                    gpu: false,
                    threads: 2,
                    idle: hushpen_core::idle::Idle::KEEP
                })
                .wait_timeout(Duration::from_secs(120))
                .unwrap()
                .is_ok()
        );
        let remote = client
            .transcribe(hushpen_engine::TranscribeSpec {
                wav_path: wav,
                language: Some("en".into()),
                prompt: None,
            })
            .wait_timeout(Duration::from_secs(120))
            .unwrap();
        assert!(
            matches!(remote, hushpen_engine::JobOutcome::Done(done) if done.text == local.text)
        );
        host.stop();
        client.shutdown();
        host.engine.shutdown();
    }

    #[test]
    fn unload_rules_follow_shared_platform_policy() {
        assert!(HostUnloadRule::WhileServerModeOpen.core(Os::Ios).is_ok());
        assert!(HostUnloadRule::KeepLoaded.core(Os::Ios).is_err());
        assert!(HostUnloadRule::WhenMemoryLow.core(Os::Android).is_ok());
        assert!(
            HostUnloadRule::AfterIdle { minutes: 0 }
                .core(Os::Android)
                .is_err()
        );
        assert!(
            HostUnloadRule::AfterIdle { minutes: 1440 }
                .core(Os::Android)
                .is_ok()
        );
        assert!(
            HostUnloadRule::AfterIdle { minutes: 1441 }
                .core(Os::Android)
                .is_err()
        );
    }

    #[test]
    fn android_open_only_rule_refuses_a_start_after_the_app_closes() {
        let directory = tempfile::tempdir().unwrap();
        let host = host(directory.path(), Os::Android);
        assert!(
            host.validate_unload_rule(HostUnloadRule::AfterIdle { minutes: 0 })
                .is_err()
        );
        host.set_foreground(false);
        assert!(
            host.start("127.0.0.1:0".into(), HostUnloadRule::WhileServerModeOpen)
                .is_err()
        );
        host.start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        host.stop();
        host.engine.shutdown();
    }

    #[test]
    fn pairing_keeps_cleanup_capable_when_the_system_model_is_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let mut host = host(directory.path(), Os::Android);
        Arc::get_mut(&mut host).unwrap().system = SystemCleanup::new(Arc::new(System(false)));
        assert!(!host.system.available());
        host.start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        let _desktop = pair_desktop(&host);
        host.stop();
        host.engine.shutdown();
    }

    #[test]
    fn a_desktop_speech_session_excludes_local_model_changes() {
        let directory = tempfile::tempdir().unwrap();
        let host = host(directory.path(), Os::Android);
        host.start("127.0.0.1:0".into(), HostUnloadRule::KeepLoaded)
            .unwrap();
        let desktop = pair_desktop(&host);
        let client = hushpen_engine::EngineClient::start_link(
            Link::Remote(RemoteSpec::new(
                connect(&host, &desktop),
                WorkerKind::Speech,
            )),
            Timing::default(),
            EngineLog::discard(),
        );
        assert!(
            client.wait_for(Duration::from_secs(3), |status| status.state
                == hushpen_engine::EngineState::Ready)
        );
        assert_eq!(
            host.engine
                .load("base-q5_1".into())
                .unwrap_err()
                .to_string(),
            "speech engine is busy"
        );
        assert!(host.engine.unload().is_err());
        assert!(host.release_local_model().is_ok());
        client.shutdown();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while host.engine.operation.try_lock().is_err() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(host.engine.unload().is_ok());
        host.stop();
        {
            let _operation = lock(&host.engine.operation);
            assert!(host.release_local_model().is_err());
        }
        host.engine.shutdown();
    }
}
