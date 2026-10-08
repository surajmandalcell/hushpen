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

/// Makes the menu bar icon a template image at menu bar size, so macOS tints it for a light or
/// dark menu bar. The tray library does not mark its image, and it keeps no handle to the status
/// item, so this walks the app's windows for the status bar button. Returns how many buttons it
/// styled; `0` where there is nothing to style.
pub fn style_status_icon() -> usize {
    imp::style_status_icon()
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

    pub(super) fn style_status_icon() -> usize {
        0
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::MainThreadMarker;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSApplication, NSStatusBarButton, NSView};
    use objc2_foundation::NSSize;

    /// Points: the height of the menu bar icons that macOS draws itself.
    const ICON_POINTS: f64 = 18.0;

    pub(super) fn available() -> bool {
        true
    }

    pub(super) fn style_status_icon() -> usize {
        let Some(mtm) = MainThreadMarker::new() else {
            return 0;
        };
        let mut styled = 0;
        for window in NSApplication::sharedApplication(mtm).windows() {
            if let Some(content) = window.contentView() {
                styled += style_in(&content);
            }
        }
        styled
    }

    fn style_in(view: &NSView) -> usize {
        let object: &AnyObject = view;
        let mut styled = 0;
        if let Some(button) = object.downcast_ref::<NSStatusBarButton>()
            && let Some(image) = button.image()
        {
            image.setTemplate(true);
            image.setSize(NSSize::new(ICON_POINTS, ICON_POINTS));
            styled += 1;
        }
        for child in view.subviews() {
            styled += style_in(&child);
        }
        styled
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod imp {
    pub(super) fn available() -> bool {
        true
    }

    pub(super) fn style_status_icon() -> usize {
        0
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
