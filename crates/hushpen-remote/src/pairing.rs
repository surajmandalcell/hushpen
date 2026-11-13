use crate::{RemoteControl, RemoteStream, denied, invalid};
use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

pub(crate) const NEARBY: &[u8] = b"nearby-v2";
pub(crate) const TYPED: &[u8] = b"spake2-v2";
pub(crate) const QR: &[u8] = b"qr-v2";

/// Closing the sheet closes its unfinished connections, including number comparisons.
#[derive(Clone, Default)]
pub struct PairingWindow(Arc<Mutex<Window>>);

#[derive(Default)]
struct Window {
    name: Option<String>,
    cancelled: bool,
    connections: Vec<RemoteControl>,
}

pub(crate) struct WindowConnection {
    window: PairingWindow,
    control: RemoteControl,
}

impl Drop for WindowConnection {
    fn drop(&mut self) {
        if let Ok(mut window) = self.window.0.lock() {
            window
                .connections
                .retain(|connection| !connection.same_connection(&self.control));
        }
    }
}

impl PairingWindow {
    pub fn close(&self) {
        let mut window = self.0.lock().unwrap_or_else(|e| e.into_inner());
        window.name = None;
        window.cancelled = true;
        for connection in window.connections.drain(..) {
            let _ = connection.shutdown();
        }
    }

    pub fn is_open(&self) -> bool {
        self.0.lock().is_ok_and(|window| window.name.is_some())
    }

    pub(crate) fn name(&self) -> io::Result<String> {
        self.0
            .lock()
            .map_err(|_| denied())?
            .name
            .clone()
            .ok_or_else(denied)
    }

    pub(crate) fn track(&self, control: RemoteControl) -> io::Result<WindowConnection> {
        let mut window = self.0.lock().map_err(|_| denied())?;
        if window.name.is_none() {
            return Err(denied());
        }
        window.connections.push(control.clone());
        Ok(WindowConnection {
            window: self.clone(),
            control,
        })
    }

    pub(crate) fn save(&self, save: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
        let window = self.0.lock().map_err(|_| denied())?;
        if window.name.is_none() {
            return Err(denied());
        }
        save()
    }
}

/// Private bootstrap state. Only the QR payload can be displayed or scanned.
pub struct PairOffer {
    pub host: String,
    pub port: u16,
    pub public_key: [u8; 32],
    pub expires_unix_secs: u64,
    code: String,
    token: [u8; 32],
    attempts: u8,
    used: bool,
    window: PairingWindow,
}

/// A QR pins the computer key and proves possession of a random one-use secret.
pub struct QrPayload {
    pub host: String,
    pub port: u16,
    pub public_key: [u8; 32],
    pub expires_unix_secs: u64,
    token: [u8; 32],
}

impl PairOffer {
    pub fn new(
        host: String,
        port: u16,
        public_key: [u8; 32],
        expires_unix_secs: u64,
    ) -> io::Result<Self> {
        validate_address(&host, port)?;
        let time = now()?;
        if expires_unix_secs <= time || expires_unix_secs > time.saturating_add(120) {
            return Err(invalid("invalid pairing expiry"));
        }
        let code = loop {
            let value = u32::from_le_bytes(random::<4>()?);
            if value < 4_200_000_000 {
                break format!("{:08}", value % 100_000_000);
            }
        };
        Ok(Self {
            host,
            port,
            public_key,
            expires_unix_secs,
            code,
            token: random()?,
            attempts: 0,
            used: false,
            window: PairingWindow::default(),
        })
    }

    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn window(&self) -> PairingWindow {
        self.window.clone()
    }

    /// Typed or scanned tries that remain across all connections.
    pub fn tries_left(&self) -> u8 {
        if self.used {
            0
        } else {
            3u8.saturating_sub(self.attempts)
        }
    }

    pub fn open_window(&mut self, computer_name: &str) -> io::Result<()> {
        self.check()?;
        crate::enrollment::validate_name(computer_name)?;
        let mut window = self.window.0.lock().map_err(|_| denied())?;
        if window.name.is_some() || window.cancelled {
            return Err(denied());
        }
        window.name = Some(computer_name.into());
        Ok(())
    }

    pub fn close_window(&mut self) {
        self.window.close();
        self.used = true;
    }

