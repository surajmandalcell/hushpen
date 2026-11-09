use hushpen_core::os::Os;
use hushpen_core::remote::{Device, DeviceKind, Serves, UnloadRule};
use hushpen_host::Workers;
use hushpen_remote::{Identity, PairOffer, RemoteStream};
use hushpen_store::secrets::Secrets;
use serde_json::json;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct Options {
    data: PathBuf,
    listen: SocketAddr,
    host: String,
    name: String,
    asr: PathBuf,
    llm: PathBuf,
    rule: UnloadRule,
    pair: bool,
    json: bool,
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("hushpen-host: {problem}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("hushpen-host {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "hushpen-host [pair [--json]] --data-dir ROOT [--listen IP:PORT] [--host NAME] [--name NAME] [--asr PATH] [--llm PATH] [--unload keep|idle:MINUTES|memory|open]"
        );
        return Ok(());
    }
    let options = options(&args)?;
    std::fs::create_dir_all(options.data.join("config"))?;
    let initialization = state_lock(&options)?;
    let identity = identity(&options)?;
    if options.pair {
        return pair(&options, &identity);
    }
    drop(initialization);
    hushpen_store::log_file::install(
        &options.data.join("logs/host.log"),
        std::env::var("RUST_LOG").ok().as_deref(),
    )
    .map_err(io::Error::other)?;
    let listener = TcpListener::bind(options.listen)?;
    let workers = Workers::start(
        options.asr.clone(),
        options.llm.clone(),
        &options.data.join("logs"),
    )?;
    #[cfg(target_os = "linux")]
    let _memory = hushpen_host::watch_memory(
        Arc::clone(&workers),
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    let _advertisement = advertise(&options, &identity, listener.local_addr()?.port())?;
    eprintln!("hushpen-host listening on {}", listener.local_addr()?);
    let mut offer: Option<PairOffer> = None;
    let mut generation = None;
    let sessions = Arc::new(AtomicUsize::new(0));
    for socket in listener.incoming() {
        let socket = socket?;
        if sessions.load(Ordering::Acquire) >= 32 {
            continue;
        }
        let state_lock = state_lock(&options)?;
        let secrets = Secrets::open(&options.data.join("config")).map_err(io::Error::other)?;
        if let Some(uri) = secrets.pair_offer()
            && generation.as_ref() != Some(&uri)
        {
            offer = PairOffer::parse(&uri).ok();
            generation = Some(uri);
        }
        let pins = secrets.paired_clients();
        // Remove the durable offer before authentication, so a restart cannot replay it.
        if offer.is_some() {
            secrets.set_pair_offer(None).map_err(io::Error::other)?;
        }
        let session = match RemoteStream::accept_with(
            socket,
            &identity,
            &pins,
            offer.as_mut(),
            Duration::from_secs(10),
            |peer_key| {
                let mut paired = pins.clone();
                paired.push(peer_key);
                secrets
                    .set_paired_clients(&paired)
                    .map_err(io::Error::other)
            },
        ) {
            Ok(session) => session,
            Err(_) => {
                log::warn!("REMOTE_AUTH_FAILED connection refused");
                continue;
            }
        };
        if session.paired {
            offer = None;
        }
        drop(state_lock);
        let data = options.data.clone();
        let workers = Arc::clone(&workers);
        let rule = options.rule;
        let sessions = Arc::clone(&sessions);
        sessions.fetch_add(1, Ordering::AcqRel);
        std::thread::spawn(move || {
            if let Err(problem) = hushpen_host::serve_session(session.stream, workers, &data, rule)
            {
                log::warn!("REMOTE_SESSION_FAILED {problem}");
            }
            sessions.fetch_sub(1, Ordering::AcqRel);
        });
    }
    workers.shutdown();
    Ok(())
}

fn state_lock(options: &Options) -> io::Result<std::fs::File> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(options.data.join("config/host.lock"))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(problem) => return Err(io::Error::other(problem)),
        }
    }
}

fn identity(options: &Options) -> io::Result<Identity> {
    let secrets = Secrets::open(&options.data.join("config")).map_err(io::Error::other)?;
    if let Some(private) = secrets.noise_private_key() {
        return Identity::from_private(private);
    }
    let identity = Identity::generate()?;
    secrets
        .set_noise_private_key(&identity.private())
        .map_err(io::Error::other)?;
    Ok(identity)
}

