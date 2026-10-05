//! Where the hook listens: a Unix socket, or TCP on loopback inside a container.

use std::fmt;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

/// The only ports the hook may use.
pub const TCP_PORTS: RangeInclusive<u16> = 4340..=4359;

const SOCKET_ENV: &str = "HUSHPEN_TESTHOOK_SOCKET";
const TCP_ENV: &str = "HUSHPEN_TESTHOOK_TCP";
/// `sun_path` holds about 104 bytes on macOS and 108 on Linux.
const SOCKET_PATH_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    Unix(PathBuf),
    /// Always bound to 127.0.0.1.
    Tcp(u16),
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Endpoint::Unix(path) => write!(f, "unix:{}", path.display()),
            Endpoint::Tcp(port) => write!(f, "tcp:127.0.0.1:{port}"),
        }
    }
}

pub fn default_socket(data_dir: &Path) -> PathBuf {
    data_dir.join("run").join("hook.sock")
}

/// Picks the endpoint. TCP wins when `tcp` is set, and then it must be a port
/// in [`TCP_PORTS`] inside a container; anything else is an error, never a
/// quiet fallback. Otherwise the socket variable, then `<data>/run/hook.sock`.
pub fn resolve(
    socket: Option<&str>,
    tcp: Option<&str>,
    in_container: bool,
    data_dir: Option<&Path>,
) -> Result<Endpoint, String> {
    if let Some(tcp) = tcp.filter(|value| !value.is_empty()) {
        if !in_container {
            return Err(format!("{TCP_ENV} is only honored inside a container"));
        }
        let port: u16 = tcp
            .parse()
            .map_err(|_| format!("{TCP_ENV}={tcp} is not a port number"))?;
        if !TCP_PORTS.contains(&port) {
            return Err(format!(
                "{TCP_ENV}={port} is outside {}-{}",
                TCP_PORTS.start(),
                TCP_PORTS.end()
            ));
        }
        return Ok(Endpoint::Tcp(port));
    }
    let path = match socket.filter(|value| !value.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => default_socket(data_dir.ok_or_else(|| {
            format!("set {SOCKET_ENV}, {TCP_ENV}, or HUSHPEN_DATA_DIR to find the hook")
        })?),
    };
    if path.as_os_str().len() > SOCKET_PATH_LIMIT {
        return Err(format!(
            "socket path {} is longer than {SOCKET_PATH_LIMIT} bytes",
            path.display()
        ));
    }
    Ok(Endpoint::Unix(path))
}

pub fn in_container() -> bool {
    Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists()
}

pub fn resolve_from_env(data_dir: Option<&Path>) -> Result<Endpoint, String> {
    resolve(
        std::env::var(SOCKET_ENV).ok().as_deref(),
        std::env::var(TCP_ENV).ok().as_deref(),
        in_container(),
        data_dir,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Option<&'static Path> {
        Some(Path::new("/data"))
    }

    #[test]
    fn the_default_is_a_socket_in_the_data_folder() {
        assert_eq!(
            resolve(None, None, false, data()),
            Ok(Endpoint::Unix(PathBuf::from("/data/run/hook.sock")))
        );
    }

    #[test]
    fn the_socket_variable_overrides_the_data_folder() {
        assert_eq!(
            resolve(Some("/run/hook.sock"), None, true, data()),
            Ok(Endpoint::Unix(PathBuf::from("/run/hook.sock")))
        );
    }

    #[test]
    fn tcp_needs_a_container_and_a_port_in_range() {
        assert_eq!(
            resolve(None, Some("4340"), true, data()),
            Ok(Endpoint::Tcp(4340))
        );
        assert_eq!(
            resolve(None, Some("4359"), true, data()),
            Ok(Endpoint::Tcp(4359))
        );
        assert!(resolve(None, Some("4340"), false, data()).is_err());
        assert!(resolve(None, Some("4339"), true, data()).is_err());
        assert!(resolve(None, Some("4360"), true, data()).is_err());
        assert!(resolve(None, Some("59559"), true, data()).is_err());
        assert!(resolve(None, Some("abc"), true, data()).is_err());
    }

    #[test]
    fn tcp_never_falls_back_to_the_socket_when_refused() {
        assert!(resolve(Some("/run/hook.sock"), Some("80"), true, data()).is_err());
    }

    #[test]
    fn empty_variables_count_as_unset() {
        assert_eq!(
            resolve(Some(""), Some(""), false, data()),
            Ok(Endpoint::Unix(PathBuf::from("/data/run/hook.sock")))
        );
    }

    #[test]
    fn nothing_to_go_on_is_an_error() {
        assert!(resolve(None, None, false, None).is_err());
    }

    #[test]
    fn a_socket_path_that_cannot_bind_is_refused() {
        let long = format!("/{}", "a".repeat(120));
        assert!(resolve(Some(&long), None, false, data()).is_err());
    }

    #[test]
    fn endpoints_print_where_they_listen() {
        assert_eq!(Endpoint::Tcp(4340).to_string(), "tcp:127.0.0.1:4340");
        assert_eq!(
            Endpoint::Unix("/x/hook.sock".into()).to_string(),
            "unix:/x/hook.sock"
        );
    }
}
