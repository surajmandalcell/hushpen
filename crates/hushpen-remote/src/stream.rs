use crate::{PairOffer, QrPayload, denied, invalid, pairing};
use snow::params::DHChoice;
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::{Builder, HandshakeState, StatelessTransportState};
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const MAX_PACKET: usize = u16::MAX as usize;
const MAX_PLAIN: usize = MAX_PACKET - 16;
const MAX_HANDSHAKE: usize = 4096;
const ACCEPTED: &[u8] = b"hushpen-ready-v1";
const RUNTIME_IDLE: Duration = Duration::from_secs(15);
const IO_POLL: Duration = Duration::from_millis(50);
const PREAUTH_TIMEOUT: Duration = Duration::from_secs(5);
const COMPARISON_TIMEOUT: Duration = Duration::from_secs(60);

/// Shared by a service so an unfinished OS lookup cannot collect more lookup threads.
#[derive(Clone, Default)]
pub struct Resolver {
    busy: Arc<AtomicBool>,
}

struct Lookup(Arc<AtomicBool>);

impl Drop for Lookup {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Resolver {
    fn resolve(
        &self,
        address: impl ToSocketAddrs + ToString + Send + 'static,
        deadline: Instant,
    ) -> io::Result<Vec<SocketAddr>> {
        let timeout = remaining(deadline)?;
        if let Ok(address) = address.to_string().parse::<SocketAddr>() {
            return Ok(vec![address]);
        }
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "a device lookup is still running",
                )
            })?;
        let lookup = Lookup(Arc::clone(&self.busy));
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("remote-resolve".into())
            .spawn(move || {
                let result = address
                    .to_socket_addrs()
                    .map(|addresses| addresses.take(16).collect());
                drop(lookup);
                let _ = sender.send(result);
            })?;
        match receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "device lookup timed out",
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(io::Error::other("device lookup stopped"))
            }
        }
    }
}

/// A device's long-term key. Save the private bytes only in the private secrets file.
pub struct Identity {
    private: [u8; 32],
    public: [u8; 32],
}

impl Identity {
    pub fn generate() -> io::Result<Self> {
        let key = builder()?.generate_keypair().map_err(io::Error::other)?;
        Self::from_private(
            key.private
                .try_into()
                .map_err(|_| invalid("invalid private key"))?,
        )
    }

    pub fn from_private(private: [u8; 32]) -> io::Result<Self> {
        let mut dh = DefaultResolver
            .resolve_dh(&DHChoice::Curve25519)
            .ok_or_else(|| io::Error::other("key exchange unavailable"))?;
        dh.set(&private);
        let public = dh
            .pubkey()
            .try_into()
            .map_err(|_| invalid("invalid public key"))?;
        Ok(Self { private, public })
    }

    pub fn public(&self) -> [u8; 32] {
        self.public
    }

    pub fn private(&self) -> [u8; 32] {
        self.private
    }
}

pub struct Session {
    pub stream: RemoteStream,
    pub peer_key: [u8; 32],
    /// The peer used the offer and its key must now be saved.
    pub paired: bool,
    pub computer_name: Option<String>,
}

