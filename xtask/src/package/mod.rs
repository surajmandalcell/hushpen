//! `cargo xtask package --target <t> --formats <list>`: builds the release
//! binaries and the installers for one target.
//!
//! macOS: `.app` and zip anywhere, dmg on the CI runner only (cargo-packager's
//! dmg step drives Finder). Linux: AppImage, deb, and rpm through
//! cargo-packager and cargo-generate-rpm, tar.gz from here. Every artifact
//! lands in `<target dir>/dist/<target>` with the name from `names.rs`, next to
//! a `SHA256SUMS` file.

mod config;
mod deps;
mod linux;
mod mac;
pub mod names;
mod verify;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use names::{Format, Target};

pub use verify::run_verify;

const APP_BINARY: &str = "hushpen";
const LLM_BINARY: &str = "hushpen-llm";
const NOTICES: &str = "THIRD-PARTY-NOTICES.html";
const USAGE: &str = "usage: cargo xtask package [--target macos-arm64|linux-x64|linux-arm64] \
[--formats app,zip,dmg,appimage,deb,rpm,tar.gz] [--out <dir>] [--skip-build] [--test-feed]";

#[derive(Debug, PartialEq, Eq)]
struct Options {
    target: Option<Target>,
    formats: Option<Vec<Format>>,
    out: Option<PathBuf>,
    skip_build: bool,
    /// Packages the app built with the `update-test-feed` feature, for the self-update tests.
    /// Such a build never ships, so it needs its own `--out` folder.
    test_feed: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        target: None,
        formats: None,
        out: None,
        skip_build: false,
        test_feed: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| iter.next().ok_or(format!("{name} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--target" => options.target = Some(Target::parse(value("--target")?)?),
            "--formats" => {
                let formats = value("--formats")?
                    .split(',')
                    .map(|word| Format::parse(word.trim()))
                    .collect::<Result<Vec<_>, _>>()?;
                options.formats = Some(formats);
            }
            "--out" => options.out = Some(PathBuf::from(value("--out")?)),
            "--skip-build" => options.skip_build = true,
            "--test-feed" => options.test_feed = true,
            other => return Err(format!("unknown package argument {other:?}\n{USAGE}")),
        }
    }
    if options.test_feed && options.out.is_none() {
        return Err(format!(
            "--test-feed needs --out <dir>: a test-feed build never goes in the dist folder\n{USAGE}"
        ));
    }
    Ok(options)
}

/// The dmg step drives Finder and mounts a volume: it runs on a CI runner only.
fn dmg_allowed(ci: Option<&str>, actions: Option<&str>) -> Result<(), String> {
    if ci == Some("true") && actions == Some("true") {
        Ok(())
    } else {
        Err(
            "the dmg is built on the GitHub Actions macOS runner only (its layout step drives \
             Finder); build the app and zip here with --formats app,zip"
                .into(),
        )
    }
}

fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from)
}

/// `HUSHPEN_BUILD_VERSION` names a release candidate; it is also compiled into
/// the binary, so the names and the `--version` line agree.
fn build_version() -> String {
    match std::env::var("HUSHPEN_BUILD_VERSION") {
        Ok(version) if !version.is_empty() => version,
        _ => env!("CARGO_PKG_VERSION").to_string(),
    }
}

pub struct Ctx {
    pub root: PathBuf,
    pub version: String,
    pub target: Target,
    pub dist: PathBuf,
    pub binaries: PathBuf,
}

impl Ctx {
    pub fn notices(&self) -> PathBuf {
        self.root.join("assets").join(NOTICES)
    }

