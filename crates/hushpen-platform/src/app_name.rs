//! The name a person knows an app by, from the id History stores for the app a dictation went to.
//!
//! On macOS the id is a bundle id such as `com.sublimetext.4`: the installed app's
//! CFBundleDisplayName, else its CFBundleName, else its file name without `.app`. Other systems
//! already store a name. An id with no installed app stays as it is.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The display name of `id`, cached for the run, so a list of rows asks the system once per app.
pub fn display_name(id: &str) -> String {
    static NAMES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    let names = NAMES.get_or_init(Default::default);
    if let Some(name) = names.lock().ok().and_then(|names| names.get(id).cloned()) {
        return name;
    }
    let name = imp::lookup(id)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| id.to_owned());
    if let Ok(mut names) = names.lock() {
        names.insert(id.to_owned(), name.clone());
    }
    name
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSBundle, NSString};

    pub(super) fn lookup(id: &str) -> Option<String> {
        let url = NSWorkspace::sharedWorkspace()
            .URLForApplicationWithBundleIdentifier(&NSString::from_str(id))?;
        let from_plist = NSBundle::bundleWithURL(&url).and_then(|bundle| {
            ["CFBundleDisplayName", "CFBundleName"]
                .into_iter()
                .find_map(|key| {
                    let value = bundle.objectForInfoDictionaryKey(&NSString::from_str(key))?;
                    let text = value.downcast::<NSString>().ok()?.to_string();
                    (!text.trim().is_empty()).then_some(text)
                })
        });
        from_plist.or_else(|| {
            let file = url.lastPathComponent()?.to_string();
            Some(file.strip_suffix(".app").unwrap_or(&file).to_owned())
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub(super) fn lookup(_: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::display_name;

    #[test]
    fn an_unknown_id_stays_as_it_is() {
        assert_eq!(display_name("no.such.app.hushpen"), "no.such.app.hushpen");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_bundle_id_becomes_the_app_name() {
        assert_eq!(display_name("com.apple.finder"), "Finder");
    }
}
