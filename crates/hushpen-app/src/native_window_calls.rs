//! Native window calls never run inside an update of the App. AppKit reports an order front, an
//! order out, a miniaturize, an activation, or a move through callbacks that update the App
//! again, and that fails with `RefCell already borrowed` while the App is borrowed. These checks
//! read the sources and name every call site:
//!
//! - `main_window.rs`: every native call sits in a function that hands it to `after_update`.
//! - `flow_bar/window.rs`: `place_overlay` and `pin_overlay` sit in a `move ||` closure that the
//!   flow bar loop runs after the update.
//! - `window_controls.rs`: the Linux-only minimize circle, an X11 message that no callback
//!   answers, and the only other native window call.
//! - Everywhere else: no direct call.

use std::fs;
use std::path::{Path, PathBuf};

/// Calls that reach AppKit (or X11) for a window or for the whole app.
const NATIVE: [&str; 13] = [
    "window::show(",
    "window::hide(",
    "window::minimize(",
    "window::activate_app(",
    "window::hide_app(",
    "native.show()",
    "native.hide()",
    "native.minimize(",
    "native.activate_app()",
    "native.hide_app()",
    ".activate_window(",
    ".minimize_window(",
    "place_overlay(",
];

/// Calls on the App that reach `NSApplication` at once.
const APP_LEVEL: [&str; 3] = ["cx.activate(", "cx.hide(", "cx.hide_other_apps("];

fn sources(dir: &Path, found: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("read the source folder").flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            sources(&path, found);
        } else if name.ends_with(".rs") && name != "tests.rs" && name != "native_window_calls.rs" {
            found.push((relative(&path), production(&path)));
        }
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn relative(path: &Path) -> String {
    path.strip_prefix(root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// The file without its test module.
fn production(path: &Path) -> String {
    let text = fs::read_to_string(path).expect("read a source file");
    let end = text.find("#[cfg(test)]").unwrap_or(text.len());
    text[..end].to_owned()
}

fn all() -> Vec<(String, String)> {
    let mut found = Vec::new();
    sources(&root(), &mut found);
    found
}

fn file(name: &str) -> String {
    all()
        .into_iter()
        .find(|(path, _)| path == name)
        .unwrap_or_else(|| panic!("{name} is a source file"))
        .1
}

/// Each place where `token` is called, with the text of its function up to the call.
fn uses<'a>(text: &'a str, token: &str) -> Vec<&'a str> {
    text.match_indices(token)
        .filter(|(at, _)| {
            // `main_window::show(` is the safe entry point, not the platform call.
            let glued = !token.starts_with('.')
                && text[..*at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let line_start = text[..*at].rfind('\n').map_or(0, |i| i + 1);
            let line = text[line_start..].lines().next().unwrap_or("");
            !glued && !line.trim_start().starts_with("//") && !line.contains("fn ")
        })
        .map(|(at, _)| {
            let start = text[..at]
                .rfind("\nfn ")
                .or_else(|| text[..at].rfind("\npub fn "));
            &text[start.unwrap_or(0)..at]
        })
        .collect()
}

#[test]
fn the_main_window_hands_every_native_call_to_after_update() {
    let text = file("main_window.rs");
    // The platform stand-in holds the calls that `after_update` work makes.
    let start = text.find("impl Native for Platform").expect("the stand-in");
    let end = start + text[start..].find("\n}\n").expect("its end");
    let rest = format!("{}{}", &text[..start], &text[end..]);
    let mut seen = 0;
    for token in NATIVE {
        for before in uses(&rest, token) {
            seen += 1;
            assert!(
                before.contains("after_update("),
                "{token} runs before after_update in:\n{before}"
            );
        }
    }
    assert!(seen >= 6, "the scan found only {seen} native calls");
}

#[test]
fn the_flow_bar_makes_its_native_calls_in_closures_the_loop_runs_later() {
    let text = file("flow_bar/window.rs");
    let mut seen = 0;
    for token in ["place_overlay(", "pin_overlay("] {
        for before in uses(&text, token) {
            seen += 1;
            assert!(
                before.contains("move ||"),
                "{token} is called inside the update in:\n{before}"
            );
        }
    }
    assert_eq!(seen, 2, "one place_overlay and one pin_overlay call");
}

#[test]
fn no_other_file_calls_the_platform_or_the_app_directly() {
    let mut direct = Vec::new();
    for (name, text) in all() {
        if matches!(
            name.as_str(),
            "main_window.rs" | "flow_bar/window.rs" | "window_controls.rs"
        ) {
            continue;
        }
        for token in NATIVE.iter().chain(&APP_LEVEL) {
            if !uses(&text, token).is_empty() {
                direct.push(format!("{name}: {token}"));
            }
        }
    }
    assert!(
        direct.is_empty(),
        "native calls outside the audited files: {direct:?}"
    );
}

#[test]
fn the_app_level_calls_live_nowhere_in_the_app() {
    let found: Vec<String> = all()
        .into_iter()
        .flat_map(|(name, text)| {
            APP_LEVEL
                .iter()
                .filter(|token| !uses(&text, token).is_empty())
                .map(move |token| format!("{name}: {token}"))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        found.is_empty(),
        "use main_window::show or hide_app: {found:?}"
    );
}

#[test]
fn the_linux_minimize_circle_is_the_only_other_native_window_call_and_never_builds_on_macos() {
    let text = file("window_controls.rs");
    assert_eq!(uses(&text, ".minimize_window(").len(), 1);
    let lib = fs::read_to_string(root().join("lib.rs")).unwrap();
    assert!(
        lib.contains("#[cfg(not(target_os = \"macos\"))]\npub mod window_controls;"),
        "window_controls must stay out of the macOS build"
    );
}
