//! Stamps the commit this binary is built from, and the date of that commit, for About.
//! `HUSHPEN_BUILD_COMMIT` and `HUSHPEN_BUILD_DATE` win over git; a source tree with no git
//! leaves both empty and About shows the version alone. The commit date keeps two builds of one
//! commit byte for byte the same.

use std::path::Path;
use std::process::Command;

const COMMIT: &str = "HUSHPEN_BUILD_COMMIT";
const DATE: &str = "HUSHPEN_BUILD_DATE";

fn git(dir: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !text.is_empty()).then_some(text)
}

fn set(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={COMMIT}");
    println!("cargo:rerun-if-env-changed={DATE}");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_owned());
    // A commit moves HEAD or the branch it names, so a new commit runs this again. A missing
    // file would make cargo run it on every build.
    let branch = git(&dir, &["symbolic-ref", "-q", "HEAD"]);
    for name in ["HEAD", "packed-refs"].into_iter().chain(branch.as_deref()) {
        if let Some(path) = git(&dir, &["rev-parse", "--git-path", name]) {
            let path = Path::new(&dir).join(path);
            if path.is_file() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    let commit = set(COMMIT)
        .or_else(|| git(&dir, &["rev-parse", "--short=7", "HEAD"]))
        .unwrap_or_default();
    let date = set(DATE)
        .or_else(|| git(&dir, &["log", "-1", "--format=%cs"]))
        .unwrap_or_default();
    println!("cargo:rustc-env={COMMIT}={commit}");
    println!("cargo:rustc-env={DATE}={date}");
}
