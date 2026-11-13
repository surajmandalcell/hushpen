//! Device setup runs outside the UI thread and returns authenticated settings.

use crate::devices::{Devices, Request};
use crate::{device_ssh, remote_jobs, storage::Storage};
use gpui_kit::{App, Entity};
use hushpen_core::os::Os;
use hushpen_core::remote::{
    Device, DeviceKind, Health, RemoteHost, Serves, UnloadRule, remote_hosts, ssh_hosts,
};
use hushpen_remote::discovery::{Advertisement, Discovery, FoundDevice};
use hushpen_remote::{Identity, PairOffer, RemoteStream};
use hushpen_store::settings::{RemoteDevice, Transport};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The protocol caps pre-authentication at five seconds; only comparison gets this long.
const PAIR_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_PAIR_CONNECTIONS: usize = 4;

#[derive(Default)]
struct Pairing {
    selected: Mutex<Option<String>>,
    decision: Mutex<Option<(String, mpsc::Sender<bool>)>>,
    /// One number owns the prompt until both sides have answered.
    comparing: Mutex<()>,
}

impl Pairing {
    fn answer(&self, number: &str, accept: bool) {
        let mut pending = self.decision.lock().unwrap_or_else(|e| e.into_inner());
        if pending
            .as_ref()
            .is_some_and(|(current, _)| current == number)
            && let Some((_, sender)) = pending.take()
        {
            let _ = sender.send(accept);
        }
    }
}

enum Update {
    Found(Vec<FoundDevice>),
    Hosts(Vec<RemoteHost>),
    Offer {
        payload: String,
        qr: Vec<Vec<bool>>,
        code: String,
        expires: u64,
    },
    /// The numbers match so far; the user decides.
    Confirming {
        number: String,
        name: String,
    },
    /// A try ended and the offer stays up with `tries` left.
    Attempt {
        message: String,
        tries: u8,
        number: Option<String>,
    },
    Expired,
    Progress(String),
    Paired(RemoteDevice),
    Health(BTreeMap<String, Health>),
    Failed(String),
    Done,
}

pub fn attach(devices: &Entity<Devices>, storage: Rc<Storage>, cx: &mut App) {
    // The pairing window and setups share one job; looking for phones and machines never
    // stops it.
    let cancel = Rc::new(RefCell::new(Arc::new(AtomicBool::new(false))));
    let lookup = Rc::new(RefCell::new(Arc::new(AtomicBool::new(false))));
    let decision: Arc<Pairing> = Arc::default();
    cx.subscribe(devices, move |devices, request: &Request, cx| {
        if matches!(request, Request::CancelOffer) {
            cancel.borrow().store(true, Ordering::Release);
            return;
        }
        if let Request::Confirm { number, accept } = request {
            decision.answer(number, *accept);
            return;
        }
        if let Request::SelectNearby(id) = request {
            *decision.selected.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());
            return;
        }
        let side = matches!(request, Request::Discover | Request::Hosts);
        if !side {
            *decision.selected.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        let cancel = if side { &lookup } else { &cancel };
        cancel.borrow().store(true, Ordering::Release);
        let stopped = Arc::new(AtomicBool::new(false));
        *cancel.borrow_mut() = Arc::clone(&stopped);
        let decision = Arc::clone(&decision);
        let identity = match remote_jobs::identity(&storage.secrets) {
            Ok(identity) => identity,
            Err(_) => {
                devices.update(cx, |view, cx| {
                    view.failed("The device key could not be saved.".into(), cx)
                });
                return;
            }
        };
        let request = request.clone();
        let settings = storage.settings.remote();
        let router = Arc::clone(&storage.remote);
        let secrets = Arc::clone(&storage.secrets);
        let (sender, receiver) = mpsc::sync_channel(32);
        std::thread::spawn(move || {
            if let Err(error) = run(
                request,
                identity,
                settings.devices,
                router,
                &secrets,
                &decision,
                &stopped,
                &sender,
            ) && !side
                && !stopped.load(Ordering::Acquire)
            {
                let _ = sender.send(Update::Failed(error.to_string()));
            }
            let _ = sender.send(Update::Done);
        });
        let devices = devices.downgrade();
        let generation = Rc::clone(cancel);
        let token = Arc::clone(&*cancel.borrow());
        cx.spawn(async move |cx| {
            let deadline = Instant::now() + Duration::from_secs(300);
            let mut progress = Vec::new();
            loop {
                if token.load(Ordering::Acquire) || !Arc::ptr_eq(&token, &generation.borrow()) {
                    break;
                }
                let update = match receiver.try_recv() {
                    Ok(Update::Done) => break,
                    Ok(update) => update,
                    Err(TryRecvError::Disconnected) => break,
                    Err(TryRecvError::Empty) if Instant::now() < deadline => {
                        cx.background_executor()
                            .timer(Duration::from_millis(25))
                            .await;
                        continue;
                    }
                    Err(TryRecvError::Empty) => {
                        token.store(true, Ordering::Release);
                        Update::Failed("Device setup timed out. Try again.".into())
                    }
                };
                let finished = matches!(&update, Update::Failed(_));
                if devices
                    .update(cx, |view, cx| match update {
                        Update::Found(found) => view.set_discovered(found, cx),
                        Update::Hosts(hosts) => view.set_hosts(hosts, cx),
                        Update::Offer {
                            payload,
                            qr,
                            code,
                            expires,
                        } => view.set_offer(payload, qr, code, expires, cx),
                        Update::Confirming { number, name } => {
                            view.set_confirming(number, name, cx)
                        }
                        Update::Attempt {
                            message,
                            tries,
                            number,
                        } => view.attempt_failed(message, tries, number.as_deref(), cx),
                        Update::Expired => view.expired(cx),
                        Update::Progress(step) => {
                            progress.push(step);
                            view.set_progress(progress.clone(), cx);
                        }
                        Update::Paired(device) => {
                            if let Err(error) = view.add_paired(device, cx) {
                                view.failed(error, cx);
                            }
                        }
                        Update::Health(health) => view.set_health(health, cx),
                        Update::Failed(error) => view.failed(error, cx),
                        Update::Done => {}
                    })
                    .is_err()
                {
                    token.store(true, Ordering::Release);
                    break;
                }
                if finished {
                    break;
                }
            }
        })
        .detach();
    })
    .detach();
}

