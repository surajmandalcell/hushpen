//! Carry a model host connection through the user's own `ssh -L` when the host port is closed
//! to this computer (D-55). SSH is only the pipe: the pairing key still encrypts and
//! authenticates every byte. No relay and no third-party host.

use std::collections::BTreeMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// SSH logs in and opens the forward within this time, or the start counts as failed.
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// One `ssh -L` child per SSH host alias, started on first use and restarted on the next use
/// after it drops.
#[derive(Default)]
pub struct Tunnels {
    open: Mutex<BTreeMap<String, Tunnel>>,
}

#[derive(Default)]
struct Tunnel {
    running: Option<Running>,
    backoff: Backoff,
}

struct Running {
    child: Child,
    /// Never written: when Hushpen ends, even by a crash, the closed pipe ends the remote `cat`,
    /// and ssh exits with it. macOS has no parent-death signal.
    _stdin: Option<ChildStdin>,
    port: u16,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Restart pacing after a failed start: 1 s, then doubling up to 30 s. A start that works resets
/// it, so a tunnel that drops after hours restarts at once.
#[derive(Default, Debug, PartialEq, Eq)]
struct Backoff {
    failures: u32,
    retry_at: Option<Instant>,
}

impl Backoff {
    fn ready(&self, now: Instant) -> bool {
        self.retry_at.is_none_or(|at| now >= at)
    }

    fn failed(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        let wait = Duration::from_secs(1 << (self.failures - 1).min(5)).min(MAX_BACKOFF);
        self.retry_at = Some(now + wait);
    }

    fn worked(&mut self) {
        *self = Self::default();
    }
}

impl Tunnels {
    /// The local address that reaches `remote_port` on the server of SSH alias `host`. Starts
    /// ssh when no tunnel runs, or restarts it when it dropped and its backoff has passed.
    pub fn address(&self, host: &str, remote_port: u16) -> io::Result<String> {
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        let tunnel = open.entry(host.to_owned()).or_default();
        if let Some(running) = &mut tunnel.running {
            if running.child.try_wait()?.is_none() {
                return Ok(local(running.port));
            }
            log::warn!("SSH_TUNNEL_DROPPED {host}");
            tunnel.running = None;
        }
        let now = Instant::now();
        if !tunnel.backoff.ready(now) {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "The SSH tunnel is waiting before it starts again.",
            ));
        }
        match start(host, remote_port) {
            Ok(running) => {
                tunnel.backoff.worked();
                let address = local(running.port);
                tunnel.running = Some(running);
                Ok(address)
            }
            Err(problem) => {
                tunnel.backoff.failed(now);
                log::warn!("SSH_TUNNEL_FAILED {host} {problem}");
                Err(problem)
            }
        }
    }

    /// Stops the tunnel of a removed device.
    pub fn close(&self, host: &str) {
        self.open
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(host);
    }

    /// Stops every tunnel when the app quits.
    pub fn close_all(&self) {
        self.open.lock().unwrap_or_else(|p| p.into_inner()).clear();
    }
}

fn local(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

fn start(host: &str, remote_port: u16) -> io::Result<Running> {
    // shortcut: the free port can be taken before ssh binds it; ExitOnForwardFailure makes that a
    // failed start, and the next use picks another port.
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port();
    #[cfg(unix)]
    let mut command = Command::new("/usr/bin/ssh");
    #[cfg(not(unix))]
    let mut command = Command::new("ssh");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A Ctrl-C in the terminal that started Hushpen must not end the tunnel by itself.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command
        .args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ExitOnForwardFailure=yes",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
            "-L",
            &format!("127.0.0.1:{port}:127.0.0.1:{remote_port}"),
            "--",
            host,
            // In place of `-N`: reads the stdin pipe, so ssh ends when Hushpen ends.
            "cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    let stdin = child.stdin.take();
    let mut running = Running {
        child,
        _stdin: stdin,
        port,
    };
    // ssh binds the local port only after login and the forward succeed.
    let deadline = Instant::now() + READY_TIMEOUT;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    loop {
        if running.child.try_wait()?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "SSH could not log in or open the tunnel.",
            ));
        }
        if TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok() {
            return Ok(running);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "SSH did not open the tunnel in time.",
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_start_waits_longer_each_time_and_a_working_start_resets_it() {
        let now = Instant::now();
        let mut backoff = Backoff::default();
        assert!(backoff.ready(now));
        let mut waits = Vec::new();
        for _ in 0..8 {
            backoff.failed(now);
            assert!(!backoff.ready(now));
            waits.push(backoff.retry_at.unwrap() - now);
        }
        let seconds: Vec<_> = waits.iter().map(Duration::as_secs).collect();
        assert_eq!(seconds, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert!(backoff.ready(now + MAX_BACKOFF));
        backoff.worked();
        assert_eq!(backoff, Backoff::default());
        assert!(backoff.ready(now));
    }

    #[cfg(unix)]
    #[test]
    fn a_dropped_tunnel_restarts_on_the_next_use_and_close_stops_it() {
        // The alias resolves nowhere, so the restart fails fast and backs off.
        let host = "hushpen-no-such-host.invalid";
        let tunnels = Tunnels::default();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        // The port still answers, but a tunnel whose child ended must not be handed out again.
        let mut ended = Command::new("true").spawn().unwrap();
        ended.wait().unwrap();
        tunnels.open.lock().unwrap().insert(
            host.into(),
            Tunnel {
                running: Some(Running {
                    child: ended,
                    _stdin: None,
                    port,
                }),
                backoff: Backoff::default(),
            },
        );
        assert!(
            tunnels.address(host, port).is_err(),
            "restarted, not reused"
        );
        let open = tunnels.open.lock().unwrap();
        assert!(open[host].running.is_none());
        assert_eq!(open[host].backoff.failures, 1);
        drop(open);
        let started = Instant::now();
        assert!(tunnels.address(host, port).is_err(), "backoff holds");
        assert!(
            started.elapsed() < Duration::from_millis(100),
            "no ssh during backoff"
        );
        tunnels.close(host);
        assert!(tunnels.open.lock().unwrap().is_empty());
    }
}
