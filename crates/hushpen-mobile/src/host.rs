mod cleanup;

use crate::{Engine, MobileError, SystemCleanup, lock};
use hushpen_core::os::Os;
use hushpen_core::remote::{Device, DeviceKind, Serves, UnloadRule};
use hushpen_engine::{EngineLog, InProcess, Link, LlmClient, Timing};
use hushpen_host::Workers;
use hushpen_remote::{Identity, PairOffer, RemoteControl, RemoteStream, Resolver};
use hushpen_store::secrets::Secrets;
use std::collections::HashMap;
use std::net::TcpListener;
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
        Ok(Arc::new(Self {
            engine,
            system,
            name,
            os: Os::current(),
            identity: Arc::new(identity),
            secrets: Arc::new(secrets),
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
        let id = key_id(&self.identity.public());
        let device = Device {
            id: id.clone(),
            name: self.name.clone(),
            kind: DeviceKind::Phone,
            os,
            serves: Serves {
                speech: true,
                cleanup: self.system.available(),
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
        let workers = Workers::from_clients(self.engine.client.clone(), cleanup, os, true);
        workers.set_job_activity(activity_callback(lock(&self.activity).clone()));
        workers.set_unload_rule(rule);
        let stop = Arc::new(AtomicBool::new(false));
        let connections = Arc::new(Mutex::new(HashMap::new()));
        let thread = accept(
            listener,
            Arc::clone(&self.identity),
            Arc::clone(&self.secrets),
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
            rule,
            _advertisement: advertisement,
        });
        Ok(port)
    }

    pub fn pair(&self, uri: String) -> Result<(), MobileError> {
        let port = lock(&self.running)
            .as_ref()
            .map(|running| running.port)
            .ok_or_else(|| MobileError::failed("start Server mode before pairing"))?;
        let offer = PairOffer::parse(&uri).map_err(MobileError::invalid)?;
        let mut session = RemoteStream::connect(
            match offer.host.parse::<std::net::IpAddr>() {
                Ok(ip) => std::net::SocketAddr::new(ip, offer.port).to_string(),
                Err(_) => format!("{}:{}", offer.host, offer.port),
            },
            &self.identity,
            offer.public_key,
            Some(&offer),
            Duration::from_secs(10),
            &Resolver::default(),
        )
        .map_err(MobileError::failed)?;
        let mut pins = self.secrets.paired_clients();
        if !pins.contains(&session.peer_key) {
            pins.push(session.peer_key);
        }
        self.secrets
            .set_paired_clients(&pins)
            .map_err(MobileError::failed)?;
        hushpen_remote::enrollment::Enrollment {
            device: Device {
                id: key_id(&self.identity.public()),
                name: self.name.clone(),
                kind: DeviceKind::Phone,
                os: self.os,
                serves: Serves {
                    speech: true,
                    cleanup: self.system.available(),
                },
            },
            port,
        }
        .send(&mut session.stream)
        .map_err(MobileError::failed)
    }

    pub fn paired_clients(&self) -> Vec<String> {
        self.secrets.paired_clients().iter().map(key_id).collect()
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
            .map_err(MobileError::failed)
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

    pub fn stop(&self) {
        let Some(running) = lock(&self.running).take() else {
            return;
        };
        running.stop.store(true, Ordering::Release);
        for control in lock(&running.connections).values() {
            let _ = control.shutdown();
        }
        let _ = running.thread.join();
        running.workers.set_job_activity(None);
        running.workers.cleanup.shutdown();
        if running.rule == UnloadRule::WhileServerModeOpen {
            self.engine.client.unload();
        }
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
    identity: Arc<Identity>,
    secrets: Arc<Secrets>,
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
                let socket = match listener.accept() {
                    Ok((socket, _)) => socket,
                    Err(problem) if problem.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20));
                        continue;
                    }
                    Err(_) => break,
                };
                if active.load(Ordering::Acquire) >= 8 {
                    continue;
                }
                let session = match RemoteStream::accept(
                    socket,
                    &identity,
                    &secrets.paired_clients(),
                    None,
                    Duration::from_secs(2),
                ) {
                    Ok(session) => session,
                    Err(_) => continue,
                };
                let control = session.stream.control();
                let mut controls = lock(&connections);
                let id = next_id;
                next_id = next_id.wrapping_add(1);
                controls.insert(id, control);
                if stop.load(Ordering::Acquire) {
                    for control in controls.values() {
                        let _ = control.shutdown();
                    }
                    break;
                }
                drop(controls);
                let workers = Arc::clone(&workers);
                let data = data.clone();
                let active = Arc::clone(&active);
                let connections = Arc::clone(&connections);
                active.fetch_add(1, Ordering::AcqRel);
                std::thread::spawn(move || {
                    if let Err(problem) =
                        hushpen_host::serve_session(session.stream, workers, &data, rule)
                    {
                        log::debug!("PHONE_SESSION_ENDED {problem}");
                    }
                    lock(&connections).remove(&id);
                    active.fetch_sub(1, Ordering::AcqRel);
                });
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MobileError, SystemLlm};
    use hushpen_engine::{LlmOutcome, LlmSpec, LlmState, LlmWork, RemoteSpec};
    use hushpen_remote::WorkerKind;

    struct System;
    impl SystemLlm for System {
        fn available(&self) -> bool {
            true
        }
        fn cleanup(&self, text: String, _: String) -> Result<String, MobileError> {
            Ok(format!("{}!", text.trim_end_matches('.')))
        }
    }

    fn host(data: &std::path::Path, os: Os) -> Arc<Host> {
        let engine = Engine::new(data.to_string_lossy().into_owned()).unwrap();
        let mut host = Host::new(
            engine,
            "Test phone".into(),
            SystemCleanup::new(Arc::new(System)),
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
        let offer = PairOffer::new(
            "127.0.0.1".into(),
            listener.local_addr().unwrap().port(),
            desktop.public(),
            hushpen_store::time::now_unix_ms() / 1000 + 120,
        )
        .unwrap();
        let uri = offer.to_uri();
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
            enrollment.port
        });
        host.pair(uri).unwrap();
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
}
