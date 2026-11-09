//! Stamps the commit this binary is built from, and the date of that commit, for About.
//! `HUSHPEN_BUILD_COMMIT` and `HUSHPEN_BUILD_DATE` win over git; a source tree with no git
//! leaves both empty and About shows the version alone. The commit date keeps two builds of one
//! commit byte for byte the same.

use std::path::Path;
use std::process::Command;

const COMMIT: &str = "HUSHPEN_BUILD_COMMIT";
const DATE: &str = "HUSHPEN_BUILD_DATE";

#[cfg(windows)]
#[path = "../../packaging/windows/icon.rs"]
mod icon;

#[cfg(windows)]
fn windows_resource(dir: &str) -> Result<(), String> {
    let assets = Path::new(dir).join("../../assets");
    let output = std::env::var_os("OUT_DIR").ok_or("OUT_DIR is missing")?;
    let output = Path::new(&output);
    let icon_path = output.join("Hushpen.ico");
    icon::write(&assets, &icon_path)?;
    let version =
        set("HUSHPEN_BUILD_VERSION").unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    let numbers = version
        .split('-')
        .next()
        .ok_or("version is empty")?
        .split('.')
        .map(str::parse::<u16>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("invalid Windows version: {e}"))?;
    if numbers.len() != 3 {
        return Err("Windows version must have three numbers".into());
    }
    let rc = output.join("Hushpen.rc");
    let icon = format!("{:?}", icon_path.to_string_lossy());
    let resource = format!(
        "#include <windows.h>\n1 ICON {icon}\n1 VERSIONINFO\nFILEVERSION {major},{minor},{patch},0\nPRODUCTVERSION {major},{minor},{patch},0\nFILEOS VOS_NT_WINDOWS32\nFILETYPE VFT_APP\nBEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904b0\"\n  BEGIN\n   VALUE \"CompanyName\", \"Suraj Mandal\\0\"\n   VALUE \"FileDescription\", \"Hushpen\\0\"\n   VALUE \"FileVersion\", \"{version}\\0\"\n   VALUE \"ProductName\", \"Hushpen\\0\"\n   VALUE \"ProductVersion\", \"{version}\\0\"\n   VALUE \"OriginalFilename\", \"hushpen.exe\\0\"\n   VALUE \"LegalCopyright\", \"Copyright (c) 2026 Suraj Mandal\\0\"\n  END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n  VALUE \"Translation\", 0x409, 1200\n END\nEND\n",
        major = numbers[0],
        minor = numbers[1],
        patch = numbers[2],
    );
    std::fs::write(&rc, resource).map_err(|e| format!("cannot write version resource: {e}"))?;
    embed_resource::compile_for(&rc, ["hushpen"], embed_resource::NONE)
        .manifest_required()
        .map_err(|e| format!("cannot compile Windows resource: {e:?}"))?;
    println!(
        "cargo:rerun-if-changed={}",
        assets.join("linux-icons").display()
    );
    println!("cargo:rerun-if-changed={dir}/../../packaging/windows/icon.rs");
    Ok(())
}

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
    println!("cargo:rerun-if-env-changed=HUSHPEN_BUILD_VERSION");
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_owned());
    #[cfg(windows)]
    if let Err(error) = windows_resource(&dir) {
        panic!("{error}");
    }
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
