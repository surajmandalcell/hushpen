//! Device setup runs outside the UI thread and returns authenticated settings.

use crate::devices::{Devices, Request};
use crate::{device_ssh, remote_jobs, storage::Storage};
use gpui_kit::{App, Entity};
use hushpen_core::os::Os;
use hushpen_core::remote::{Device, DeviceKind, Health, Serves, UnloadRule};
use hushpen_remote::discovery::{Advertisement, Discovery, FoundDevice};
use hushpen_remote::{Identity, PairOffer, RemoteStream};
use hushpen_store::settings::{RemoteDevice, Transport};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

enum Update {
    Found(Vec<FoundDevice>),
    Offer {
        uri: String,
        qr: Vec<Vec<bool>>,
        expires: u64,
    },
    Progress(String),
    Paired(RemoteDevice),
    Health(BTreeMap<String, Health>),
    Failed(String),
    Done,
}

pub fn attach(devices: &Entity<Devices>, storage: Rc<Storage>, cx: &mut App) {
    let config = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .and_then(|home| std::fs::read_to_string(home.join(".ssh/config")).ok())
        .unwrap_or_default();
    devices.update(cx, |view, cx| {
        view.set_hosts(device_ssh::hosts(&config), cx)
    });
    let cancel = Rc::new(RefCell::new(Arc::new(AtomicBool::new(false))));
    cx.subscribe(devices, move |devices, request: &Request, cx| {
        if matches!(request, Request::CancelOffer) {
            cancel.borrow().store(true, Ordering::Release);
            return;
        }
        cancel.borrow().store(true, Ordering::Release);
        let stopped = Arc::new(AtomicBool::new(false));
        *cancel.borrow_mut() = Arc::clone(&stopped);
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
                &stopped,
                &sender,
            ) && !stopped.load(Ordering::Acquire)
            {
                let _ = sender.send(Update::Failed(error.to_string()));
            }
            let _ = sender.send(Update::Done);
        });
        let devices = devices.downgrade();
        let generation = Rc::clone(&cancel);
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
                        Update::Offer { uri, qr, expires } => view.set_offer(uri, qr, expires, cx),
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

fn run(
    request: Request,
    identity: Identity,
    devices: Vec<RemoteDevice>,
    router: Arc<remote_jobs::Router>,
    secrets: &hushpen_store::secrets::Secrets,
    stopped: &AtomicBool,
    sender: &SyncSender<Update>,
) -> io::Result<()> {
    match request {
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
        Request::Offer { host } => offer(&identity, host, secrets, stopped, sender),
        Request::Pair {
            uri,
            device,
            transport,
        } => {
            let (address, static_public_key) =
                remote_jobs::authenticate_pair(&identity, &uri, &router.resolver)?;
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
            let (address, key) = remote_jobs::authenticate_pair(&identity, &uri, &router.resolver)?;
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
        Request::CancelOffer => Ok(()),
    }
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
    stopped: &AtomicBool,
    sender: &SyncSender<Update>,
) -> io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:0")?;
    listener.set_nonblocking(true)?;
    let id = key_id(&identity.public());
    let hostname = format!("hushpen-{id}.local.");
    let host = if host.is_empty() {
        hostname.trim_end_matches('.').to_owned()
    } else {
        host
    };
    let device = Device {
        id,
        name: "Hushpen".into(),
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
    let uri = offer.to_uri();
    let qr = qrcode::QrCode::new(uri.as_bytes()).map_err(io::Error::other)?;
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
            uri,
            qr: bits,
            expires,
        },
    )?;
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline && !stopped.load(Ordering::Acquire) {
        let (socket, peer) = match listener.accept() {
            Ok(pair) => pair,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            Err(error) => return Err(error),
        };
        let mut session = match RemoteStream::accept_with(
            socket,
            identity,
            &[],
            Some(&mut offer),
            Duration::from_secs(3),
            |key| {
                let mut pins = secrets.paired_clients();
                if !pins.contains(&key) {
                    pins.push(key);
                }
                secrets.set_paired_clients(&pins).map_err(io::Error::other)
            },
        ) {
            Ok(session) => session,
            Err(_) => continue,
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
        return send(
            sender,
            Update::Paired(paired(
                enrollment.device,
                address,
                session.peer_key,
                Transport::Lan,
            )),
        );
    }
    if stopped.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "The pairing code expired. Try again.",
        ))
    }
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
                &AtomicBool::new(false),
                &sender,
            )
            .unwrap();
        });
        let Update::Offer { uri, qr, expires } =
            receiver.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("pairing offer expected");
        };
        assert!(!qr.is_empty());
        assert!(qr.iter().all(|row| row.len() == qr.len()));
        let parsed = PairOffer::parse(&uri).unwrap();
        secrets.set_endpoint_api_key(Some("test-token")).unwrap();
        assert_eq!(parsed.expires_unix_secs, expires);
        let mut session = RemoteStream::connect(
            format!("127.0.0.1:{}", parsed.port),
            &phone,
            parsed.public_key,
            Some(&parsed),
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
            offer(&identity, "127.0.0.1".into(), &secrets, &stop, &sender)
        });
        let Update::Offer { uri, .. } = receiver.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("offer expected")
        };
        let port = PairOffer::parse(&uri).unwrap().port;
        cancelled.store(true, Ordering::Release);
        worker.join().unwrap().unwrap();
        assert!(std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).is_err());
    }
}