fn pair(options: &Options, identity: &Identity) -> io::Result<()> {
    let expires = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs()
        .saturating_add(120);
    let host = if options.host.is_empty() {
        format!("hushpen-{}.local", key_id(&identity.public()))
    } else {
        options.host.clone()
    };
    let offer = PairOffer::new(host, options.listen.port(), identity.public(), expires)?;
    let uri = offer.to_uri();
    Secrets::open(&options.data.join("config"))
        .map_err(io::Error::other)?
        .set_pair_offer(Some(&uri))
        .map_err(io::Error::other)?;
    if options.json {
        println!(
            "{}",
            json!({"uri":uri,"otp":offer.otp(),"publicKey":identity.public(),"expires":expires})
        );
    } else {
        println!(
            "Pairing code: {}\nExpires in 2 minutes.\n{}",
            offer.otp(),
            uri
        );
        let qr = qrcode::QrCode::new(uri.as_bytes()).map_err(io::Error::other)?;
        println!(
            "{}",
            qr.render::<qrcode::render::unicode::Dense1x2>()
                .quiet_zone(true)
                .build()
        );
    }
    Ok(())
}

fn options(args: &[String]) -> io::Result<Options> {
    let executable = std::env::current_exe()?;
    let folder = executable
        .parent()
        .ok_or_else(|| io::Error::other("missing executable folder"))?;
    let mut options = Options {
        data: std::env::var_os("HUSHPEN_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_default(),
        listen: "0.0.0.0:47321".parse().map_err(io::Error::other)?,
        host: String::new(),
        name: "Hushpen server".into(),
        asr: folder.join(if cfg!(windows) {
            "hushpen-asr.exe"
        } else {
            "hushpen-asr"
        }),
        llm: folder.join(if cfg!(windows) {
            "hushpen-llm.exe"
        } else {
            "hushpen-llm"
        }),
        rule: UnloadRule::KeepLoaded,
        pair: false,
        json: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "pair" => options.pair = true,
            "--json" => options.json = true,
            "--data-dir" | "--listen" | "--host" | "--name" | "--asr" | "--llm" | "--unload" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "missing option value")
                })?;
                match arg.as_str() {
                    "--data-dir" => options.data = value.into(),
                    "--listen" => options.listen = value.parse().map_err(io::Error::other)?,
                    "--host" => options.host = value.clone(),
                    "--name" => options.name = value.clone(),
                    "--asr" => options.asr = value.into(),
                    "--llm" => options.llm = value.into(),
                    "--unload" => options.rule = parse_rule(value)?,
                    _ => {}
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unknown option",
                ));
            }
        }
    }
    if options.data.as_os_str().is_empty() || !options.data.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set an absolute --data-dir or HUSHPEN_DATA_DIR",
        ));
    }
    if !options.rule.allowed_on(Os::current()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unload rule is unavailable on this system",
        ));
    }
    Ok(options)
}

fn parse_rule(value: &str) -> io::Result<UnloadRule> {
    match value {
        "keep" => Ok(UnloadRule::KeepLoaded),
        "memory" => Ok(UnloadRule::WhenMemoryLow),
        "open" => Ok(UnloadRule::WhileServerModeOpen),
        value if value.starts_with("idle:") => value[5..]
            .parse::<u32>()
            .ok()
            .filter(|minutes| *minutes > 0 && *minutes <= 1440)
            .map(UnloadRule::AfterIdle)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "idle minutes must be 1 to 1440",
                )
            }),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown unload rule",
        )),
    }
}

fn key_id(key: &[u8; 32]) -> String {
    key[..16].iter().map(|byte| format!("{byte:02x}")).collect()
}

fn advertise(
    options: &Options,
    identity: &Identity,
    port: u16,
) -> io::Result<hushpen_remote::discovery::Advertisement> {
    let device = Device {
        id: key_id(&identity.public()),
        name: options.name.clone(),
        kind: DeviceKind::Server,
        os: Os::current(),
        serves: Serves {
            speech: true,
            cleanup: true,
        },
    };
    let hostname = format!("hushpen-{}.local.", device.id);
    hushpen_remote::discovery::Advertisement::new(
        &device,
        &hostname,
        port,
        env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_options_require_an_explicit_absolute_data_folder() {
        assert!(options(&["--data-dir".into(), "relative".into()]).is_err());
        assert!(parse_rule("idle:0").is_err());
        assert!(parse_rule("idle:5").is_ok());
    }
}
