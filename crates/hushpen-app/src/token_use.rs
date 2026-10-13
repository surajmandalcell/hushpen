//! The few-colors rule, enforced on the view code: the accent token reaches the screen only
//! through the default button (and the link-style plain button), a switch that is on, the
//! focus ring, and the recorder that waits for keys; the status tokens reach it only through
//! a status word.

use std::fs;
use std::path::{Path, PathBuf};

/// Where a token may be used: a file under `src`, the function that holds the use, the token.
/// A use inside `focus_visible(..)` is a focus ring and is allowed anywhere.
const ALLOWED: [(&str, &str, &str); 14] = [
    // The default button fills with it; the plain button is a link and takes it for text.
    ("ui.rs", "button", "accent"),
    // The track of a switch that is on.
    ("ui.rs", "switch", "accent"),
    // The recorder that waits for keys has the focus ring while it waits.
    ("shortcuts/panel.rs", "shortcut_row", "accent"),
    // The switch of the updates step.
    ("onboarding/panel.rs", "updates", "accent"),
    // Status words: Ready, Needs your help, Active, Failed.
    ("ui.rs", "status_row", "error"),
    ("dictation/panel.rs", "status_tone", "success"),
    ("dictation/panel.rs", "status_tone", "warning"),
    ("dictation/panel.rs", "status_tone", "error"),
    ("models/panel.rs", "status", "success"),
    ("models/panel.rs", "status", "error"),
    ("onboarding/panel.rs", "permission_row", "success"),
    ("onboarding/panel.rs", "permission_row", "warning"),
    ("onboarding/panel.rs", "model", "success"),
    ("onboarding/panel.rs", "model", "error"),
];

const TOKENS: [&str; 4] = ["accent", "success", "warning", "error"];

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read the source folder").flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" {
                sources(&path, found);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && name != "theme.rs" {
            found.push(path);
        }
    }
}

fn function_name(line: &str) -> Option<String> {
    let rest = line.trim_start();
    let rest = rest.strip_prefix("pub ").unwrap_or(rest);
    let rest = rest
        .strip_prefix("(crate) ")
        .map_or(rest, |_| &rest["(crate) ".len()..]);
    let rest = rest.strip_prefix("fn ")?;
    Some(
        rest.chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect(),
    )
}

/// Every use of a palette token in the view code as `(file, function, token, line)`.
fn uses() -> Vec<(String, String, &'static str, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let mut found = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&root)
            .expect("under src")
            .to_string_lossy()
            .into_owned();
        let text = fs::read_to_string(&file).expect("read a source file");
        let mut current = String::new();
        for line in text.lines() {
            if let Some(name) = function_name(line) {
                current = name;
            }
            let code = line.split("//").next().unwrap_or("");
            for token in TOKENS {
                if code.contains(&format!("palette.{token}")) {
                    found.push((relative.clone(), current.clone(), token, line.to_owned()));
                }
            }
        }
    }
    found
}

fn allowed(file: &str, function: &str, token: &str) -> bool {
    ALLOWED
        .iter()
        .any(|(f, n, t)| *f == file && *n == function && *t == token)
}

#[test]
fn the_accent_and_status_colors_reach_the_screen_only_through_the_allowed_parts() {
    let found = uses();
    assert!(found.len() > 15, "the scan found only {} uses", found.len());
    let stray: Vec<String> = found
        .iter()
        .filter(|(file, function, token, line)| {
            let focus_ring = *token == "accent" && line.contains("focus_visible(");
            !focus_ring && !allowed(file, function, token)
        })
        .map(|(file, function, token, line)| {
            format!("{file} {function}: palette.{token} in `{}`", line.trim())
        })
        .collect();
    assert!(
        stray.is_empty(),
        "a color token is used outside its allowed parts:\n{}",
        stray.join("\n")
    );
}

#[test]
fn every_allowed_use_still_exists() {
    let found = uses();
    for (file, function, token) in ALLOWED {
        assert!(
            found
                .iter()
                .any(|(f, n, t, _)| f == file && n == function && *t == token),
            "{file} {function} no longer uses palette.{token}; remove it from the allowed list"
        );
    }
}

#[test]
fn no_view_names_a_status_color_by_hex() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    for file in files {
        let relative = file.strip_prefix(&root).expect("under src");
        if relative.starts_with("flow_bar") || relative.ends_with("token_use.rs") {
            continue;
        }
        let text = fs::read_to_string(&file).expect("read a source file");
        for color in [
            "30D158", "34C759", "FF9F0A", "FF9500", "FF453A", "FF3B30", "0A84FF", "007AFF",
        ] {
            assert!(
                !text.to_uppercase().contains(color),
                "{} holds the color {color}; read it from the palette",
                relative.display()
            );
        }
    }
}
