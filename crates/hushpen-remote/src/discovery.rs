//! Local network discovery advertises facts, never trust or private keys.

use crate::invalid;
use hushpen_core::os::Os;
use hushpen_core::remote::{Device, DeviceKind, Serves};
use mdns_sd::{Receiver, ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};
use std::io;
use std::net::{SocketAddr, SocketAddrV6};
use std::time::{Duration, Instant};

pub const SERVICE_TYPE: &str = "_hushpen._tcp.local.";

pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertisement {
    pub fn new(device: &Device, hostname: &str, port: u16, version: &str) -> io::Result<Self> {
        let info = service_info(device, hostname, port, version)?;
        let fullname = info.get_fullname().to_owned();
        let daemon = ServiceDaemon::new().map_err(io::Error::other)?;
        if let Err(error) = daemon.register(info) {
            let _ = daemon.shutdown();
            return Err(io::Error::other(error));
        }
        Ok(Self { daemon, fullname })
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundDevice {
    pub device: Device,
    pub hostname: String,
    pub port: u16,
    pub addresses: Vec<SocketAddr>,
    pub version: String,
}

pub struct Discovery {
    daemon: ServiceDaemon,
    events: Receiver<ServiceEvent>,
}

impl Discovery {
    pub fn new() -> io::Result<Self> {
        let daemon = ServiceDaemon::new().map_err(io::Error::other)?;
        let events = match daemon.browse(SERVICE_TYPE) {
            Ok(events) => events,
            Err(error) => {
                let _ = daemon.shutdown();
                return Err(io::Error::other(error));
            }
        };
        Ok(Self { daemon, events })
    }

    /// Waits at most `timeout` for a resolved, valid device.
    pub fn next(&self, timeout: Duration) -> io::Result<Option<FoundDevice>> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            match self.events.recv_timeout(remaining) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    if let Ok(device) = found(&info) {
                        return Ok(Some(device));
                    }
                }
                Ok(_) => {}
                Err(mdns_sd::RecvTimeoutError::Timeout) => return Ok(None),
                Err(mdns_sd::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::other("discovery stopped"));
                }
            }
        }
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.daemon.stop_browse(SERVICE_TYPE);
        let _ = self.daemon.shutdown();
    }
}

fn service_info(
    device: &Device,
    hostname: &str,
    port: u16,
    version: &str,
) -> io::Result<ServiceInfo> {
    validate_text(&device.id)?;
    validate_text(&device.name)?;
    validate_text(version)?;
    if port == 0 {
        return Err(invalid("invalid discovery port"));
    }
    let serves = match (device.serves.speech, device.serves.cleanup) {
        (true, true) => "speech,cleanup",
        (true, false) => "speech",
        (false, true) => "cleanup",
        (false, false) => "none",
    };
    let os = os_name(device.os);
    let properties = [
        ("id", device.id.as_str()),
        ("name", device.name.as_str()),
        ("os", os),
        ("serves", serves),
        ("version", version),
    ];
    ServiceInfo::new(
        SERVICE_TYPE,
        &device.id,
        hostname,
        "",
        port,
        &properties[..],
    )
    .map(ServiceInfo::enable_addr_auto)
    .map_err(io::Error::other)
}

fn found(info: &ResolvedService) -> io::Result<FoundDevice> {
    let field = |name| {
        info.get_property_val_str(name)
            .ok_or_else(|| invalid("missing discovery field"))
    };
    let id = field("id")?;
    let name = field("name")?;
    let version = field("version")?;
    for text in [id, name, version] {
        validate_text(text)?;
    }
    let os = match field("os")? {
        "macos" => Os::Mac,
        "linux" => Os::Linux,
        "windows" => Os::Windows,
        "ios" => Os::Ios,
        "android" => Os::Android,
        _ => return Err(invalid("unknown discovery OS")),
    };
    let serves = match field("serves")? {
        "speech,cleanup" => Serves {
            speech: true,
            cleanup: true,
        },
        "speech" => Serves {
            speech: true,
            cleanup: false,
        },
        "cleanup" => Serves {
            speech: false,
            cleanup: true,
        },
        "none" => Serves {
            speech: false,
            cleanup: false,
        },
        _ => return Err(invalid("unknown discovery model kind")),
    };
    if !info.is_valid() || info.port == 0 || info.host.len() > 255 {
        return Err(invalid("invalid discovery address"));
    }
    let mut addresses: Vec<_> = info
        .addresses
        .iter()
        .filter_map(|address| match address {
            ScopedIp::V4(ip) => Some(SocketAddr::from((*ip.addr(), info.port))),
            ScopedIp::V6(ip) => Some(SocketAddr::V6(SocketAddrV6::new(
                *ip.addr(),
                info.port,
                0,
                ip.scope_id().index,
            ))),
            _ => None,
        })
        .collect();
    addresses.sort();
    Ok(FoundDevice {
        device: Device {
            id: id.into(),
            name: name.into(),
            kind: match os {
                Os::Ios | Os::Android => DeviceKind::Phone,
                _ => DeviceKind::Server,
            },
            os,
            serves,
        },
        hostname: info.host.clone(),
        port: info.port,
        addresses,
        version: version.into(),
    })
}

fn validate_text(text: &str) -> io::Result<()> {
    if text.is_empty() || text.len() > 128 || text.chars().any(char::is_control) {
        return Err(invalid("invalid discovery text"));
    }
    Ok(())
}

fn os_name(os: Os) -> &'static str {
    match os {
        Os::Mac => "macos",
        Os::Linux => "linux",
        Os::Windows => "windows",
        Os::Ios => "ios",
        Os::Android => "android",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertisements_contain_only_the_public_discovery_fields() {
        let device = Device {
            id: "hushpen-test".into(),
            name: "Test phone".into(),
            kind: DeviceKind::Phone,
            os: Os::Android,
            serves: Serves {
                speech: true,
                cleanup: true,
            },
        };
        let info = service_info(&device, "hushpen-test.local.", 7342, "2.4.0").unwrap();
        assert_eq!(info.get_type(), SERVICE_TYPE);
        assert_eq!(info.get_property_val_str("id"), Some("hushpen-test"));
        assert_eq!(info.get_property_val_str("name"), Some("Test phone"));
        assert_eq!(info.get_property_val_str("os"), Some("android"));
        assert_eq!(info.get_property_val_str("serves"), Some("speech,cleanup"));
        assert_eq!(info.get_property_val_str("version"), Some("2.4.0"));
        assert!(info.get_property_val_str("key").is_none());
        assert!(info.get_property_val_str("otp").is_none());
        let mut resolved = info.as_resolved_service();
        resolved
            .addresses
            .insert("192.168.1.2".parse::<std::net::IpAddr>().unwrap().into());
        let found = found(&resolved).unwrap();
        assert_eq!(found.device, device);
        assert_eq!(
            found.addresses,
            vec!["192.168.1.2:7342".parse::<SocketAddr>().unwrap()]
        );
    }
}