enum ClientPairing<'a> {
    Trusted([u8; 32]),
    Bootstrap(&'a PairOffer),
    Code(&'a str),
    Qr(&'a QrPayload),
    Nearby(&'a str),
}

/// A bounded encrypted byte stream. Protocol frames are unchanged inside it.
pub struct RemoteStream {
    reader: RemoteReader,
    writer: RemoteWriter,
    control: RemoteControl,
}

/// Closes a connection without waiting for its reader or writer.
#[derive(Clone)]
pub struct RemoteControl {
    socket: Arc<TcpStream>,
    cancelled: Arc<AtomicBool>,
}

impl RemoteControl {
    fn handshake(socket: &TcpStream) -> io::Result<Self> {
        Ok(Self {
            socket: Arc::new(socket.try_clone()?),
            cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(crate) fn same_connection(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.socket, &other.socket)
    }

    pub fn shutdown(&self) -> io::Result<()> {
        self.cancelled.store(true, Ordering::Release);
        self.socket.shutdown(Shutdown::Both)
    }
}

pub struct RemoteReader {
    socket: TcpStream,
    cipher: Arc<StatelessTransportState>,
    nonce: u64,
    pending: Vec<u8>,
    offset: usize,
    failed: bool,
    packet_timeout: Duration,
    deadline: Option<Instant>,
    cancelled: Arc<AtomicBool>,
}

pub struct RemoteWriter {
    socket: TcpStream,
    cipher: Arc<StatelessTransportState>,
    nonce: u64,
    failed: bool,
    cancelled: Arc<AtomicBool>,
}

impl RemoteStream {
    pub fn connect(
        address: impl ToSocketAddrs + ToString + Send + 'static,
        identity: &Identity,
        pinned_server: [u8; 32],
        pairing: Option<&PairOffer>,
        timeout: Duration,
        resolver: &Resolver,
    ) -> io::Result<Session> {
        if pairing.is_some_and(|offer| offer.public_key != pinned_server) {
            return Err(denied());
        }
        let method = pairing
            .map(ClientPairing::Bootstrap)
            .unwrap_or(ClientPairing::Trusted(pinned_server));
        Self::connect_with(address, identity, method, timeout, resolver, |_, _| {
            Ok(false)
        })
    }

    pub fn connect_qr(
        address: impl ToSocketAddrs + ToString + Send + 'static,
        identity: &Identity,
        qr: &QrPayload,
        timeout: Duration,
        resolver: &Resolver,
    ) -> io::Result<Session> {
        Self::connect_with(
            address,
            identity,
            ClientPairing::Qr(qr),
            timeout,
            resolver,
            |_, _| Ok(false),
        )
    }

    pub fn connect_code(
        address: impl ToSocketAddrs + ToString + Send + 'static,
        identity: &Identity,
        code: &str,
        timeout: Duration,
        resolver: &Resolver,
    ) -> io::Result<Session> {
        Self::connect_with(
            address,
            identity,
            ClientPairing::Code(code),
            timeout,
            resolver,
            |_, _| Ok(false),
        )
    }

    /// The callback waits for the local user to compare and approve the number.
    pub fn connect_nearby(
        address: impl ToSocketAddrs + ToString + Send + 'static,
        identity: &Identity,
        timeout: Duration,
        resolver: &Resolver,
        confirm: impl FnOnce(&str, &str) -> io::Result<bool>,
    ) -> io::Result<Session> {
        let key = identity.public();
        let short_key: String = key[..6].iter().map(|byte| format!("{byte:02x}")).collect();
        Self::connect_nearby_named(
            address,
            identity,
            &format!("Device {short_key}"),
            timeout,
            resolver,
            confirm,
        )
    }

    /// Sends the validated local name in the protected handshake before comparing numbers.
    pub fn connect_nearby_named(
        address: impl ToSocketAddrs + ToString + Send + 'static,
        identity: &Identity,
        name: &str,
        timeout: Duration,
        resolver: &Resolver,
        confirm: impl FnOnce(&str, &str) -> io::Result<bool>,
    ) -> io::Result<Session> {
        crate::enrollment::validate_name(name)?;
        Self::connect_with(
            address,
            identity,
            ClientPairing::Nearby(name),
            timeout,
            resolver,
            confirm,
        )
    }

    fn connect_with(
        address: impl ToSocketAddrs + ToString + Send + 'static,
        identity: &Identity,
        method: ClientPairing<'_>,
        timeout: Duration,
        resolver: &Resolver,
        confirm: impl FnOnce(&str, &str) -> io::Result<bool>,
    ) -> io::Result<Session> {
        let nearby_payload;
        let (pin, payload) = match method {
            ClientPairing::Trusted(key) => (Some(key), &[][..]),
            ClientPairing::Bootstrap(offer) => {
                offer.check()?;
                (Some(offer.public_key), pairing::TYPED)
            }
            ClientPairing::Qr(qr) => {
                if pairing::now()? >= qr.expires_unix_secs {
                    return Err(denied());
                }
                (Some(qr.public_key), pairing::QR)
            }
            ClientPairing::Code(_) => (None, pairing::TYPED),
            ClientPairing::Nearby(name) => {
                nearby_payload = [pairing::NEARBY, b"\0", name.as_bytes()].concat();
                (None, nearby_payload.as_slice())
            }
        };
        let mut deadline = Instant::now()
            .checked_add(timeout.min(PREAUTH_TIMEOUT))
            .ok_or_else(|| invalid("invalid connection timeout"))?;
        let mut last_error = invalid("no device address");
        let mut socket = None;
        for address in resolver.resolve(address, deadline)? {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "connection timed out",
                ));
            }
            match TcpStream::connect_timeout(&address, remaining) {
                Ok(connected) => {
                    socket = Some(connected);
                    break;
                }
                Err(error) => last_error = error,
            }
        }
        let mut socket = socket.ok_or(last_error)?;
        configure(&socket, remaining(deadline)?)?;
        let mut noise = builder()?
            .local_private_key(&identity.private)
            .map_err(io::Error::other)?
            .build_initiator()
            .map_err(io::Error::other)?;
        send_handshake(&mut socket, &mut noise, &[], deadline)?;
        let name = receive_handshake(&mut socket, &mut noise, deadline)?;
        let peer_key = peer_key(&noise)?;
        if pin.is_some_and(|key| peer_key != key) {
            return Err(denied());
        }
        let paired = !payload.is_empty();
        let computer_name = if paired {
            let name = String::from_utf8(name).map_err(|_| invalid("invalid computer name"))?;
            crate::enrollment::validate_name(&name)?;
            Some(name)
        } else {
            None
        };
        send_handshake(&mut socket, &mut noise, payload, deadline)?;
        let context = pairing::context(
            &peer_key,
            &identity.public(),
            computer_name.as_deref().unwrap_or_default(),
            noise.get_handshake_hash(),
        );
        let mut stream = Self::from_handshake(socket, noise, timeout, deadline)?;
        let accepted = match method {
            ClientPairing::Trusted(_) => true,
            ClientPairing::Bootstrap(offer) => {
                pairing::pake(&mut stream, &context, offer.code(), false)?;
                true
            }
            ClientPairing::Code(code) => {
                pairing::pake(&mut stream, &context, code, false)?;
                true
            }
            ClientPairing::Qr(qr) => {
                pairing::qr_client(&mut stream, &context, qr)?;
                true
            }
            ClientPairing::Nearby(_) => {
                let number = pairing::nearby(&mut stream, &context, false)?;
                remaining(deadline)?;
                deadline = stream.comparison_deadline(timeout)?;
                confirm(&number, computer_name.as_deref().unwrap_or_default())?
            }
        };
        if paired {
            remaining(deadline)?;
            pairing::confirm(&mut stream, accepted)?;
        }
        let mut answer = [0; ACCEPTED.len()];
        stream.read_exact(&mut answer)?;
        if answer != ACCEPTED {
            return Err(denied());
        }
        stream.enable_runtime(timeout)?;
        Ok(Session {
            stream,
            peer_key,
            paired,
            computer_name,
        })
    }

    pub fn accept(
        socket: TcpStream,
        identity: &Identity,
        pinned_clients: &[[u8; 32]],
        pairing: Option<&mut PairOffer>,
        timeout: Duration,
    ) -> io::Result<Session> {
        Self::accept_with(socket, identity, pinned_clients, pairing, timeout, |_| {
            Ok(())
        })
    }

    /// Saves a new peer's key before the peer receives its pairing receipt.
    pub fn accept_with(
        socket: TcpStream,
        identity: &Identity,
        pinned_clients: &[[u8; 32]],
        pairing: Option<&mut PairOffer>,
        timeout: Duration,
        on_pair: impl FnOnce([u8; 32]) -> io::Result<()>,
    ) -> io::Result<Session> {
        Self::accept_pairing_with(
            socket,
            identity,
            pinned_clients,
            pairing,
            timeout,
            |_, _| Ok(false),
            on_pair,
        )
    }

    /// Nearby trust requires a local approval and an authenticated approval from the peer.
    pub fn accept_pairing_with(
        socket: TcpStream,
        identity: &Identity,
        pinned_clients: &[[u8; 32]],
        pairing: Option<&mut PairOffer>,
        timeout: Duration,
        confirm: impl FnOnce(&str, &str) -> io::Result<bool>,
        on_pair: impl FnOnce([u8; 32]) -> io::Result<()>,
    ) -> io::Result<Session> {
        let name = pairing
            .as_ref()
            .and_then(|offer| offer.window().name().ok())
            .unwrap_or_default();
        Self::accept_pairing_peer_with(
            socket,
            identity,
            pinned_clients,
            pairing,
            timeout,
            |number, _, _| confirm(number, &name),
            on_pair,
        )
    }

    /// Exposes the connecting peer's protected name and static key for local approval.
    pub fn accept_pairing_peer_with(
        mut socket: TcpStream,
        identity: &Identity,
        pinned_clients: &[[u8; 32]],
        mut pairing: Option<&mut PairOffer>,
        timeout: Duration,
        confirm: impl FnOnce(&str, &str, &[u8; 32]) -> io::Result<bool>,
        on_pair: impl FnOnce([u8; 32]) -> io::Result<()>,
    ) -> io::Result<Session> {
        let mut deadline = Instant::now()
            .checked_add(timeout.min(PREAUTH_TIMEOUT))
            .ok_or_else(|| invalid("invalid connection timeout"))?;
        configure(&socket, timeout.min(PREAUTH_TIMEOUT))?;
        let _window_connection = pairing
            .as_ref()
            .map(|offer| offer.window())
            .map(|window| window.track(RemoteControl::handshake(&socket)?))
            .transpose()?;
        let mut noise = builder()?
            .local_private_key(&identity.private)
            .map_err(io::Error::other)?
            .build_responder()
            .map_err(io::Error::other)?;
        if !receive_handshake(&mut socket, &mut noise, deadline)?.is_empty() {
            return Err(invalid("unexpected handshake payload"));
        }
        let name = pairing
            .as_ref()
            .and_then(|offer| offer.window().name().ok())
            .unwrap_or_default();
        send_handshake(&mut socket, &mut noise, name.as_bytes(), deadline)?;
        let payload = receive_handshake(&mut socket, &mut noise, deadline)?;
        let peer_key = peer_key(&noise)?;
        let paired = !payload.is_empty();
        if !paired && !pinned_clients.contains(&peer_key) {
            return Err(denied());
        }
        let context = pairing::context(
            &identity.public(),
            &peer_key,
            &name,
            noise.get_handshake_hash(),
        );
        let mut stream = Self::from_handshake(socket, noise, timeout, deadline)?;
        if paired {
            let offer = pairing.as_mut().ok_or_else(denied)?;
            if offer.public_key != identity.public() {
                return Err(denied());
            }
            let window = offer.window();
            let result = (|| {
                let accepted = match payload.as_slice() {
                    pairing::TYPED => {
                        offer.begin_attempt()?;
                        pairing::pake(&mut stream, &context, offer.code(), true)?;
                        true
                    }
                    pairing::QR => {
                        offer.begin_attempt()?;
                        pairing::qr_server(&mut stream, &context, offer)?;
                        true
                    }
                    payload if payload.starts_with(pairing::NEARBY) => {
                        offer.check()?;
                        let peer_name = payload
                            .strip_prefix(pairing::NEARBY)
                            .and_then(|name| name.strip_prefix(b"\0"))
                            .ok_or_else(denied)?;
                        let peer_name = std::str::from_utf8(peer_name)
                            .map_err(|_| invalid("invalid peer name"))?;
                        crate::enrollment::validate_name(peer_name)?;
                        let number = pairing::nearby(&mut stream, &context, true)?;
                        remaining(deadline)?;
                        deadline = stream.comparison_deadline(timeout)?;
                        confirm(&number, peer_name, &peer_key)?
                    }
                    _ => return Err(denied()),
                };
                remaining(deadline)?;
                pairing::confirm(&mut stream, accepted)?;
                remaining(deadline)?;
                offer.consume()?;
                window.save(|| on_pair(peer_key))
            })();
            result?;
        }
        stream
            .writer
            .socket
            .set_write_timeout(Some(remaining(deadline)?))?;
        stream.write_all(ACCEPTED)?;
        stream.flush()?;
        stream.enable_runtime(timeout)?;
        Ok(Session {
            stream,
            peer_key,
            paired,
            computer_name: paired.then_some(name),
        })
    }

    fn from_handshake(
        socket: TcpStream,
        noise: HandshakeState,
        packet_timeout: Duration,
        deadline: Instant,
    ) -> io::Result<Self> {
        // Windows can leave a blocking send pending after another thread calls shutdown.
        socket.set_nonblocking(cfg!(windows))?;
        let cancelled = Arc::new(AtomicBool::new(false));
        let control = RemoteControl {
            socket: Arc::new(socket.try_clone()?),
            cancelled: Arc::clone(&cancelled),
        };
        let cipher = Arc::new(
            noise
                .into_stateless_transport_mode()
                .map_err(io::Error::other)?,
        );
        let reader = RemoteReader {
            socket: socket.try_clone()?,
            cipher: Arc::clone(&cipher),
            nonce: 0,
            pending: Vec::new(),
            offset: 0,
            failed: false,
            packet_timeout,
            deadline: Some(deadline),
            cancelled: Arc::clone(&cancelled),
        };
        let writer = RemoteWriter {
            socket,
            cipher,
            nonce: 0,
            failed: false,
            cancelled,
        };
        Ok(Self {
            reader,
            writer,
            control,
        })
    }

    fn enable_runtime(&mut self, packet_timeout: Duration) -> io::Result<()> {
        self.reader
            .socket
            .set_read_timeout(Some(RUNTIME_IDLE.max(packet_timeout)))?;
        self.writer.socket.set_write_timeout(Some(packet_timeout))?;
        self.reader.deadline = None;
        Ok(())
    }

    fn comparison_deadline(&mut self, timeout: Duration) -> io::Result<Instant> {
        let timeout = timeout.min(COMPARISON_TIMEOUT);
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| invalid("invalid comparison timeout"))?;
        self.writer.socket.set_write_timeout(Some(timeout))?;
        self.reader.deadline = Some(deadline);
        Ok(deadline)
    }

    /// Only selected worker sessions can wait without an idle deadline.
    /// Partial encrypted packets still use the packet timeout.
    pub fn allow_idle_wait(&mut self) -> io::Result<()> {
        self.reader.socket.set_read_timeout(None)
    }

    pub fn split(self) -> io::Result<(RemoteReader, RemoteWriter)> {
        Ok((self.reader, self.writer))
    }

    pub fn control(&self) -> RemoteControl {
        self.control.clone()
    }

    pub fn shutdown(&self) -> io::Result<()> {
        self.writer.shutdown()
    }
}

impl Read for RemoteStream {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reader.read(output)
    }
}

