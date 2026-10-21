//! Whether something on this desktop can show a tray icon.
//!
//! On Linux the tray is a StatusNotifier item, and it shows only while a process owns
//! `org.kde.StatusNotifierWatcher` on the session bus. GNOME without an extension has none.
//! macOS always has a menu bar.

/// True when a tray icon can show. The answer is read from the bus each time, because a watcher
/// can start or stop while the app runs.
pub fn available() -> bool {
    imp::available()
}

#[cfg(target_os = "linux")]
mod imp {
    use zbus::blocking::{Connection, fdo::DBusProxy};
    use zbus::names::BusName;

    const WATCHER: &str = "org.kde.StatusNotifierWatcher";

    pub(super) fn available() -> bool {
        has_watcher().unwrap_or_else(|error| {
            log::debug!("the session bus could not be asked about the tray: {error}");
            false
        })
    }

    fn has_watcher() -> zbus::Result<bool> {
        let connection = Connection::session()?;
        let bus = DBusProxy::new(&connection)?;
        let name = BusName::try_from(WATCHER)?;
        Ok(bus.name_has_owner(name)?)
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub(super) fn available() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn the_menu_bar_always_has_room_for_the_icon() {
        assert!(super::available());
    }
}
