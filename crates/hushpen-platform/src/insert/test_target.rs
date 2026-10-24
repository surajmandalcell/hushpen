//! Test builds only: the bundle id of an app that gets the paste, copy, and undo keys in place of
//! the app in front. A test can then dictate into a background window and leave the focus where
//! it is. The keys go to that app's process; no app is activated.

#[cfg(any(target_os = "macos", test))]
use super::Target;
use std::sync::Mutex;

struct Override {
    bundle: Option<String>,
    /// A missing app is logged once for each bundle id that is set.
    warned: bool,
}

static OVERRIDE: Mutex<Override> = Mutex::new(Override {
    bundle: None,
    warned: false,
});

fn lock() -> std::sync::MutexGuard<'static, Override> {
    OVERRIDE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Sets the bundle id, or clears it with `None` or an empty string.
pub fn set(bundle: Option<&str>) {
    let mut slot = lock();
    slot.bundle = bundle
        .map(str::trim)
        .filter(|bundle| !bundle.is_empty())
        .map(str::to_owned);
    slot.warned = false;
}

/// The bundle id that is set, if any.
pub fn bundle() -> Option<String> {
    lock().bundle.clone()
}

/// The process id of a running app with this bundle id.
pub fn running_pid(bundle: &str) -> Option<u32> {
    #[cfg(target_os = "macos")]
    {
        super::macos::running(bundle)
            .and_then(|target| target.window)
            .and_then(|pid| u32::try_from(pid).ok())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        None
    }
}

#[cfg(any(target_os = "macos", test))]
fn warn_once(bundle: &str) {
    let mut slot = lock();
    if slot.warned || slot.bundle.as_deref() != Some(bundle) {
        return;
    }
    slot.warned = true;
    drop(slot);
    log::warn!("insert target {bundle} is not running, so the app in front gets the text");
}

/// The app that gets the text: the running app of the bundle id that is set, else the app in
/// front.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn choose(
    bundle: Option<&str>,
    running: impl FnOnce(&str) -> Option<Target>,
    frontmost: impl FnOnce() -> Target,
) -> Target {
    let Some(bundle) = bundle else {
        return frontmost();
    };
    match running(bundle) {
        Some(target) => target,
        None => {
            warn_once(bundle);
            frontmost()
        }
    }
}

/// The process that gets the key events for `target`: its pid when it is the set app, else
/// `None`, which posts them to the whole session like a key press.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn key_pid(bundle: Option<&str>, target: &Target) -> Option<i32> {
    let bundle = bundle?;
    if target.label != bundle {
        return None;
    }
    target.window.and_then(|pid| i32::try_from(pid).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(pid: u64, bundle: &str) -> Target {
        Target {
            window: Some(pid),
            label: bundle.into(),
            classes: vec![bundle.into()],
            own: false,
        }
    }

    #[test]
    fn with_no_bundle_the_app_in_front_gets_the_text() {
        let target = choose(
            None,
            |_| panic!("nothing to look up"),
            || app(1, "com.apple.Safari"),
        );
        assert_eq!(target, app(1, "com.apple.Safari"));
    }

    #[test]
    fn a_running_set_app_gets_the_text_in_place_of_the_app_in_front() {
        let target = choose(
            Some("com.apple.TextEdit"),
            |bundle| Some(app(42, bundle)),
            || app(1, "com.apple.Safari"),
        );
        assert_eq!(target, app(42, "com.apple.TextEdit"));
    }

    #[test]
    fn a_set_app_that_is_not_running_leaves_the_app_in_front() {
        let target = choose(
            Some("com.example.Gone"),
            |_| None,
            || app(1, "com.apple.Safari"),
        );
        assert_eq!(target, app(1, "com.apple.Safari"));
    }

    #[test]
    fn keys_go_to_the_pid_of_the_set_app_only() {
        let edit = app(42, "com.apple.TextEdit");
        assert_eq!(key_pid(Some("com.apple.TextEdit"), &edit), Some(42));
        assert_eq!(key_pid(None, &edit), None);
        assert_eq!(key_pid(Some("com.apple.Safari"), &edit), None);
        assert_eq!(
            key_pid(Some("com.apple.TextEdit"), &Target::default()),
            None
        );
    }

    #[test]
    fn the_bundle_is_trimmed_and_an_empty_one_clears_it() {
        set(Some("  com.apple.TextEdit "));
        assert_eq!(bundle().as_deref(), Some("com.apple.TextEdit"));
        set(Some(""));
        assert_eq!(bundle(), None);
        set(Some("com.apple.TextEdit"));
        set(None);
        assert_eq!(bundle(), None);
    }
}
