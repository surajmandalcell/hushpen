//! A focusable element with an id but no role has no accessibility node, so a screen reader
//! announces the whole window when focus lands on it. Each function that makes an element
//! focusable also gives a role.

use crate::token_use::{function_name, sources};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[test]
fn every_function_that_tracks_focus_gives_a_role() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let mut tracked = 0;
    let mut missing = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&root)
            .expect("under src")
            .to_string_lossy()
            .into_owned();
        if relative.ends_with("focus_roles.rs") {
            continue;
        }
        let text = fs::read_to_string(&file).expect("read a source file");
        let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut current = String::new();
        for line in text.lines() {
            if let Some(name) = function_name(line) {
                current = name;
            }
            let code = line.split("//").next().unwrap_or("");
            let entry = counts.entry(current.clone()).or_default();
            entry.0 += code.matches(".track_focus(").count();
            entry.1 += code.matches(".role(").count();
        }
        for (function, (focus, roles)) in counts {
            tracked += focus;
            if roles < focus {
                missing.push(format!(
                    "{relative} {function}: {focus} focusable, {roles} with a role"
                ));
            }
        }
    }
    assert!(tracked > 10, "the scan found only {tracked} focusable uses");
    assert!(
        missing.is_empty(),
        "a focusable element has no role:\n{}",
        missing.join("\n")
    );
}