#[expect(clippy::too_many_arguments, reason = "the parts of one device job")]
fn run(
    request: Request,
    identity: Identity,
    devices: Vec<RemoteDevice>,
    router: Arc<remote_jobs::Router>,
    secrets: &hushpen_store::secrets::Secrets,
    decision: &Pairing,
    stopped: &AtomicBool,
    sender: &SyncSender<Update>,
) -> io::Result<()> {
    match request {
        Request::Confirm { .. } | Request::SelectNearby(_) | Request::CancelOffer => Ok(()),
        Request::Hosts => {
            let config = std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .and_then(|home| std::fs::read_to_string(home.join(".ssh/config")).ok())
                .unwrap_or_default();
            let status = device_ssh::tailscale_status(stopped);
            // A status that does not parse leaves the SSH hosts.
            let hosts = remote_hosts(&config, &status).unwrap_or_else(|_| ssh_hosts(&config));
            send(sender, Update::Hosts(hosts))
        }
        Request::Discover => {
            let discovery = Discovery::new()?;
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut found = BTreeMap::new();
            while Instant::now() < deadline && !stopped.load(Ordering::Acquire) {
                if let Some(device) = discovery.next(Duration::from_millis(100))? {
                    found.insert(device.device.id.clone(), device);
                    send(sender, Update::Found(found.values().cloned().collect()))?;
                }
            }
            send(sender, Update::Found(found.into_values().collect()))
        }
        Request::Offer { host } => offer(&identity, host, secrets, decision, stopped, sender),
        Request::Pair {
            bootstrap,
            device,
            transport,
        } => {
            let (address, static_public_key) =
                remote_jobs::authenticate_pair(&identity, &bootstrap, &router.resolver)?;
            send(
                sender,
                Update::Paired(paired(device, address, static_public_key, transport)),
            )
        }
        Request::SetupSsh { host } => {
            let source = ssh_source()?;
            let uri = device_ssh::setup(&host, &source, stopped, |step| {
                let _ = sender.try_send(Update::Progress(step.label().into()));
            })?;
            let (address, key) = pair_ssh(
                &identity,
                &uri,
                device_ssh::hostname(&host, stopped),
                device_ssh::tailscale(&host, stopped),
                &router.resolver,
            )?;
            let device = Device {
                id: key_id(&key),
                name: host.clone(),
                kind: DeviceKind::Server,
                os: Os::Linux,
                serves: Serves {
                    speech: true,
                    cleanup: true,
                },
            };
            send(
                sender,
                Update::Paired(paired(device, address, key, Transport::Ssh { host })),
            )
        }
        Request::Probe => {
            router.probe(&identity, &devices, stopped);
            send(sender, Update::Health(router.snapshot()))
        }
    }
}

