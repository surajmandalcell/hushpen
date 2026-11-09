//! A phone returns its serving address over the authenticated pairing stream.

use crate::{RemoteStream, invalid};
use hushpen_core::os::Os;
use hushpen_core::protocol::{self, FrameKind};
use hushpen_core::remote::{Device, DeviceKind, Serves};
use serde_json::{Value, json};
use std::io::{self, Read};
use std::net::IpAddr;

pub struct Enrollment {
    pub device: Device,
    pub port: u16,
}

impl Enrollment {
    pub fn send(&self, stream: &mut RemoteStream) -> io::Result<()> {
        let os = match self.device.os {
            Os::Ios => "ios",
            Os::Android => "android",
            _ => return Err(invalid("only phones can enroll")),
        };
        let value = json!({"type":"enroll", "name":self.device.name, "os":os,
            "port":self.port, "speech":self.device.serves.speech,
            "cleanup":self.device.serves.cleanup});
        Self::parse(&value, &self.device.id)?;
        protocol::write_json(stream, &value).map_err(io::Error::other)
    }

    pub fn receive(stream: &mut impl Read, public_key: &[u8; 32]) -> io::Result<Self> {
        let mut header = [0; 5];
        stream.read_exact(&mut header)?;
        let length = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
        if header[4] != FrameKind::Json as u8 || length > 1024 {
            return Err(invalid("invalid phone details"));
        }
        let mut payload = vec![0; length];
        stream.read_exact(&mut payload)?;
        let value =
            serde_json::from_slice(&payload).map_err(|_| invalid("invalid phone details"))?;
        let id = public_key
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Self::parse(&value, &id)
    }

    fn parse(value: &Value, id: &str) -> io::Result<Self> {
        let name = value["name"]
            .as_str()
            .ok_or_else(|| invalid("missing phone name"))?;
        if value["type"] != "enroll"
            || name.trim().is_empty()
            || name.len() > 128
            || name.chars().any(char::is_control)
        {
            return Err(invalid("invalid phone details"));
        }
        let os = match value["os"].as_str() {
            Some("ios") => Os::Ios,
            Some("android") => Os::Android,
            _ => return Err(invalid("unknown phone system")),
        };
        let port = value["port"]
            .as_u64()
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port != 0)
            .ok_or_else(|| invalid("invalid phone port"))?;
        let flag = |key| {
            value[key]
                .as_bool()
                .ok_or_else(|| invalid("invalid phone capabilities"))
        };
        Ok(Self {
            device: Device {
                id: id.into(),
                name: name.into(),
                kind: DeviceKind::Phone,
                os,
                serves: Serves {
                    speech: flag("speech")?,
                    cleanup: flag("cleanup")?,
                },
            },
            port,
        })
    }

    pub fn address(&self, peer: IpAddr) -> String {
        std::net::SocketAddr::new(peer, self.port).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enrollment_uses_authenticated_key_for_identity_and_peer_for_address() {
        let value = json!({"type":"enroll","id":"untrusted","name":"Phone","os":"ios",
            "port":47321,"speech":true,"cleanup":false});
        let mut bytes = Vec::new();
        protocol::write_json(&mut bytes, &value).unwrap();
        let phone = Enrollment::receive(&mut bytes.as_slice(), &[42; 32]).unwrap();
        assert_eq!(phone.device.id, "2a".repeat(32));
        assert_eq!(phone.address("::1".parse().unwrap()), "[::1]:47321");
        assert!(!phone.device.serves.cleanup);
        for (key, bad) in [
            ("name", json!("\n")),
            ("os", json!("linux")),
            ("port", json!(65536)),
            ("port", json!(0)),
            ("speech", json!("true")),
        ] {
            let mut invalid = value.clone();
            invalid[key] = bad;
            assert!(Enrollment::parse(&invalid, "pin").is_err());
        }
    }

    #[test]
    fn oversized_or_binary_enrollment_is_refused() {
        let mut bytes = Vec::new();
        protocol::write_frame(&mut bytes, FrameKind::Binary, b"{}").unwrap();
        assert!(Enrollment::receive(&mut bytes.as_slice(), &[1; 32]).is_err());
        bytes = (1025u32).to_le_bytes().to_vec();
        bytes.push(0);
        assert!(Enrollment::receive(&mut bytes.as_slice(), &[1; 32]).is_err());
    }
}
