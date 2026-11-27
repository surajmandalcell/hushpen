//! The floating guide beside System Settings: a small panel under the Settings window that
//! follows it and says what to do. Accessibility and Input Monitoring show a card the user drags
//! into the list; Microphone, whose list holds only apps that asked, shows one plain line.
//!
//! The look and the follow rules are a port of the drag panel of PermissionFlow by jaywcjlove
//! (MIT, https://github.com/jaywcjlove/PermissionFlow) and of the guide panel of SamPermissions.

use hushpen_core::permission::Permission;

#[cfg(target_os = "macos")]
mod macos;

/// Shows the guide for the Privacy pane of `pane`, which was just opened. A guide that is still
/// on screen closes first. It reads the Settings window from window-server bounds, so it needs no
/// permission, and it never takes focus. Call it on the main thread. Linux and Windows have none.
pub fn show(pane: Permission) {
    #[cfg(target_os = "macos")]
    macos::show(pane);
    #[cfg(not(target_os = "macos"))]
    let _ = pane;
}

/// The guide for `pane` drawn offscreen at 2x as a PNG, `width` points wide, for the look check.
/// `app` is the bundle it shows. The window-server blur does not exist offscreen, so a flat fill
/// stands in for it.
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub fn snapshot_png(
    pane: Permission,
    dark: bool,
    width: f64,
    app: &std::path::Path,
) -> Option<Vec<u8>> {
    macos::snapshot_png(pane, dark, width, app)
}