fn pair_ssh(
    identity: &Identity,
    bootstrap: &str,
    hostname: io::Result<String>,
    tailscale: io::Result<String>,
    resolver: &hushpen_remote::Resolver,
) -> io::Result<(String, [u8; 32])> {
    let mut offer = PairOffer::from_bootstrap(bootstrap)?;
    let optional = |result: io::Result<String>| match result {
        Ok(output) => Ok(Some(output)),
        Err(problem) if problem.kind() == io::ErrorKind::Interrupted => Err(problem),
        Err(_) => Ok(None),
    };
    let hostname = optional(hostname)?;
    let tailscale = optional(tailscale)?;
    let hosts = hushpen_core::remote::ssh_pair_hosts(
        hostname.as_deref(),
        tailscale.as_deref().unwrap_or_default(),
        &offer.host,
    );
    let mut failure = io::Error::other("The server did not answer at any address.");
    for host in hosts {
        offer.host = host;
        match remote_jobs::authenticate_pair(identity, &offer.to_bootstrap(), resolver) {
            Ok(paired) => return Ok(paired),
            Err(problem) => failure = problem,
        }
    }
    Err(failure)
}

fn ssh_source() -> io::Result<device_ssh::Source> {
    #[cfg(all(debug_assertions, feature = "test-automation"))]
    if let Some(archive) = std::env::var_os("HUSHPEN_TEST_HOST_ARCHIVE") {
        let sha256 = std::env::var("HUSHPEN_TEST_HOST_SHA256").map_err(io::Error::other)?;
        let architecture = std::env::var("HUSHPEN_TEST_HOST_ARCH").map_err(io::Error::other)?;
        return Ok(device_ssh::Source::Local {
            archive: archive.into(),
            sha256,
            architecture,
        });
    }
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("The app folder is unavailable."))?;
    device_ssh::source(directory, env!("CARGO_PKG_VERSION"))
}

fn paired(
    device: Device,
    address: String,
    static_public_key: [u8; 32],
    transport: Transport,
) -> RemoteDevice {
    let unload_rule = if device.os == Os::Ios {
        UnloadRule::WhileServerModeOpen
    } else {
        UnloadRule::KeepLoaded
    };
    RemoteDevice {
        device,
        address,
        static_public_key,
        transport,
        unload_rule,
    }
}