impl Write for RemoteStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writer.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl RemoteReader {
    /// Bounds an active transfer while the connection otherwise waits for work.
    pub fn set_read_deadline(&mut self, deadline: Option<Instant>) {
        self.deadline = deadline;
    }

    pub fn shutdown(&self) -> io::Result<()> {
        self.cancelled.store(true, Ordering::Release);
        self.socket.shutdown(Shutdown::Both)
    }

    fn read_packet(&mut self) -> io::Result<bool> {
        let Some(packet) = receive_packet(
            &mut self.socket,
            MAX_PACKET,
            self.packet_timeout,
            self.deadline,
            Some(&self.cancelled),
        )?
        else {
            return Ok(false);
        };
        if packet.len() < 16 {
            return Err(invalid("invalid encrypted packet"));
        }
        self.pending.resize(packet.len() - 16, 0);
        let length = self
            .cipher
            .read_message(self.nonce, &packet, &mut self.pending)
            .map_err(|_| invalid("encrypted packet authentication failed"))?;
        self.nonce = self
            .nonce
            .checked_add(1)
            .ok_or_else(|| invalid("connection exhausted"))?;
        if length == 0 {
            return Err(invalid("empty encrypted packet"));
        }
        self.pending.truncate(length);
        self.offset = 0;
        Ok(true)
    }
}

