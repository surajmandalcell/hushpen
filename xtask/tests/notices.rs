//! The shipped THIRD-PARTY-NOTICES.html: the manual sections are there, and every crate the two
//! binaries link is listed with its version.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn notices() -> String {
    let path = root().join("assets/THIRD-PARTY-NOTICES.html");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

const SECTIONS: [(&str, &str); 10] = [
    ("whisper-cpp", "whisper.cpp"),
    ("llama-cpp", "llama.cpp"),
    ("ggml", "ggml"),
    ("gpui", "GPUI"),
    ("gpui-kit", "GPUI Kit"),
    ("symphonia", "symphonia"),
    ("sqlite", "SQLite"),
    ("inter", "Inter"),
    ("whisper-models", "Whisper models"),
    ("llm-models", "LLM models"),
];

#[test]
fn the_notices_hold_the_ten_manual_sections() {
    let html = notices();
    for (id, title) in SECTIONS {
        assert!(
            html.contains(&format!("<section class=\"manual\" id=\"{id}\">")),
            "section {id} is missing"
        );
        assert!(
            html.contains(&format!("<h3>{title}")),
            "heading {title} is missing"
        );
    }
}

#[test]
fn the_notices_carry_the_license_texts_the_sections_name() {
    let html = notices();
    for needle in [
        "Mozilla Public License Version 2.0",
        "SIL OPEN FONT LICENSE Version 1.1",
        "Apache License",
        "MIT License",
        "May you do good and not evil",
        "Gemma 4 E2B",
        "Qwen3.5 2B",
    ] {
        assert!(html.contains(needle), "the notices lack {needle:?}");
    }
    assert!(
        !html.contains("replaced by the full list"),
        "the placeholder page is still in place"
    );
}

/// `name version` of every third-party crate in the normal dependency tree of the two binaries,
/// for the three release targets.
fn linked_crates() -> BTreeSet<String> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root())
        .args(["tree", "--locked", "--offline", "-e", "normal"])
        .args(["-p", "hushpen-app", "-p", "hushpen-llm"])
        .args(["--prefix", "none", "--format", "{p}"])
        .args(["--target", "aarch64-apple-darwin"])
        .args(["--target", "x86_64-unknown-linux-gnu"])
        .args(["--target", "aarch64-unknown-linux-gnu"])
        .output()
        .expect("cargo tree did not start");
    assert!(output.status.success(), "cargo tree failed");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (name, version) = (words.next()?, words.next()?);
            let version = version.strip_prefix('v')?;
            (!name.starts_with("hushpen-")).then(|| format!("{name} {version}"))
        })
        .collect()
}

#[test]
fn every_linked_crate_is_listed_with_its_version() {
    let html = notices();
    let missing: Vec<String> = linked_crates()
        .into_iter()
        .filter(|krate| !html.contains(&format!("<li>{krate}")))
        .collect();
    assert!(
        missing.is_empty(),
        "crates missing from the notices: {missing:?}"
    );
}