    pub fn qr_payload(&self) -> String {
        json!({"version":2,"host":self.host,"port":self.port,"key":STANDARD.encode(self.public_key), "expires":self.expires_unix_secs,"token":STANDARD.encode(self.token)}).to_string()
    }

    /// Private file or SSH pipe only. This contains the typed secret.
    pub fn to_bootstrap(&self) -> String {
        json!({"qr":self.qr_payload(),"code":self.code}).to_string()
    }

    pub fn from_bootstrap(text: &str) -> io::Result<Self> {
        let value = object(text, 2)?;
        let qr = QrPayload::parse(value["qr"].as_str().ok_or_else(|| invalid("missing QR"))?)?;
        let code = value["code"]
            .as_str()
            .ok_or_else(|| invalid("missing pairing code"))?;
        validate_code(code)?;
        Ok(Self {
            host: qr.host,
            port: qr.port,
            public_key: qr.public_key,
            expires_unix_secs: qr.expires_unix_secs,
            token: qr.token,
            code: code.into(),
            attempts: 0,
            used: false,
            window: PairingWindow::default(),
        })
    }

    pub(crate) fn check(&self) -> io::Result<()> {
        self.check_at(now()?)
    }

    fn check_at(&self, time: u64) -> io::Result<()> {
        if self.used || self.attempts >= 3 || time >= self.expires_unix_secs {
            return Err(denied());
        }
        Ok(())
    }

    pub(crate) fn begin_attempt(&mut self) -> io::Result<()> {
        self.check()?;
        self.window.name()?;
        self.attempts += 1;
        Ok(())
    }

    pub(crate) fn consume(&mut self) -> io::Result<()> {
        // The third in-flight attempt may succeed, but no fourth attempt may start.
        if self.used || now()? >= self.expires_unix_secs || !self.window.is_open() {
            return Err(denied());
        }
        self.used = true;
        Ok(())
    }
}

impl QrPayload {
    pub fn parse(text: &str) -> io::Result<Self> {
        let value = object(text, 6)?;
        if value["version"] != 2 {
            return Err(invalid("invalid pairing version"));
        }
        let host = value["host"]
            .as_str()
            .ok_or_else(|| invalid("invalid pairing host"))?
            .to_owned();
        let port = value["port"]
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(|| invalid("invalid pairing port"))?;
        validate_address(&host, port)?;
        let expires_unix_secs = value["expires"]
            .as_u64()
            .ok_or_else(|| invalid("invalid pairing expiry"))?;
        let time = now()?;
        if expires_unix_secs <= time || expires_unix_secs > time.saturating_add(120) {
            return Err(denied());
        }
        Ok(Self {
            host,
            port,
            public_key: key(&value["key"])?,
            expires_unix_secs,
            token: key(&value["token"])?,
        })
    }
}

fn object(text: &str, fields: usize) -> io::Result<Value> {
    if text.len() > 2048 {
        return Err(invalid("pairing payload is too long"));
    }
    let value: Value =
        serde_json::from_str(text).map_err(|_| invalid("invalid pairing payload"))?;
    if !value.as_object().is_some_and(|o| o.len() == fields) {
        return Err(invalid("invalid pairing fields"));
    }
    Ok(value)
}

fn key(value: &Value) -> io::Result<[u8; 32]> {
    STANDARD
        .decode(
            value
                .as_str()
                .ok_or_else(|| invalid("invalid pairing key"))?,
        )
        .map_err(|_| invalid("invalid pairing key"))?
        .try_into()
        .map_err(|_| invalid("invalid pairing key"))
}

fn validate_address(host: &str, port: u16) -> io::Result<()> {
    if port == 0
        || host.is_empty()
        || host.len() > 253
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:-".contains(&b))
    {
        return Err(invalid("invalid pairing address"));
    }
    Ok(())
}

fn validate_code(code: &str) -> io::Result<()> {
    if code.len() != 8 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("invalid pairing code"));
    }
    Ok(())
}

pub(crate) fn now() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_secs())
        .map_err(io::Error::other)
}

fn random<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    DefaultResolver
        .resolve_rng()
        .ok_or_else(|| io::Error::other("random source unavailable"))?
        .try_fill_bytes(&mut bytes)
        .map_err(io::Error::other)?;
    Ok(bytes)
}

