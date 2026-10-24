//! What a click on a permission button does: the request API of the permission, the Privacy
//! pane, and the reopen. The real guide asks macOS; tests pass a fake.

use futures::channel::oneshot;
use hushpen_core::permission::Permission;

/// What a click on a permission button does. The real one asks macOS and opens System
/// Settings; tests pass a fake.
pub trait Guide {
    /// Calls the request API of the permission, which lists Hushpen in the Privacy pane and
    /// shows the system prompt. The Allow button calls it on every click. With `repair` the
    /// record of Hushpen is reset first: the grant was held and is gone, so the record is a
    /// stale one from an older build. Without it a denied row stays in the list. Returns `None`
    /// when the request is over, or a receiver that answers once it is.
    fn request(&self, permission: Permission, repair: bool) -> Option<oneshot::Receiver<()>>;
    fn open(&self, url: &str) -> Result<(), String>;
    /// Starts a new copy of the app once this one has ended. The caller quits right after.
    fn relaunch(&self) -> Result<(), String>;
}

impl Guide for SystemGuide {
    fn request(&self, permission: Permission, repair: bool) -> Option<oneshot::Receiver<()>> {
        let (done, finished) = oneshot::channel();
        let started = on_permission_thread(move || {
            hushpen_platform::permissions::request(permission, repair);
            let _ = done.send(());
        });
        match started {
            Ok(()) => Some(finished),
            Err(error) => {
                log::warn!("the permission request thread did not start: {error}");
                hushpen_platform::permissions::request(permission, repair);
                None
            }
        }
    }

    fn open(&self, url: &str) -> Result<(), String> {
        hushpen_platform::permissions::open_url(url).map_err(|error| error.to_string())
    }

    fn relaunch(&self) -> Result<(), String> {
        hushpen_platform::permissions::relaunch().map_err(|error| error.to_string())
    }
}

pub struct SystemGuide;

/// Runs `job` on its own thread. A permission request first reads the grant and can run
/// `tccutil reset`, which may take up to 2 s, so it never runs on the UI thread.
pub(super) fn on_permission_thread(job: impl FnOnce() + Send + 'static) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("hushpen-permission".into())
        .spawn(job)
        .map(|_| ())
}
