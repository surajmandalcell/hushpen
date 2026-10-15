//! macOS aborts the process when a main-queue-only framework call runs on another thread
//! ("BUG IN CLIENT OF LIBDISPATCH: Block was expected to execute on queue main-thread").
//! The key tap thread, the insert worker, and every other std thread must never reach one, so
//! these checks read the sources: the Text Input Sources calls live in one file, and the AppKit
//! classes that need the main thread are only used in files that take a `MainThreadMarker`.

use std::fs;
use std::path::{Path, PathBuf};

const TIS_HOME: &str = "src/keyboard_layout.rs";

/// Every `.rs` file under `dir`, as (path relative to the crate, text).
fn sources(dir: &Path, root: &Path, found: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, root, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            found.push((name, fs::read_to_string(&path).unwrap()));
        }
    }
}

fn platform_sources() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    sources(&root.join("src"), &root, &mut found);
    found
}

/// Words are joined so this file does not match itself.
fn tis_prefix() -> String {
    ["TIS", "Copy"].concat()
}

#[test]
fn text_input_source_calls_live_in_one_file() {
    let names = [
        ["TIS", "Copy"].concat(),
        ["TIS", "Get"].concat(),
        ["TIS", "Select"].concat(),
        ["TIS", "Create"].concat(),
        ["TIS", "Set"].concat(),
        ["kTIS", "Property"].concat(),
    ];
    let outside: Vec<String> = platform_sources()
        .into_iter()
        .filter(|(name, _)| name != TIS_HOME)
        .filter(|(_, text)| names.iter().any(|call| text.contains(call.as_str())))
        .map(|(name, _)| name)
        .collect();
    assert!(
        outside.is_empty(),
        "TIS calls outside {TIS_HOME} can run on a worker thread: {outside:?}"
    );
}

#[test]
fn the_one_text_input_source_read_checks_for_the_main_thread() {
    let home = platform_sources()
        .into_iter()
        .find(|(name, _)| name == TIS_HOME)
        .map(|(_, text)| text);
    let Some(home) = home else {
        if cfg!(target_os = "macos") {
            panic!("{TIS_HOME} holds the layout read on macOS");
        }
        return;
    };
    let callers: Vec<&str> = home
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains(&format!("{}Current", tis_prefix())) && line.contains("()"))
        .collect();
    assert_eq!(
        callers.len(),
        2,
        "one declaration and one call: {callers:?}"
    );
    let body = &home[home.find("fn read_system").expect("fn read_system")..];
    let guard = body
        .find("assert_main_thread(")
        .expect("assert_main_thread");
    let tis_call = body.find(&tis_prefix()).expect("the read calls TIS");
    assert!(
        guard < tis_call,
        "read_system asserts the main thread before its first TIS call"
    );
}

#[test]
fn appkit_classes_that_need_the_main_thread_are_used_only_with_a_main_thread_marker() {
    let classes = [
        "NSApplication",
        "NSWindow",
        "NSView",
        "NSScreen",
        "NSStatusBar",
        "NSStatusItem",
    ];
    let unguarded: Vec<String> = platform_sources()
        .into_iter()
        .filter(|(_, text)| {
            text.contains("objc2_app_kit") && classes.iter().any(|class| text.contains(class))
        })
        .filter(|(_, text)| !text.contains("MainThreadMarker"))
        .map(|(name, _)| name)
        .collect();
    assert!(
        unguarded.is_empty(),
        "main-thread AppKit classes without a MainThreadMarker check: {unguarded:?}"
    );
}