fn offer(
    identity: &Identity,
    host: String,
    secrets: &hushpen_store::secrets::Secrets,
    decision: &Pairing,
    stopped: &AtomicBool,
    sender: &SyncSender<Update>,
) -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:0")?;
    listener.set_nonblocking(true)?;
    let id = key_id(&identity.public())[..32].to_owned();
    let hostname = format!("hushpen-{id}.local.");
    let host = if host.is_empty() {
        hostname.trim_end_matches('.').to_owned()
    } else {
        host
    };
    let device = Device {
        id,
        name: computer_name()?,
        kind: DeviceKind::ThisComputer,
        os: Os::current(),
        serves: Serves::default(),
    };
    let port = listener.local_addr()?.port();
    let _advertisement = Advertisement::new(&device, &hostname, port, env!("CARGO_PKG_VERSION"))?;
    let expires = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs()
        + 120;
    let mut offer = PairOffer::new(host, port, identity.public(), expires)?;
    offer.open_window(&device.name)?;
    let payload = offer.qr_payload();
    let qr = qrcode::QrCode::new(payload.as_bytes()).map_err(io::Error::other)?;
    let width = qr.width();
    let bits = (0..width)
        .map(|y| {
            (0..width)
                .map(|x| qr[(x, y)] == qrcode::Color::Dark)
                .collect()
        })
        .collect();
    send(
        sender,
        Update::Offer {
            payload,
            qr: bits,
            code: offer.code().to_owned(),
            expires,
        },
    )?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let window = offer.window();
    let finished = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while Instant::now() < deadline
                && !stopped.load(Ordering::Acquire)
                && !finished.load(Ordering::Acquire)
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            window.close();
        });
        let result = (|| {
            let mut connections: Vec<
                std::thread::ScopedJoinHandle<'_, io::Result<Option<RemoteDevice>>>,
            > = Vec::new();
            while Instant::now() < deadline && !stopped.load(Ordering::Acquire) {
                for index in (0..connections.len()).rev() {
                    if connections[index].is_finished() {
                        let result = connections
                            .swap_remove(index)
                            .join()
                            .map_err(|_| io::Error::other("Pairing stopped unexpectedly."))??;
                        if let Some(device) = result {
                            return send(sender, Update::Paired(device));
                        }
                    }
                }
                if connections.is_empty() && offer.tries_left() == 0 {
                    return Err(io::Error::other("Too many tries. Show a new code."));
                }
                let (socket, peer) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25));
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                if connections.len() == MAX_PAIR_CONNECTIONS {
                    drop(socket);
                    continue;
                }
                let mut offer = offer.clone();
                connections.push(scope.spawn(move || {
                    pair_connection(
                        socket, peer, identity, &mut offer, secrets, decision, stopped, sender,
                    )
                }));
            }
            if stopped.load(Ordering::Acquire) {
                Ok(())
            } else {
                send(sender, Update::Expired)
            }
        })();
        drop(listener);
        finished.store(true, Ordering::Release);
        window.close();
        result
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the parts of one pairing connection"
)]
fn pair_connection(
    socket: TcpStream,
    peer: SocketAddr,
    identity: &Identity,
    offer: &mut PairOffer,
    secrets: &hushpen_store::secrets::Secrets,
    pairing: &Pairing,
    stopped: &AtomicBool,
    sender: &SyncSender<Update>,
) -> io::Result<Option<RemoteDevice>> {
    let window = offer.window();
    let tries_before = offer.tries_left();
    let mut compared = None;
    let mut comparison = None;
    let result = RemoteStream::accept_pairing_peer_with(
        socket,
        identity,
        &[],
        Some(offer),
        PAIR_TIMEOUT,
        |number, name, key| {
            let selected = pairing.selected.lock().unwrap_or_else(|e| e.into_inner());
            if selected.as_ref().is_some_and(|id| *id != key_id(key)) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "A different device connected.",
                ));
            }
            let Ok(guard) = pairing.comparing.try_lock() else {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "A number is already being compared.",
                ));
            };
            drop(selected);
            comparison = Some(guard);
            compared = Some(number.to_owned());
            let (decide, decided) = mpsc::channel();
            *pairing.decision.lock().unwrap_or_else(|e| e.into_inner()) =
                Some((number.to_owned(), decide));
            send(
                sender,
                Update::Confirming {
                    number: number.to_owned(),
                    name: name.to_owned(),
                },
            )?;
            let deadline = Instant::now() + PAIR_TIMEOUT;
            loop {
                match decided.recv_timeout(Duration::from_millis(50)) {
                    Ok(accept) => return Ok(accept),
                    Err(mpsc::RecvTimeoutError::Timeout)
                        if Instant::now() < deadline
                            && window.is_open()
                            && !stopped.load(Ordering::Acquire) => {}
                    Err(_) => return Ok(false),
                }
            }
        },
        |key| {
            let mut pins = secrets.paired_clients();
            if !pins.contains(&key) {
                pins.push(key);
            }
            secrets.set_paired_clients(&pins).map_err(io::Error::other)
        },
    );
    if comparison.is_some() {
        *pairing.decision.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    let mut session = match result {
        Ok(session) => session,
        Err(_) => {
            let tries = offer.tries_left();
            if !stopped.load(Ordering::Acquire) && (compared.is_some() || tries < tries_before) {
                let message = if compared.is_some() {
                    "Pairing was cancelled or the numbers did not match."
                } else {
                    "Pairing did not finish. Try again."
                }
                .to_owned();
                send(
                    sender,
                    Update::Attempt {
                        message,
                        tries,
                        number: compared,
                    },
                )?;
            }
            return Ok(None);
        }
    };
    let control = session.stream.control();
    let watchdog_stop = Arc::new(AtomicBool::new(false));
    let watch = Arc::clone(&watchdog_stop);
    let watchdog = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !watch.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(25));
        }
        if !watch.load(Ordering::Acquire) {
            let _ = control.shutdown();
        }
    });
    let enrollment =
        hushpen_remote::enrollment::Enrollment::receive(&mut session.stream, &session.peer_key);
    watchdog_stop.store(true, Ordering::Release);
    let _ = watchdog.join();
    let enrollment = enrollment?;
    let address = enrollment.address(peer.ip());
    Ok(Some(paired(
        enrollment.device,
        address,
        session.peer_key,
        Transport::Lan,
    )))
}