impl Read for RemoteReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        check_cancelled(Some(&self.cancelled))?;
        if self.failed {
            return Err(invalid("connection closed after a read failure"));
        }
        if self.offset == self.pending.len() {
            match self.read_packet() {
                Ok(true) => {}
                Ok(false) => return Ok(0),
                Err(error) => {
                    self.failed = true;
                    let _ = self.shutdown();
                    return Err(error);
                }
            }
        }
        let count = output.len().min(self.pending.len() - self.offset);
        output[..count].copy_from_slice(&self.pending[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

impl RemoteWriter {
    pub fn shutdown(&self) -> io::Result<()> {
        self.cancelled.store(true, Ordering::Release);
        self.socket.shutdown(Shutdown::Both)
    }

    fn write_packet(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(MAX_PLAIN);
        let mut packet = vec![0; count + 16];
        let length = self
            .cipher
            .write_message(self.nonce, &bytes[..count], &mut packet)
            .map_err(io::Error::other)?;
        self.nonce = self
            .nonce
            .checked_add(1)
            .ok_or_else(|| invalid("connection exhausted"))?;
        send_packet(&mut self.socket, &packet[..length], Some(&self.cancelled))?;
        Ok(count)
    }
}

impl Write for RemoteWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        check_cancelled(Some(&self.cancelled))?;
        if self.failed {
            return Err(invalid("connection closed after a write failure"));
        }
        match self.write_packet(bytes) {
            Ok(count) => Ok(count),
            Err(error) => {
                self.failed = true;
                let _ = self.shutdown();
                Err(error)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}

fn builder() -> io::Result<Builder<'static>> {
    let pattern = PATTERN.parse().map_err(io::Error::other)?;
    Builder::new(pattern)
        .prologue(b"hushpen-remote-v1")
        .map_err(io::Error::other)
}

fn configure(socket: &TcpStream, timeout: Duration) -> io::Result<()> {
    socket.set_read_timeout(Some(timeout))?;
    socket.set_write_timeout(Some(timeout))?;
    socket.set_nodelay(true)
}

fn peer_key(noise: &HandshakeState) -> io::Result<[u8; 32]> {
    noise
        .get_remote_static()
        .ok_or_else(denied)?
        .try_into()
        .map_err(|_| denied())
}

fn send_handshake(
    socket: &mut TcpStream,
    noise: &mut HandshakeState,
    payload: &[u8],
    deadline: Instant,
) -> io::Result<()> {
    socket.set_write_timeout(Some(remaining(deadline)?))?;
    let mut packet = [0; MAX_HANDSHAKE];
    let size = noise
        .write_message(payload, &mut packet)
        .map_err(io::Error::other)?;
    send_packet(socket, &packet[..size], None)
}

fn receive_handshake(
    socket: &mut TcpStream,
    noise: &mut HandshakeState,
    deadline: Instant,
) -> io::Result<Vec<u8>> {
    let timeout = remaining(deadline)?;
    socket.set_read_timeout(Some(timeout))?;
    let packet = receive_packet(socket, MAX_HANDSHAKE, timeout, Some(deadline), None)?
        .ok_or_else(|| invalid("handshake ended early"))?;
    let mut plain = vec![0; MAX_HANDSHAKE];
    let size = noise
        .read_message(&packet, &mut plain)
        .map_err(|_| denied())?;
    plain.truncate(size);
    Ok(plain)
}

fn send_packet(
    socket: &mut TcpStream,
    bytes: &[u8],
    cancelled: Option<&AtomicBool>,
) -> io::Result<()> {
    let length = u16::try_from(bytes.len()).map_err(|_| invalid("packet too large"))?;
    let timeout = socket
        .write_timeout()?
        .ok_or_else(|| invalid("missing write timeout"))?;
    let deadline = Instant::now() + timeout;
    for bytes in [&length.to_le_bytes()[..], bytes] {
        let mut left = bytes;
        while !left.is_empty() {
            check_cancelled(cancelled)?;
            socket.set_write_timeout(Some(remaining(deadline)?))?;
            match socket.write(left) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "device stopped reading",
                    ));
                }
                Ok(count) => left = &left[count..],
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    wait_for_io(deadline, cancelled)?
                }
                Err(error) => return Err(error),
            }
        }
    }
    socket.set_write_timeout(Some(timeout))
}

fn receive_packet(
    socket: &mut TcpStream,
    limit: usize,
    packet_timeout: Duration,
    read_deadline: Option<Instant>,
    cancelled: Option<&AtomicBool>,
) -> io::Result<Option<Vec<u8>>> {
    let timeout = socket.read_timeout()?;
    let idle_deadline = read_deadline.or_else(|| timeout.map(|timeout| Instant::now() + timeout));
    let mut header = [0; 2];
    match read_before(socket, &mut header[..1], idle_deadline, cancelled)? {
        0 => return Ok(None),
        1 => {}
        _ => return Err(invalid("invalid packet header")),
    }
    let packet_deadline = Instant::now() + packet_timeout;
    let deadline = read_deadline.map_or(packet_deadline, |deadline| deadline.min(packet_deadline));
    read_exact_before(socket, &mut header[1..], deadline, cancelled)?;
    let size = usize::from(u16::from_le_bytes(header));
    if size == 0 || size > limit {
        return Err(invalid("packet length is outside the limit"));
    }
    let mut packet = vec![0; size];
    read_exact_before(socket, &mut packet, deadline, cancelled)?;
    socket.set_read_timeout(timeout)?;
    Ok(Some(packet))
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "device packet timed out"))
}

fn read_before(
    socket: &mut TcpStream,
    bytes: &mut [u8],
    deadline: Option<Instant>,
    cancelled: Option<&AtomicBool>,
) -> io::Result<usize> {
    loop {
        check_cancelled(cancelled)?;
        socket.set_read_timeout(deadline.map(remaining).transpose()?)?;
        match socket.read(bytes) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if let Some(deadline) = deadline {
                    wait_for_io(deadline, cancelled)?;
                } else {
                    std::thread::sleep(IO_POLL);
                    check_cancelled(cancelled)?;
                }
            }
            result => return result,
        }
    }
}

fn read_exact_before(
    socket: &mut TcpStream,
    mut bytes: &mut [u8],
    deadline: Instant,
    cancelled: Option<&AtomicBool>,
) -> io::Result<()> {
    while !bytes.is_empty() {
        let count = read_before(socket, bytes, Some(deadline), cancelled)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "device packet ended early",
            ));
        }
        bytes = &mut bytes[count..];
    }
    Ok(())
}

fn check_cancelled(cancelled: Option<&AtomicBool>) -> io::Result<()> {
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "device connection cancelled",
        ));
    }
    Ok(())
}