    /// A fresh work folder holding both binaries, which cargo-packager reads
    /// from its out dir (`--binaries-dir` is ignored).
    pub fn work_dir(&self, name: &str) -> Result<PathBuf, String> {
        let dir = self.dist.join("work").join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
        }
        fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        for binary in [APP_BINARY, LLM_BINARY] {
            copy(&self.binaries.join(binary), &dir.join(binary))?;
        }
        Ok(dir)
    }

    /// Resources every package carries: the notices and the install kind.
    pub fn resources(&self, work: &Path, kind: &str) -> Result<Vec<(PathBuf, String)>, String> {
        if !self.notices().is_file() {
            return Err(format!("{} is missing", self.notices().display()));
        }
        let install_kind = work.join("install-kind");
        fs::write(&install_kind, format!("{kind}\n"))
            .map_err(|e| format!("cannot write {}: {e}", install_kind.display()))?;
        Ok(vec![
            (self.notices(), NOTICES.to_string()),
            (install_kind, "install-kind".to_string()),
        ])
    }

    /// Final artifact path for a format.
    pub fn artifact(&self, format: Format) -> PathBuf {
        let name = names::artifact_name(&self.version, self.target, format)
            .expect("only shipped formats have an artifact name");
        self.dist.join(name)
    }
}

pub fn copy(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))
}

pub fn run_tool(command: &mut Command, what: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|e| format!("cannot start {what}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what} failed ({status})"))
    }
}

/// Runs `cargo packager` with a config file for the given formats.
pub fn run_packager(root: &Path, config: &Path, formats: &str, out: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    run_tool(
        Command::new(cargo)
            .current_dir(root)
            .args(["packager", "--verbose", "--config"])
            .arg(config)
            .args(["--formats", formats, "--out-dir"])
            .arg(out)
            // Packaging here is ad-hoc only: never let the packager reach for a certificate.
            .env_remove("APPLE_CERTIFICATE")
            .env_remove("APPLE_CERTIFICATE_PASSWORD")
            .env_remove("APPLE_SIGNING_IDENTITY"),
        &format!("cargo packager --formats {formats}"),
    )
}

pub fn first_file_with_extension(dir: &Path, extension: &str) -> Result<PathBuf, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(extension))
        })
        .ok_or_else(|| format!("no *{extension} file in {}", dir.display()))
}

fn build_binaries(root: &Path, test_feed: bool) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut command = Command::new(cargo);
    command.current_dir(root).args([
        "build",
        "--release",
        "--locked",
        "-p",
        "hushpen-app",
        "-p",
        "hushpen-llm",
    ]);
    if test_feed {
        command.args(["--features", "hushpen-app/update-test-feed"]);
    }
    run_tool(command.env("GGML_NATIVE", "OFF"), "cargo build --release")
}

pub fn sha256_hex(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// `SHA256SUMS`: one `<hash>  <name>` line for each artifact in the folder.
fn write_sums(dist: &Path, artifacts: &[PathBuf]) -> Result<(), String> {
    let mut lines = Vec::new();
    for artifact in artifacts {
        let name = artifact
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("artifact name is not UTF-8")?;
        lines.push(format!("{}  {name}\n", sha256_hex(artifact)?));
    }
    lines.sort();
    fs::write(dist.join("SHA256SUMS"), lines.concat())
        .map_err(|e| format!("cannot write SHA256SUMS: {e}"))
}

pub fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;
    let target = match options.target {
        Some(target) => target,
        None => Target::host()?,
    };
    if Target::host()? != target {
        return Err(format!(
            "this machine builds {} only, not {}; build each target on its own runner",
            Target::host()?.name(),
            target.name()
        ));
    }
    let mut formats = options.formats.unwrap_or_else(|| {
        target
            .formats()
            .iter()
            .copied()
            .filter(|f| *f != Format::Dmg || dmg_allowed_here())
            .collect()
    });
    for format in &formats {
        format.check_target(target)?;
    }
    if formats.contains(&Format::Dmg) {
        dmg_allowed(
            std::env::var("CI").ok().as_deref(),
            std::env::var("GITHUB_ACTIONS").ok().as_deref(),
        )?;
    }
    formats.dedup();

    if !options.skip_build {
        build_binaries(root, options.test_feed)?;
    }
    let binaries = target_dir(root).join("release");
    let ctx = Ctx {
        root: root.to_path_buf(),
        version: build_version(),
        target,
        dist: options
            .out
            .unwrap_or_else(|| target_dir(root).join("dist").join(target.name())),
        binaries,
    };
    fs::create_dir_all(&ctx.dist).map_err(|e| format!("cannot create dist: {e}"))?;
    // The app of a test-feed build carries the feed override on purpose. The other binary and
    // the packaging steps are held to the release rules as usual.
    let checked: &[&str] = if options.test_feed {
        println!("test-feed build: never publish these files");
        &[LLM_BINARY]
    } else {
        &[APP_BINARY, LLM_BINARY]
    };
    for binary in checked {
        crate::release_check::check_binary(&ctx.binaries.join(binary))?;
    }

    let mut artifacts = if target.is_linux() {
        linux::build(&ctx, &formats)?
    } else {
        mac::build(&ctx, &formats)?
    };
    artifacts.sort();
    write_sums(&ctx.dist, &artifacts)?;
    for artifact in &artifacts {
        let size = fs::metadata(artifact).map_or(0, |m| m.len());
        println!("{} ({size} bytes)", artifact.display());
    }
    println!(
        "package passed: {} artifact(s) in {}",
        artifacts.len(),
        ctx.dist.display()
    );
    Ok(())
}