/// Both keys, fixed roles, validated computer name, and the completed Noise transcript.
pub(crate) fn context(
    computer: &[u8; 32],
    phone: &[u8; 32],
    name: &str,
    transcript: &[u8],
) -> [u8; 32] {
    hash(&[
        b"hushpen-pair-v2",
        b"computer",
        computer,
        b"phone",
        phone,
        name.as_bytes(),
        transcript,
    ])
}

fn hash(parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}

fn commitment(context: &[u8; 32], role: &[u8], nonce: &[u8; 32]) -> [u8; 32] {
    hash(&[b"commit", context, role, nonce])
}

fn verify(expected: &[u8; 32], actual: &[u8; 32]) -> io::Result<()> {
    if !bool::from(expected.ct_eq(actual)) {
        return Err(denied());
    }
    Ok(())
}

fn number(context: &[u8; 32], computer: &[u8; 32], phone: &[u8; 32]) -> String {
    let digest = hash(&[b"number", context, computer, phone]);
    format!(
        "{:06}",
        u64::from_be_bytes(digest[..8].try_into().expect("eight bytes")) % 1_000_000
    )
}

pub(crate) fn nearby(
    stream: &mut RemoteStream,
    context: &[u8; 32],
    computer: bool,
) -> io::Result<String> {
    let nonce = random()?;
    let role: &[u8] = if computer { b"computer" } else { b"phone" };
    let other: &[u8] = if computer { b"phone" } else { b"computer" };
    let committed = commitment(context, role, &nonce);
    let mut peer_commit = [0; 32];
    // Neither peer reveals until both commitments have crossed the wire.
    if computer {
        stream.read_exact(&mut peer_commit)?;
    }
    stream.write_all(&committed)?;
    if !computer {
        stream.read_exact(&mut peer_commit)?;
    }
    let mut peer_nonce = [0; 32];
    if computer {
        stream.read_exact(&mut peer_nonce)?;
        verify(&peer_commit, &commitment(context, other, &peer_nonce))?;
    }
    stream.write_all(&nonce)?;
    if !computer {
        stream.read_exact(&mut peer_nonce)?;
        verify(&peer_commit, &commitment(context, other, &peer_nonce))?;
    }
    Ok(if computer {
        number(context, &nonce, &peer_nonce)
    } else {
        number(context, &peer_nonce, &nonce)
    })
}

pub(crate) fn pake(
    stream: &mut RemoteStream,
    context: &[u8; 32],
    code: &str,
    computer: bool,
) -> io::Result<()> {
    validate_code(code)?;
    let (state, message) = start_pake(context, code, computer);
    stream.write_all(&message)?;
    let mut peer_message = [0; 33];
    stream.read_exact(&mut peer_message)?;
    let key = state.finish(&peer_message).map_err(|_| denied())?;
    prove(stream, context, &key, computer)
}

fn start_pake(context: &[u8; 32], code: &str, computer: bool) -> (Spake2<Ed25519Group>, Vec<u8>) {
    let phone = hash(&[b"phone", context]);
    let desktop = hash(&[b"computer", context]);
    let start = if computer {
        Spake2::<Ed25519Group>::start_b
    } else {
        Spake2::<Ed25519Group>::start_a
    };
    start(
        &Password::new(code.as_bytes()),
        &Identity::new(&phone),
        &Identity::new(&desktop),
    )
}

fn proof(key: &[u8], context: &[u8; 32], role: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(b"hushpen-pair-confirm-v2");
    mac.update(context);
    mac.update(role);
    mac.finalize().into_bytes().into()
}

pub(crate) fn prove(
    stream: &mut RemoteStream,
    context: &[u8; 32],
    key: &[u8],
    computer: bool,
) -> io::Result<()> {
    let role: &[u8] = if computer { b"computer" } else { b"phone" };
    let other: &[u8] = if computer { b"phone" } else { b"computer" };
    stream.write_all(&proof(key, context, role))?;
    let mut received = [0; 32];
    stream.read_exact(&mut received)?;
    verify(&proof(key, context, other), &received)
}

pub(crate) fn qr_server(
    stream: &mut RemoteStream,
    context: &[u8; 32],
    offer: &PairOffer,
) -> io::Result<()> {
    prove(stream, context, &offer.token, true)
}
pub(crate) fn qr_client(
    stream: &mut RemoteStream,
    context: &[u8; 32],
    qr: &QrPayload,
) -> io::Result<()> {
    prove(stream, context, &qr.token, false)
}