fn wait_for_io(deadline: Instant, cancelled: Option<&AtomicBool>) -> io::Result<()> {
    check_cancelled(cancelled)?;
    std::thread::sleep(remaining(deadline)?.min(IO_POLL));
    check_cancelled(cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorkerKind, read_worker, select_worker};
    use hushpen_core::protocol::{self, FrameKind};
    use std::net::TcpListener;
    use std::thread;

    const TIMEOUT: Duration = Duration::from_secs(2);

    fn connected() -> (RemoteStream, RemoteStream) {
        connected_with_timeout(TIMEOUT)
    }

    fn connected_with_timeout(timeout: Duration) -> (RemoteStream, RemoteStream) {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let server_key = server.public();
        let client_key = client.public();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let thread = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let session =
                RemoteStream::accept(socket, &server, &[client_key], None, timeout).unwrap();
            assert_eq!(session.peer_key, client_key);
            assert!(!session.paired);
            session.stream
        });
        let session = RemoteStream::connect(
            address,
            &client,
            server_key,
            None,
            timeout,
            &Resolver::default(),
        )
        .unwrap();
        assert_eq!(session.peer_key, server_key);
        assert!(!session.paired);
        (session.stream, thread.join().unwrap())
    }

    #[test]
    fn pinned_keys_carry_unchanged_frames_larger_than_one_noise_packet() {
        let (mut client, mut server) = connected();
        let bytes = vec![73; 200_000];
        let expected = bytes.clone();
        let writer = thread::spawn(move || {
            select_worker(&mut client, WorkerKind::Speech).unwrap();
            protocol::write_frame(&mut client, FrameKind::Binary, &bytes).unwrap();
            protocol::read_frame(&mut client).unwrap().unwrap()
        });
        assert_eq!(read_worker(&mut server).unwrap(), WorkerKind::Speech);
        let frame = protocol::read_frame(&mut server).unwrap().unwrap();
        assert_eq!(frame.kind, FrameKind::Binary);
        assert_eq!(frame.payload, expected);
        protocol::write_frame(&mut server, FrameKind::Json, br#"{"type":"pong"}"#).unwrap();
        assert_eq!(writer.join().unwrap().payload, br#"{"type":"pong"}"#);
    }

    #[test]
    fn split_halves_allow_cancel_while_waiting_for_a_reply() {
        let (client, mut server) = connected();
        let (mut read, mut write) = client.split().unwrap();
        let reading = thread::spawn(move || {
            let mut answer = [0; 4];
            read.read_exact(&mut answer).unwrap();
            answer
        });
        write.write_all(b"cancel").unwrap();
        let mut command = [0; 6];
        server.read_exact(&mut command).unwrap();
        assert_eq!(&command, b"cancel");
        server.write_all(b"done").unwrap();
        assert_eq!(&reading.join().unwrap(), b"done");
    }

    #[test]
    fn pairing_proves_the_code_and_consumes_the_offer() {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let server_key = server.public();
        let client_key = client.public();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server_key,
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Test computer").unwrap();
        let scanned = PairOffer::from_bootstrap(&offer.to_bootstrap()).unwrap();
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let first =
                RemoteStream::accept(socket, &server, &[], Some(&mut offer), TIMEOUT).unwrap();
            assert!(first.paired);
            assert_eq!(first.peer_key, client_key);
            let (socket, _) = listener.accept().unwrap();
            assert!(RemoteStream::accept(socket, &server, &[], Some(&mut offer), TIMEOUT).is_err());
        });
        let first = RemoteStream::connect(
            address,
            &client,
            server_key,
            Some(&scanned),
            TIMEOUT,
            &Resolver::default(),
        )
        .unwrap();
        assert!(first.paired);
        assert!(
            RemoteStream::connect(
                address,
                &client,
                server_key,
                Some(&scanned),
                TIMEOUT,
                &Resolver::default()
            )
            .is_err()
        );
        host.join().unwrap();
    }

    #[test]
    fn qr_pairing_pins_the_key_and_cannot_reuse_an_offer() {
        for wrong_pin in [false, true] {
            let server = Identity::generate().unwrap();
            let client = Identity::generate().unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut offer = PairOffer::new(
                "127.0.0.1".into(),
                address.port(),
                server.public(),
                crate::pairing::now().unwrap() + 60,
            )
            .unwrap();
            offer.open_window("QR computer").unwrap();
            let mut qr = QrPayload::parse(&offer.qr_payload()).unwrap();
            if wrong_pin {
                qr.public_key = [0; 32];
            }
            let host = thread::spawn(move || {
                let (socket, _) = listener.accept().unwrap();
                let first = RemoteStream::accept(socket, &server, &[], Some(&mut offer), TIMEOUT);
                assert_eq!(first.is_ok(), !wrong_pin);
                if !wrong_pin {
                    let (socket, _) = listener.accept().unwrap();
                    assert!(
                        RemoteStream::accept(socket, &server, &[], Some(&mut offer), TIMEOUT)
                            .is_err()
                    );
                }
            });
            let first =
                RemoteStream::connect_qr(address, &client, &qr, TIMEOUT, &Resolver::default());
            assert_eq!(first.is_ok(), !wrong_pin);
            if !wrong_pin {
                assert_eq!(first.unwrap().computer_name.as_deref(), Some("QR computer"));
                assert!(
                    RemoteStream::connect_qr(address, &client, &qr, TIMEOUT, &Resolver::default())
                        .is_err()
                );
            }
            host.join().unwrap();
        }
    }

    #[test]
    fn typed_code_has_three_attempts_across_connections() {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Code computer").unwrap();
        let code = offer.code().to_owned();
        let wrong = if code == "00000000" {
            "11111111"
        } else {
            "00000000"
        };
        let host = thread::spawn(move || {
            for _ in 0..4 {
                let (socket, _) = listener.accept().unwrap();
                assert!(
                    RemoteStream::accept_with(
                        socket,
                        &server,
                        &[],
                        Some(&mut offer),
                        TIMEOUT,
                        |_| panic!("a failed code must not save trust")
                    )
                    .is_err()
                );
            }
        });
        for attempt in 0..4 {
            let entered = if attempt < 3 { wrong } else { &code };
            assert!(
                RemoteStream::connect_code(
                    address,
                    &client,
                    entered,
                    TIMEOUT,
                    &Resolver::default()
                )
                .is_err()
            );
        }
        host.join().unwrap();
    }

    #[test]
    fn a_closed_pairing_window_rejects_the_correct_code() {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Closed computer").unwrap();
        let code = offer.code().to_owned();
        offer.window().close();
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            assert!(RemoteStream::accept(socket, &server, &[], Some(&mut offer), TIMEOUT).is_err());
        });
        assert!(
            RemoteStream::connect_code(address, &client, &code, TIMEOUT, &Resolver::default())
                .is_err()
        );
        host.join().unwrap();
    }

    #[test]
    fn nearby_pairing_compares_the_same_number_and_requires_both_approvals() {
        for (computer_approves, phone_approves) in
            [(true, true), (true, false), (false, true), (false, false)]
        {
            let server = Identity::generate().unwrap();
            let client = Identity::generate().unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let mut offer = PairOffer::new(
                "127.0.0.1".into(),
                address.port(),
                server.public(),
                crate::pairing::now().unwrap() + 60,
            )
            .unwrap();
            offer.open_window("My computer").unwrap();
            let phone_key = client.public();
            let (sender, receiver) = mpsc::sync_channel(1);
            let saved = Arc::new(AtomicBool::new(false));
            let saved_by_server = Arc::clone(&saved);
            let host = thread::spawn(move || {
                let (socket, _) = listener.accept().unwrap();
                let mut result = RemoteStream::accept_pairing_peer_with(
                    socket,
                    &server,
                    &[],
                    Some(&mut offer),
                    TIMEOUT,
                    |number, name, key| {
                        assert_eq!(name, "My phone");
                        assert_eq!(*key, phone_key);
                        sender.send(number.to_owned()).unwrap();
                        Ok(computer_approves)
                    },
                    |_| {
                        saved_by_server.store(true, Ordering::Release);
                        Ok(())
                    },
                );
                assert_eq!(result.is_ok(), computer_approves && phone_approves);
                if result.is_err() {
                    assert_eq!(offer.tries_left(), 3);
                }
                if let Ok(session) = &mut result {
                    offer.close_window();
                    let mut payload = [0; 4];
                    session.stream.read_exact(&mut payload).unwrap();
                    assert_eq!(&payload, b"ping");
                }
            });
            let mut phone_number = String::new();
            let mut result = RemoteStream::connect_nearby_named(
                address,
                &client,
                "My phone",
                TIMEOUT,
                &Resolver::default(),
                |number, name| {
                    assert_eq!(name, "My computer");
                    phone_number = number.to_owned();
                    Ok(phone_approves)
                },
            );
            assert_eq!(result.is_ok(), computer_approves && phone_approves);
            assert_eq!(phone_number, receiver.recv_timeout(TIMEOUT).unwrap());
            assert_eq!(phone_number.len(), 6);
            if let Ok(session) = &mut result {
                session.stream.write_all(b"ping").unwrap();
            }
            host.join().unwrap();
            assert_eq!(
                saved.load(Ordering::Acquire),
                computer_approves && phone_approves
            );
        }
    }

    #[test]
    fn nearby_rejects_missing_or_invalid_peer_names_without_spending_code_tries() {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Computer").unwrap();
        let host = thread::spawn(move || {
            for _ in 0..4 {
                let (socket, _) = listener.accept().unwrap();
                assert!(
                    RemoteStream::accept_pairing_peer_with(
                        socket,
                        &server,
                        &[],
                        Some(&mut offer),
                        TIMEOUT,
                        |_, _, _| panic!("invalid names must not reach comparison"),
                        |_| panic!("invalid names must not save trust"),
                    )
                    .is_err()
                );
                assert_eq!(offer.tries_left(), 3);
            }
        });
        for payload in [
            &b"nearby-v2"[..],
            b"nearby-v2\0",
            b"nearby-v2\0\n",
            b"nearby-v2\0\xff",
        ] {
            let mut socket = TcpStream::connect(address).unwrap();
            configure(&socket, TIMEOUT).unwrap();
            let deadline = Instant::now() + TIMEOUT;
            let mut noise = builder()
                .unwrap()
                .local_private_key(&client.private)
                .unwrap()
                .build_initiator()
                .unwrap();
            send_handshake(&mut socket, &mut noise, &[], deadline).unwrap();
            receive_handshake(&mut socket, &mut noise, deadline).unwrap();
            send_handshake(&mut socket, &mut noise, payload, deadline).unwrap();
        }
        host.join().unwrap();
    }

    #[test]
    fn nearby_number_comparison_outlives_the_preauth_deadline() {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Computer").unwrap();
        let timeout = Duration::from_secs(7);
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            RemoteStream::accept_pairing_peer_with(
                socket,
                &server,
                &[],
                Some(&mut offer),
                timeout,
                |_, _, _| {
                    thread::sleep(PREAUTH_TIMEOUT + Duration::from_millis(100));
                    Ok(true)
                },
                |_| Ok(()),
            )
            .unwrap();
        });
        RemoteStream::connect_nearby_named(
            address,
            &client,
            "Phone",
            timeout,
            &Resolver::default(),
            |_, _| Ok(true),
        )
        .unwrap();
        host.join().unwrap();
    }

    #[test]
    fn closing_the_window_during_number_confirmation_cannot_save_trust() {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Closing computer").unwrap();
        let window = offer.window();
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            assert!(
                RemoteStream::accept_pairing_with(
                    socket,
                    &server,
                    &[],
                    Some(&mut offer),
                    TIMEOUT,
                    |_, _| {
                        window.close();
                        Ok(true)
                    },
                    |_| panic!("a closed window must not save trust")
                )
                .is_err()
            );
        });
        assert!(
            RemoteStream::connect_nearby(
                address,
                &client,
                TIMEOUT,
                &Resolver::default(),
                |_, _| Ok(true)
            )
            .is_err()
        );
        host.join().unwrap();
    }

    #[test]
    fn a_silent_preauth_peer_cannot_use_the_long_comparison_timeout() {
        let server = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Computer").unwrap();
        let (completed, result) = mpsc::sync_channel(1);
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let started = Instant::now();
            let result = RemoteStream::accept_pairing_peer_with(
                socket,
                &server,
                &[],
                Some(&mut offer),
                Duration::from_secs(60),
                |_, _, _| panic!("a silent peer cannot reach comparison"),
                |_| panic!("a silent peer cannot save trust"),
            );
            completed
                .send((result.is_err(), started.elapsed()))
                .unwrap();
        });
        let peer = TcpStream::connect(address).unwrap();
        let (failed, elapsed) = result.recv_timeout(Duration::from_secs(7)).unwrap();
        assert!(failed);
        assert!(elapsed >= Duration::from_secs(4));
        assert!(elapsed < Duration::from_secs(7));
        drop(peer);
        host.join().unwrap();
    }

    #[test]
    fn closing_the_window_interrupts_a_peer_stalled_before_noise() {
        let server = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Computer").unwrap();
        let window = offer.window();
        let (started, ready) = mpsc::sync_channel(1);
        let (completed, result) = mpsc::sync_channel(1);
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            started.send(()).unwrap();
            completed
                .send(
                    RemoteStream::accept(
                        socket,
                        &server,
                        &[],
                        Some(&mut offer),
                        Duration::from_secs(10),
                    )
                    .is_err(),
                )
                .unwrap();
        });
        let _stalled = TcpStream::connect(address).unwrap();
        ready.recv_timeout(TIMEOUT).unwrap();
        thread::sleep(Duration::from_millis(100));
        let closing = Instant::now();
        window.close();
        // Well under the 10 s handshake limit, with room for a busy CI runner.
        assert!(result.recv_timeout(Duration::from_secs(3)).unwrap());
        assert!(closing.elapsed() < Duration::from_secs(3));
        host.join().unwrap();
    }

    #[test]
    fn a_delayed_number_approval_cannot_save_trust_after_the_deadline() {
        use sha2::{Digest, Sha256};

        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server.public(),
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Computer").unwrap();
        let saved = Arc::new(AtomicBool::new(false));
        let saved_by_server = Arc::clone(&saved);
        let (completed, result) = mpsc::sync_channel(1);
        let (comparing, comparison) = mpsc::sync_channel(1);
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let accepted = RemoteStream::accept_pairing_with(
                socket,
                &server,
                &[],
                Some(&mut offer),
                Duration::from_millis(200),
                |_, _| {
                    comparing.send(()).unwrap();
                    thread::sleep(Duration::from_millis(300));
                    Ok(true)
                },
                |_| {
                    saved_by_server.store(true, Ordering::Release);
                    Ok(())
                },
            );
            completed.send(accepted.is_err()).unwrap();
        });
        let started = Instant::now();
        let deadline = started + TIMEOUT;
        let mut socket = TcpStream::connect_timeout(&address, TIMEOUT).unwrap();
        configure(&socket, TIMEOUT).unwrap();
        let mut noise = builder()
            .unwrap()
            .local_private_key(&client.private)
            .unwrap()
            .build_initiator()
            .unwrap();
        send_handshake(&mut socket, &mut noise, &[], deadline).unwrap();
        let name = receive_handshake(&mut socket, &mut noise, deadline).unwrap();
        send_handshake(&mut socket, &mut noise, b"nearby-v2\0Phone", deadline).unwrap();
        let context = pairing::context(
            &peer_key(&noise).unwrap(),
            &client.public(),
            std::str::from_utf8(&name).unwrap(),
            noise.get_handshake_hash(),
        );
        let mut stream = RemoteStream::from_handshake(socket, noise, TIMEOUT, deadline).unwrap();
        let nonce = [42; 32];
        let mut commitment = Sha256::new();
        for part in [b"commit".as_slice(), &context, b"phone", &nonce] {
            commitment.update((part.len() as u64).to_be_bytes());
            commitment.update(part);
        }
        stream.write_all(&commitment.finalize()).unwrap();
        let mut peer_commitment = [0; 32];
        stream.read_exact(&mut peer_commitment).unwrap();
        // One encrypted packet leaves approval buffered while the computer waits for its user.
        let mut reveal_and_approval = nonce.to_vec();
        reveal_and_approval.push(1);
        stream.write_all(&reveal_and_approval).unwrap();
        let mut peer_nonce = [0; 32];
        stream.read_exact(&mut peer_nonce).unwrap();
        comparison.recv_timeout(TIMEOUT).unwrap();
        assert!(result.recv_timeout(TIMEOUT).unwrap());
        assert!(started.elapsed() < TIMEOUT);
        host.join().unwrap();
        assert!(!saved.load(Ordering::Acquire));
    }

    #[test]
    fn unknown_keys_and_wrong_server_pins_are_refused() {
        for wrong_server in [false, true] {
            let server = Identity::generate().unwrap();
            let client = Identity::generate().unwrap();
            let server_key = if wrong_server {
                [0; 32]
            } else {
                server.public()
            };
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let host = thread::spawn(move || {
                let (socket, _) = listener.accept().unwrap();
                assert!(RemoteStream::accept(socket, &server, &[], None, TIMEOUT).is_err());
            });
            assert!(
                RemoteStream::connect(
                    address,
                    &client,
                    server_key,
                    None,
                    TIMEOUT,
                    &Resolver::default()
                )
                .is_err()
            );
            host.join().unwrap();
        }
    }

    #[test]
    fn pairing_has_no_success_receipt_when_the_peer_key_cannot_be_saved() {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let server_key = server.public();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut offer = PairOffer::new(
            "127.0.0.1".into(),
            address.port(),
            server_key,
            crate::pairing::now().unwrap() + 60,
        )
        .unwrap();
        offer.open_window("Test computer").unwrap();
        let scanned = PairOffer::from_bootstrap(&offer.to_bootstrap()).unwrap();
        let host = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let result =
                RemoteStream::accept_with(socket, &server, &[], Some(&mut offer), TIMEOUT, |_| {
                    Err(io::Error::other("pin save failed"))
                });
            assert!(result.is_err());
        });
        assert!(
            RemoteStream::connect(
                address,
                &client,
                server_key,
                Some(&scanned),
                TIMEOUT,
                &Resolver::default()
            )
            .is_err()
        );
        host.join().unwrap();
    }

    #[test]
    fn a_changed_ciphertext_is_refused_and_closes_the_connection() {
        let (client, mut server) = connected();
        let (_, mut writer) = client.split().unwrap();
        let mut packet = vec![0; 20];
        writer
            .cipher
            .write_message(writer.nonce, b"ping", &mut packet)
            .unwrap();
        packet[0] ^= 1;
        send_packet(&mut writer.socket, &packet, None).unwrap();
        let mut answer = [0; 4];
        assert_eq!(
            server.read(&mut answer).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(server.read(&mut answer).is_err());
    }

    #[test]
    fn malformed_packets_and_oversized_frames_are_refused() {
        let (client, mut server) = connected();
        let (_, mut writer) = client.split().unwrap();
        writer.socket.write_all(&0_u16.to_le_bytes()).unwrap();
        assert!(server.read(&mut [0]).is_err());
        let (mut client, mut server) = connected();
        client
            .write_all(&((protocol::MAX_FRAME + 1) as u32).to_le_bytes())
            .unwrap();
        client.write_all(&[0]).unwrap();
        assert!(matches!(
            protocol::read_frame(&mut server),
            Err(protocol::FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn a_silent_peer_hits_the_read_deadline() {
        let (_client, mut server) = connected();
        server
            .reader
            .socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let error = server.read(&mut [0]).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
    }

    #[test]
    fn a_slow_byte_stream_cannot_extend_the_packet_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let sender = thread::spawn(move || {
            let mut socket = TcpStream::connect(address).unwrap();
            socket.write_all(&10_u16.to_le_bytes()).unwrap();
            for _ in 0..10 {
                thread::sleep(Duration::from_millis(30));
                if socket.write_all(&[0]).is_err() {
                    break;
                }
            }
        });
        let (mut socket, _) = listener.accept().unwrap();
        configure(&socket, Duration::from_millis(80)).unwrap();
        assert!(receive_packet(&mut socket, 4096, Duration::from_millis(80), None, None).is_err());
        drop(socket);
        sender.join().unwrap();
    }

    #[test]
    fn runtime_waits_longer_than_handshake_but_partial_packets_stay_bounded() {
        let (mut client, mut server) = connected_with_timeout(Duration::from_millis(100));
        assert_eq!(
            client.reader.socket.read_timeout().unwrap(),
            Some(RUNTIME_IDLE)
        );
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            server.write_all(b"ready").unwrap();
            server
        });
        let mut answer = [0; 5];
        client.read_exact(&mut answer).unwrap();
        assert_eq!(&answer, b"ready");
        let server = sender.join().unwrap();
        let (_, mut writer) = server.split().unwrap();
        writer.socket.write_all(&[32]).unwrap();
        assert!(client.read(&mut answer).is_err());
    }

    #[test]
    fn selected_worker_idle_wait_outlives_the_old_timeout_but_packets_stay_bounded() {
        let (mut client, mut server) = connected_with_timeout(Duration::from_millis(100));
        select_worker(&mut client, WorkerKind::Speech).unwrap();
        assert_eq!(read_worker(&mut server).unwrap(), WorkerKind::Speech);
        client
            .reader
            .socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        client.allow_idle_wait().unwrap();
        server.allow_idle_wait().unwrap();
        assert_eq!(client.reader.socket.read_timeout().unwrap(), None);
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            server.write_all(b"ready").unwrap();
            server
        });
        let mut answer = [0; 5];
        client.read_exact(&mut answer).unwrap();
        assert_eq!(&answer, b"ready");
        let server = sender.join().unwrap();
        let (_, mut writer) = server.split().unwrap();
        client
            .reader
            .set_read_deadline(Some(Instant::now() + Duration::from_secs(2)));
        writer.socket.write_all(&[32]).unwrap();
        let started = Instant::now();
        assert!(client.read(&mut answer).is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn an_unselected_authenticated_stream_keeps_its_idle_deadline() {
        let (_client, mut server) = connected();
        assert!(server.reader.socket.read_timeout().unwrap().is_some());
        server
            .reader
            .socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        assert!(read_worker(&mut server).is_err());
    }

    #[test]
    fn an_active_transfer_deadline_bounds_an_idle_allowed_reader() {
        let (mut client, mut server) = connected();
        select_worker(&mut client, WorkerKind::Speech).unwrap();
        assert_eq!(read_worker(&mut server).unwrap(), WorkerKind::Speech);
        server.allow_idle_wait().unwrap();
        let control = server.control();
        let (mut reader, _writer) = server.split().unwrap();
        reader.set_read_deadline(Some(Instant::now() + Duration::from_millis(30)));
        let (sent, received) = mpsc::sync_channel(1);
        let reading = thread::spawn(move || {
            sent.send(reader.read(&mut [0])).unwrap();
        });
        let result = received.recv_timeout(Duration::from_secs(1));
        let _ = control.shutdown();
        reading.join().unwrap();
        let problem = result.unwrap().unwrap_err();
        assert!(matches!(
            problem.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn a_control_handle_interrupts_a_blocked_read() {
        let (mut client, _server) = connected();
        let control = client.control();
        let (sender, receiver) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || {
            let result = client.read(&mut [0]);
            let _ = sender.send(result.map(|count| count == 0).unwrap_or(true));
        });
        control.shutdown().unwrap();
        assert!(receiver.recv_timeout(Duration::from_secs(1)).unwrap());
        reader.join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn cancellation_stops_a_backpressured_write_without_os_shutdown() {
        let (client, _server) = connected();
        let control = client.control();
        let (_, mut writer) = client.split().unwrap();
        let padding = vec![0; 64 * 1024];
        let deadline = Instant::now() + TIMEOUT;
        let mut filled = 0;
        loop {
            match writer.socket.write(&padding) {
                Ok(count) if count > 0 => filled += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                result => panic!("the nonblocking socket must fill: {result:?}"),
            }
            assert!(filled < 64 * 1024 * 1024 && Instant::now() < deadline);
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let writing = thread::spawn(move || {
            let result = writer.write_all(&vec![1; 32 * 1024 * 1024]);
            let _ = sender.send(result);
        });
        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        control.cancelled.store(true, Ordering::Release);
        let error = receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
        writing.join().unwrap();
        let _ = control.shutdown();
    }

    #[cfg(unix)]
    #[test]
    fn shutdown_interrupts_a_backpressured_blocking_write() {
        let (client, _server) = connected();
        let control = client.control();
        let (_, mut writer) = client.split().unwrap();
        writer.socket.set_nonblocking(true).unwrap();
        let padding = vec![0; 64 * 1024];
        let deadline = Instant::now() + TIMEOUT;
        let mut filled = 0;
        loop {
            match writer.socket.write(&padding) {
                Ok(count) if count > 0 => filled += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                result => panic!("the nonblocking socket must fill: {result:?}"),
            }
            assert!(filled < 64 * 1024 * 1024 && Instant::now() < deadline);
        }
        writer.socket.set_nonblocking(false).unwrap();
        let (sent, received) = mpsc::sync_channel(1);
        let writing = thread::spawn(move || {
            sent.send(writer.write_all(&vec![1; 32 * 1024 * 1024]))
                .unwrap();
        });
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        control.shutdown().unwrap();
        assert!(
            received
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );
        writing.join().unwrap();
    }

    #[test]
    fn a_slow_lookup_times_out_and_blocks_further_lookup_threads() {
        struct SlowAddress(mpsc::Receiver<()>);
        impl std::fmt::Display for SlowAddress {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("slow.invalid:1")
            }
        }
        impl ToSocketAddrs for SlowAddress {
            type Iter = std::vec::IntoIter<SocketAddr>;
            fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
                let _ = self.0.recv_timeout(Duration::from_secs(1));
                Ok(vec!["127.0.0.1:1".parse().unwrap()].into_iter())
            }
        }
        let resolver = Resolver::default();
        let (release, wait) = mpsc::sync_channel(1);
        let error = resolver
            .resolve(
                SlowAddress(wait),
                Instant::now() + Duration::from_millis(20),
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        let address: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let error = resolver
            .clone()
            .resolve("localhost:1".to_owned(), Instant::now() + TIMEOUT)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(
            resolver.resolve(address, Instant::now() + TIMEOUT).unwrap(),
            vec![address]
        );
        release.send(()).unwrap();
        let deadline = Instant::now() + TIMEOUT;
        while resolver.busy.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            resolver.resolve(address, Instant::now() + TIMEOUT).unwrap(),
            vec![address]
        );
    }

    #[test]
    fn a_peer_that_never_handshakes_cannot_use_the_runtime_idle_allowance() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (release, wait) = mpsc::sync_channel::<()>(1);
        let server = thread::spawn(move || {
            let (_socket, _) = listener.accept().unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(1));
        });
        let identity = Identity::generate().unwrap();
        let result = RemoteStream::connect(
            address,
            &identity,
            [1; 32],
            None,
            Duration::from_millis(50),
            &Resolver::default(),
        );
        let error = result.err().expect("the handshake must time out");
        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        release.send(()).unwrap();
        server.join().unwrap();
    }
}