fn dmg_allowed_here() -> bool {
    dmg_allowed(
        std::env::var("CI").ok().as_deref(),
        std::env::var("GITHUB_ACTIONS").ok().as_deref(),
    )
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn arguments_are_parsed_and_unknown_ones_refused() {
        let options = parse_args(&args(&[
            "--target",
            "linux-x64",
            "--formats",
            "deb,tar.gz",
            "--skip-build",
        ]))
        .unwrap();
        assert_eq!(options.target, Some(Target::LinuxX64));
        assert_eq!(options.formats, Some(vec![Format::Deb, Format::TarGz]));
        assert!(options.skip_build);
        assert!(parse_args(&args(&["--target"])).is_err());
        assert!(parse_args(&args(&["--formats", "msi"])).is_err());
        assert!(parse_args(&args(&["--bogus"])).is_err());
    }

    #[test]
    fn a_test_feed_build_must_name_its_own_out_folder() {
        assert!(parse_args(&args(&["--test-feed"])).is_err());
        let options = parse_args(&args(&["--test-feed", "--out", "/x/feed-test"])).unwrap();
        assert!(options.test_feed);
        assert!(!parse_args(&args(&[])).unwrap().test_feed);
    }

    #[test]
    fn the_dmg_is_refused_off_ci_and_outside_github_actions() {
        assert!(dmg_allowed(None, None).is_err());
        assert!(dmg_allowed(Some("true"), None).is_err());
        assert!(dmg_allowed(Some("1"), Some("true")).is_err());
        assert!(dmg_allowed(Some("true"), Some("true")).is_ok());
    }

    #[test]
    fn sha256sums_cover_every_artifact_and_no_extra_file() {
        let dir = std::env::temp_dir().join(format!("pkg-sums-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let first = dir.join("Hushpen-2.0.0-linux-x64.deb");
        let second = dir.join("Hushpen-2.0.0-linux-x64.rpm");
        fs::write(&first, b"abc").unwrap();
        fs::write(&second, b"").unwrap();
        fs::write(dir.join("stray.txt"), b"x").unwrap();
        write_sums(&dir, &[first, second]).unwrap();
        let sums = fs::read_to_string(dir.join("SHA256SUMS")).unwrap();
        let lines: Vec<&str> = sums.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines.contains(
            &"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  Hushpen-2.0.0-linux-x64.deb"
        ));
        assert!(lines.contains(
            &"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  Hushpen-2.0.0-linux-x64.rpm"
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn resources_hold_the_notices_and_an_install_kind_file() {
        let root = std::env::temp_dir().join(format!("pkg-res-{}", std::process::id()));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("assets").join(NOTICES), b"<html></html>").unwrap();
        let ctx = Ctx {
            root: root.clone(),
            version: "2.0.0".into(),
            target: Target::LinuxX64,
            dist: root.join("dist"),
            binaries: root.join("bin"),
        };
        let resources = ctx.resources(&root, "deb").unwrap();
        let targets: Vec<&str> = resources.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(targets, [NOTICES, "install-kind"]);
        assert_eq!(
            fs::read_to_string(root.join("install-kind")).unwrap(),
            "deb\n"
        );
        fs::remove_file(root.join("assets").join(NOTICES)).unwrap();
        assert!(ctx.resources(&root, "deb").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