pub(crate) fn confirm(stream: &mut RemoteStream, accepted: bool) -> io::Result<()> {
    stream.write_all(&[u8::from(accepted)])?;
    let mut peer = [0];
    stream.read_exact(&mut peer)?;
    if !accepted || peer != [1] {
        return Err(denied());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn offer() -> PairOffer {
        PairOffer::new(
            "computer.local".into(),
            47321,
            [3; 32],
            now().unwrap() + 120,
        )
        .unwrap()
    }

    #[test]
    fn numbers_bind_both_keys_name_and_noise_transcript() {
        let context = context(&[1; 32], &[2; 32], "Computer", &[3; 32]);
        let left = number(&context, &[4; 32], &[5; 32]);
        assert_eq!(left.len(), 6);
        for swapped in [
            super::context(&[9; 32], &[2; 32], "Computer", &[3; 32]),
            super::context(&[1; 32], &[9; 32], "Computer", &[3; 32]),
            super::context(&[1; 32], &[2; 32], "Imposter", &[3; 32]),
            super::context(&[1; 32], &[2; 32], "Computer", &[9; 32]),
        ] {
            assert_ne!(left, number(&swapped, &[4; 32], &[5; 32]));
        }
    }

    #[test]
    fn commitments_reject_changed_randomness_roles_and_context() {
        let committed = commitment(&[1; 32], b"phone", &[2; 32]);
        assert!(verify(&committed, &commitment(&[1; 32], b"phone", &[2; 32])).is_ok());
        for changed in [
            commitment(&[1; 32], b"phone", &[3; 32]),
            commitment(&[1; 32], b"computer", &[2; 32]),
            commitment(&[3; 32], b"phone", &[2; 32]),
        ] {
            assert!(verify(&committed, &changed).is_err());
        }
    }

    #[test]
    fn spake2_requires_same_code_context_and_mutual_role_proofs() {
        for (code, context) in [
            ("12345678", [1; 32]),
            ("87654321", [1; 32]),
            ("12345678", [2; 32]),
        ] {
            let (a, am) = start_pake(&[1; 32], "12345678", false);
            let (b, bm) = start_pake(&context, code, true);
            let ak = a.finish(&bm).unwrap();
            let bk = b.finish(&am).unwrap();
            let expected = code == "12345678" && context == [1; 32];
            assert_eq!(ak == bk, expected);
            assert_eq!(
                verify(
                    &proof(&ak, &[1; 32], b"computer"),
                    &proof(&bk, &context, b"computer")
                )
                .is_ok(),
                expected
            );
            assert_ne!(
                proof(&ak, &[1; 32], b"computer"),
                proof(&ak, &[1; 32], b"phone")
            );
        }
    }

    #[test]
    fn offers_expire_limit_three_attempts_and_cannot_be_reused_or_reopened() {
        let mut first = offer();
        assert!(first.begin_attempt().is_err());
        first.open_window("Computer").unwrap();
        for _ in 0..3 {
            first.begin_attempt().unwrap();
        }
        assert!(first.begin_attempt().is_err());
        first.consume().unwrap();
        assert!(first.begin_attempt().is_err());
        assert!(first.consume().is_err());
        let mut expiring = offer();
        assert!(expiring.check_at(expiring.expires_unix_secs - 1).is_ok());
        assert!(expiring.check_at(expiring.expires_unix_secs).is_err());
        expiring.open_window("Computer").unwrap();
        expiring.window().close();
        assert!(expiring.begin_attempt().is_err());
        assert!(expiring.open_window("Computer").is_err());
        let mut cancelled = offer();
        cancelled.window().close();
        assert!(cancelled.open_window("Computer").is_err());
    }

    #[test]
    fn qr_pins_key_without_exposing_typed_code() {
        let offer = offer();
        let payload = offer.qr_payload();
        assert!(!payload.contains("code"));
        let qr = QrPayload::parse(&payload).unwrap();
        assert_eq!(qr.public_key, offer.public_key);
        assert_eq!(qr.token, offer.token);
        assert_eq!(offer.code().len(), 8);
        let restored = PairOffer::from_bootstrap(&offer.to_bootstrap()).unwrap();
        assert_eq!(restored.code(), offer.code());
        assert!(!restored.window.is_open());
        assert!(QrPayload::parse("{}").is_err());
    }
}
