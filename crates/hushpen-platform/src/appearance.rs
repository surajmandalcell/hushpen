//! The desktop color scheme, where the desktop states one.
//!
//! macOS always has an appearance, and GPUI reports it, so this module has nothing to add there.
//! On Linux the answer comes from the settings portal (`org.freedesktop.appearance`). GPUI turns
//! "no portal" and "no preference" into Light, which would paint a Light window on a desktop that
//! never asked for one. This module keeps the two apart: `None` means the desktop did not say.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Dark,
    Light,
}

/// What the desktop prefers, or `None` when it does not say (no session bus, no portal, or "no
/// preference"). The answer is read from the bus each time, because the user can change it while
/// the app runs. It waits half a second at most for a portal that does not answer.
pub fn desktop_scheme() -> Option<Scheme> {
    imp::desktop_scheme()
}

/// The portal's `color-scheme` value: 1 is dark, 2 is light, 0 is no preference.
pub fn scheme_of(value: u32) -> Option<Scheme> {
    match value {
        1 => Some(Scheme::Dark),
        2 => Some(Scheme::Light),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{Scheme, scheme_of};
    use std::sync::mpsc;
    use std::time::Duration;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedValue, Value};

    const WAIT: Duration = Duration::from_millis(500);

    pub(super) fn desktop_scheme() -> Option<Scheme> {
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(read());
        });
        match receive.recv_timeout(WAIT) {
            Ok(Ok(scheme)) => scheme,
            Ok(Err(error)) => {
                log::debug!("the desktop color scheme could not be read: {error}");
                None
            }
            Err(_) => {
                log::debug!("the settings portal did not answer in time");
                None
            }
        }
    }

    fn read() -> zbus::Result<Option<Scheme>> {
        let connection = Connection::session()?;
        let proxy = Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )?;
        let value: OwnedValue =
            proxy.call("Read", &("org.freedesktop.appearance", "color-scheme"))?;
        // `Read` wraps the value in a variant, and some portals wrap it twice.
        let mut inner: &Value = &value;
        while let Value::Value(next) = inner {
            inner = next;
        }
        Ok(match inner {
            Value::U32(number) => scheme_of(*number),
            _ => None,
        })
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::Scheme;

    pub(super) fn desktop_scheme() -> Option<Scheme> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portal_numbers_map_to_schemes() {
        assert_eq!(scheme_of(1), Some(Scheme::Dark));
        assert_eq!(scheme_of(2), Some(Scheme::Light));
        assert_eq!(scheme_of(0), None);
        assert_eq!(scheme_of(7), None);
    }
}
