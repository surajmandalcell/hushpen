use crate::{denied, invalid};
use base64::{Engine, engine::general_purpose::STANDARD};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use std::collections::BTreeMap;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// A short-lived, single-use offer. Its code must not be logged.
pub struct PairOffer {
    pub host: String,
    pub port: u16,
    pub public_key: [u8; 32],
    pub expires_unix_secs: u64,
    otp: String,
    attempts: u8,
    used: bool,
}

impl PairOffer {
    pub fn new(
        host: String,
        port: u16,
        public_key: [u8; 32],
        expires_unix_secs: u64,
    ) -> io::Result<Self> {
        validate_host(&host)?;
        if port == 0 || expires_unix_secs <= now()? {
            return Err(invalid("invalid pairing offer"));
        }
        let mut rng = DefaultResolver
            .resolve_rng()
            .ok_or_else(|| io::Error::other("random source unavailable"))?;
        let code = loop {
            let mut bytes = [0_u8; 4];
            rng.try_fill_bytes(&mut bytes).map_err(io::Error::other)?;
            let value = u32::from_le_bytes(bytes);
            if value < 4_294_000_000 {
                break value % 1_000_000;
            }
        };
        Ok(Self {
            host,
            port,
            public_key,
            expires_unix_secs,
            otp: format!("{code:06}"),
            attempts: 0,
            used: false,
        })
    }

    pub fn otp(&self) -> &str {
        &self.otp
    }

    pub fn to_uri(&self) -> String {
        let key = STANDARD
            .encode(self.public_key)
            .replace('+', "%2B")
            .replace('/', "%2F")
            .replace('=', "%3D");
        format!(
            "hushpen-pair://v1?host={}&port={}&pk={key}&otp={}&exp={}",
            self.host, self.port, self.otp, self.expires_unix_secs,
        )
    }

    pub fn parse(uri: &str) -> io::Result<Self> {
        if uri.len() > 1024 {
            return Err(invalid("pairing offer is too long"));
        }
        let query = uri
            .strip_prefix("hushpen-pair://v1?")
            .ok_or_else(|| invalid("invalid pairing version"))?;
        let mut fields = BTreeMap::new();
        for field in query.split('&') {
            let (name, value) = field
                .split_once('=')
                .ok_or_else(|| invalid("invalid pairing field"))?;
            if !["host", "port", "pk", "otp", "exp"].contains(&name)
                || fields.insert(name, decode(value)?).is_some()
            {
                return Err(invalid("invalid pairing field"));
            }
        }
        let field = |name| {
            fields
                .get(name)
                .map(String::as_str)
                .ok_or_else(|| invalid("missing pairing field"))
        };
        let host = field("host")?.to_owned();
        validate_host(&host)?;
        let port = field("port")?
            .parse::<u16>()
            .map_err(|_| invalid("invalid pairing port"))?;
        let public_key: [u8; 32] = STANDARD
            .decode(field("pk")?)
            .map_err(|_| invalid("invalid pairing key"))?
            .try_into()
            .map_err(|_| invalid("invalid pairing key"))?;
        let otp = field("otp")?.to_owned();
        let expires_unix_secs = field("exp")?
            .parse::<u64>()
            .map_err(|_| invalid("invalid pairing expiry"))?;
        if port == 0 || otp.len() != 6 || !otp.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid("invalid pairing offer"));
        }
        Ok(Self {
            host,
            port,
            public_key,
            expires_unix_secs,
            otp,
            attempts: 0,
            used: false,
        })
    }

    pub(crate) fn check(&self) -> io::Result<()> {
        if self.used || self.attempts >= 3 || now()? >= self.expires_unix_secs {
            return Err(denied());
        }
        Ok(())
    }

    pub(crate) fn consume(&mut self, payload: &[u8]) -> io::Result<()> {
        self.check()?;
        self.attempts += 1;
        let mut different = payload.len() ^ self.otp.len();
        for (index, byte) in self.otp.bytes().enumerate() {
            different |= usize::from(byte ^ payload.get(index).copied().unwrap_or(0));
        }
        if different != 0 {
            return Err(denied());
        }
        self.used = true;
        Ok(())
    }
}

fn validate_host(host: &str) -> io::Result<()> {
    if host.is_empty()
        || host.len() > 253
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".:-".contains(&byte))
    {
        return Err(invalid("invalid pairing host"));
    }
    Ok(())
}

fn decode(text: &str) -> io::Result<String> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(|| invalid("invalid pairing escape"))?;
            let low = bytes
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(|| invalid("invalid pairing escape"))?;
            out.push((high * 16 + low) as u8);
        } else {
            out.push(byte);
        }
    }
    String::from_utf8(out).map_err(|_| invalid("invalid pairing text"))
}

pub(crate) fn now() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_uri_round_trips_and_rejects_bad_fields() {
        let offer =
            PairOffer::new("pixel.local".into(), 7342, [255; 32], now().unwrap() + 60).unwrap();
        let uri = offer.to_uri();
        let parsed = PairOffer::parse(&uri).unwrap();
        assert_eq!(parsed.host, offer.host);
        assert_eq!(parsed.public_key, offer.public_key);
        assert_eq!(parsed.otp(), offer.otp());
        for invalid in [
            uri.replace("v1?", "v2?"),
            format!("{uri}&otp=123456"),
            uri.replace("7342", "0"),
            uri.replace("pixel.local", "%26bad"),
            uri.replace("pk=", "pk=%zz"),
            uri.replace("pk=", "pk=x"),
            uri.replace("otp=", "otp=x"),
            format!("{uri}&unknown=x"),
        ] {
            assert!(PairOffer::parse(&invalid).is_err());
        }
    }

    #[test]
    fn offers_expire_are_single_use_and_limit_failed_codes() {
        let mut offer = PairOffer::new("host".into(), 1, [1; 32], now().unwrap() + 60).unwrap();
        let code = offer.otp().as_bytes().to_vec();
        offer.consume(&code).unwrap();
        assert!(offer.consume(&code).is_err());
        let mut offer = PairOffer::new("host".into(), 1, [1; 32], now().unwrap() + 60).unwrap();
        for _ in 0..3 {
            assert!(offer.consume(b"wrong").is_err());
        }
        let code = offer.otp().as_bytes().to_vec();
        assert!(offer.consume(&code).is_err());
        offer.expires_unix_secs = 0;
        assert!(offer.check().is_err());
    }
}