fn computer_name() -> io::Result<String> {
    #[cfg(target_os = "macos")]
    let name = {
        let output = std::process::Command::new("/usr/sbin/scutil")
            .args(["--get", "ComputerName"])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("The computer name is unavailable."));
        }
        String::from_utf8(output.stdout).map_err(io::Error::other)?
    };
    #[cfg(target_os = "windows")]
    let name = std::env::var("COMPUTERNAME").map_err(io::Error::other)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let name = std::fs::read_to_string("/etc/hostname")?;
    Ok(name.trim().to_owned())
}

fn send(sender: &SyncSender<Update>, update: Update) -> io::Result<()> {
    sender
        .send(update)
        .map_err(|_| io::Error::other("device setup closed"))
}

fn key_id(key: &[u8; 32]) -> String {
    key.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_remote::enrollment::Enrollment;

    struct Listening {
        _directory: tempfile::TempDir,
        secrets: Arc<hushpen_store::secrets::Secrets>,
        pairing: Arc<Pairing>,
        stopped: Arc<AtomicBool>,
        updates: mpsc::Receiver<Update>,
        worker: Option<std::thread::JoinHandle<io::Result<()>>>,
        qr: hushpen_remote::QrPayload,
    }

    impl Listening {
        fn start() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let secrets =
                Arc::new(hushpen_store::secrets::Secrets::open(directory.path()).unwrap());
            let pairing = Arc::new(Pairing::default());
            let stopped = Arc::new(AtomicBool::new(false));
            let (sender, updates) = mpsc::sync_channel(32);
            let (pins, decisions, stop) = (
                Arc::clone(&secrets),
                Arc::clone(&pairing),
                Arc::clone(&stopped),
            );
            let worker = std::thread::spawn(move || {
                offer(
                    &Identity::generate().unwrap(),
                    "127.0.0.1".into(),
                    &pins,
                    &decisions,
                    &stop,
                    &sender,
                )
            });
            let Update::Offer { payload, .. } =
                updates.recv_timeout(Duration::from_secs(5)).unwrap()
            else {
                panic!("offer expected")
            };
            Self {
                _directory: directory,
                secrets,
                pairing,
                stopped,
                updates,
                worker: Some(worker),
                qr: hushpen_remote::QrPayload::parse(&payload).unwrap(),
            }
        }

        fn address(&self) -> String {
            format!("127.0.0.1:{}", self.qr.port)
        }

        fn next(&self) -> Update {
            self.updates.recv_timeout(Duration::from_secs(5)).unwrap()
        }

        fn nearby(
            &self,
            phone: Identity,
            approve: bool,
        ) -> std::thread::JoinHandle<io::Result<()>> {
            let address = self.address();
            std::thread::spawn(move || {
                let mut session = RemoteStream::connect_nearby_named(
                    address,
                    &phone,
                    "Actual phone name",
                    Duration::from_secs(3),
                    &hushpen_remote::Resolver::default(),
                    |_, _| Ok(approve),
                )?;
                enroll(&mut session.stream)
            })
        }
    }

    impl Drop for Listening {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::Release);
            if let Some(worker) = self.worker.take() {
                worker.join().unwrap().unwrap();
            }
        }
    }

    fn enroll(stream: &mut RemoteStream) -> io::Result<()> {
        Enrollment {
            device: Device {
                id: "ignored".into(),
                name: "Actual phone name".into(),
                kind: DeviceKind::Phone,
                os: Os::Ios,
                serves: Serves::default(),
            },
            port: 47321,
        }
        .send(stream)
    }

    #[test]
    fn wrong_nearby_key_is_rejected_and_the_selected_peer_can_still_pair() {
        let listening = Listening::start();
        let phone = Identity::generate().unwrap();
        let key = phone.public();
        *listening.pairing.selected.lock().unwrap() = Some(key_id(&key));
        assert!(
            listening
                .nearby(Identity::generate().unwrap(), true)
                .join()
                .unwrap()
                .is_err()
        );
        assert!(listening.secrets.paired_clients().is_empty());
        assert!(matches!(
            listening.updates.try_recv(),
            Err(TryRecvError::Empty)
        ));
        let honest = listening.nearby(phone, true);
        let Update::Confirming { number, name } = listening.next() else {
            panic!("confirmation expected")
        };
        assert_eq!(name, "Actual phone name");
        listening.pairing.answer(&number, true);
        honest.join().unwrap().unwrap();
        let Update::Paired(device) = listening.next() else {
            panic!("paired device expected")
        };
        assert_eq!(device.static_public_key, key);
        assert_eq!(listening.secrets.paired_clients(), [key]);
    }

    #[test]
    fn stalled_preauth_socket_does_not_block_an_honest_qr_peer() {
        let listening = Listening::start();
        let _stalled = TcpStream::connect(listening.address()).unwrap();
        let started = Instant::now();
        let mut session = RemoteStream::connect_qr(
            listening.address(),
            &Identity::generate().unwrap(),
            &listening.qr,
            Duration::from_secs(3),
            &hushpen_remote::Resolver::default(),
        )
        .unwrap();
        enroll(&mut session.stream).unwrap();
        assert!(matches!(listening.next(), Update::Paired(_)));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn nearby_cancellation_leaves_the_code_tries_unchanged() {
        let listening = Listening::start();
        for _ in 0..4 {
            let peer = listening.nearby(Identity::generate().unwrap(), true);
            let Update::Confirming { number, .. } = listening.next() else {
                panic!("confirmation expected")
            };
            listening.pairing.answer(&number, false);
            assert!(peer.join().unwrap().is_err());
            assert!(matches!(listening.next(), Update::Attempt { tries: 3, .. }));
        }
        let mut session = RemoteStream::connect_qr(
            listening.address(),
            &Identity::generate().unwrap(),
            &listening.qr,
            Duration::from_secs(3),
            &hushpen_remote::Resolver::default(),
        )
        .unwrap();
        enroll(&mut session.stream).unwrap();
        assert!(matches!(listening.next(), Update::Paired(_)));
    }

    #[test]
    fn stale_number_cannot_approve_a_later_connection() {
        let pairing = Pairing::default();
        let (sender, receiver) = mpsc::channel();
        *pairing.decision.lock().unwrap() = Some(("123456".into(), sender));
        pairing.answer("654321", true);
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
        pairing.answer("123456", true);
        assert!(receiver.recv().unwrap());
    }

    #[test]
    fn ssh_pairing_uses_the_configured_hostname_or_the_original_offer() {
        for (offered_host, hostname) in [
            ("10.0.0.48", Ok("127.0.0.1".to_owned())),
            ("127.0.0.1", Ok("hushpen-no-such-server.invalid".to_owned())),
            (
                "127.0.0.1",
                Err(io::Error::other("SSH config lookup failed.")),
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let server = Identity::generate().unwrap();
            let client = Identity::generate().unwrap();
            let key = server.public();
            let client_key = client.public();
            let expires = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 60;
            let mut offer =
                PairOffer::new(offered_host.into(), address.port(), key, expires).unwrap();
            offer.open_window("Test SSH host").unwrap();
            let uri = offer.to_bootstrap();
            let worker = std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(8);
                let socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "SSH pairing never reached the host"
                            );
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("SSH pairing listener failed: {error}"),
                    }
                };
                let session = RemoteStream::accept(
                    socket,
                    &server,
                    &[],
                    Some(&mut offer),
                    Duration::from_secs(3),
                )
                .unwrap();
                assert_eq!(session.peer_key, client_key);
                assert!(session.paired);
            });
            let (paired_address, paired_key) = pair_ssh(
                &client,
                &uri,
                hostname,
                Err(io::Error::other("Tailscale is unavailable.")),
                &hushpen_remote::Resolver::default(),
            )
            .unwrap();
            assert_eq!(paired_address, address.to_string());
            assert_eq!(paired_key, key);
            worker.join().unwrap();
        }
    }

    #[test]
    fn cancelled_ssh_address_lookup_does_not_pair_the_original_offer() {
        let client = Identity::generate().unwrap();
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 60;
        let offer = PairOffer::new("127.0.0.1".into(), 47321, client.public(), expires).unwrap();
        for cancel_hostname in [true, false] {
            let cancelled = || Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"));
            let (hostname, tailscale) = if cancel_hostname {
                (cancelled(), Ok(String::new()))
            } else {
                (Ok(String::new()), cancelled())
            };
            let problem = pair_ssh(
                &client,
                &offer.to_bootstrap(),
                hostname,
                tailscale,
                &hushpen_remote::Resolver::default(),
            )
            .unwrap_err();
            assert_eq!(problem.kind(), io::ErrorKind::Interrupted);
        }
    }

    #[test]
    fn qr_pairing_pins_the_phone_before_returning_its_serving_address() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = Arc::new(hushpen_store::secrets::Secrets::open(directory.path()).unwrap());
        let worker_secrets = Arc::clone(&secrets);
        let desktop = Identity::generate().unwrap();
        let phone = Identity::generate().unwrap();
        let phone_key = phone.public();
        let (sender, receiver) = mpsc::sync_channel(4);
        let worker = std::thread::spawn(move || {
            offer(
                &desktop,
                "127.0.0.1".into(),
                &worker_secrets,
                &Pairing::default(),
                &AtomicBool::new(false),
                &sender,
            )
            .unwrap();
        });
        let Update::Offer {
            payload,
            qr,
            expires,
            code,
        } = receiver.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("pairing offer expected");
        };
        assert!(!qr.is_empty());
        assert!(qr.iter().all(|row| row.len() == qr.len()));
        assert!(code.len() == 8 && code.bytes().all(|digit| digit.is_ascii_digit()));
        let parsed = hushpen_remote::QrPayload::parse(&payload).unwrap();
        secrets.set_endpoint_api_key(Some("test-token")).unwrap();
        assert_eq!(parsed.expires_unix_secs, expires);
        let mut session = RemoteStream::connect_qr(
            format!("127.0.0.1:{}", parsed.port),
            &phone,
            &parsed,
            Duration::from_secs(3),
            &hushpen_remote::Resolver::default(),
        )
        .unwrap();
        let pins = hushpen_store::secrets::Secrets::open(directory.path())
            .unwrap()
            .paired_clients();
        assert_eq!(pins, vec![phone_key]);
        assert_eq!(secrets.endpoint_api_key().as_deref(), Some("test-token"));
        secrets
            .set_endpoint_api_key(Some("updated-test-token"))
            .unwrap();
        assert_eq!(
            hushpen_store::secrets::Secrets::open(directory.path())
                .unwrap()
                .paired_clients(),
            pins
        );
        Enrollment {
            device: Device {
                id: "ignored".into(),
                name: "Test phone".into(),
                kind: DeviceKind::Phone,
                os: Os::Ios,
                serves: Serves {
                    speech: true,
                    cleanup: true,
                },
            },
            port: 47321,
        }
        .send(&mut session.stream)
        .unwrap();
        let Update::Paired(device) = receiver.recv_timeout(Duration::from_secs(5)).unwrap() else {
            panic!("paired device expected");
        };
        assert_eq!(device.static_public_key, phone_key);
        assert_eq!(device.device.id, key_id(&phone_key));
        assert_eq!(device.address, "127.0.0.1:47321");
        assert_eq!(device.unload_rule, UnloadRule::WhileServerModeOpen);
        worker.join().unwrap();
    }

    #[test]
    fn cancelling_a_qr_offer_closes_its_listener() {
        let directory = tempfile::tempdir().unwrap();
        let secrets = hushpen_store::secrets::Secrets::open(directory.path()).unwrap();
        let identity = Identity::generate().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancelled);
        let (sender, receiver) = mpsc::sync_channel(4);
        let worker = std::thread::spawn(move || {
            offer(
                &identity,
                String::new(),
                &secrets,
                &Pairing::default(),
                &stop,
                &sender,
            )
        });
        let Update::Offer { payload, .. } = receiver.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("offer expected")
        };
        let parsed = hushpen_remote::QrPayload::parse(&payload).unwrap();
        assert!(parsed.host.split('.').all(|label| label.len() <= 63));
        let port = parsed.port;
        let _stalled = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        let started = Instant::now();
        cancelled.store(true, Ordering::Release);
        worker.join().unwrap().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
    }
}
