//! `cargo xtask package-verify --target <t> [--dir <dist>] [--formats ...]`:
//! opens every artifact in a dist folder and checks it against the plan: the
//! file set, `SHA256SUMS`, the metadata, the dependency lines, the notices, the
//! install kind, the ad-hoc signature, and the release-check scan of both
//! binaries inside. The dmg is only read with `hdiutil imageinfo` unless this
//! is the CI runner, which may mount it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::names::{self, Format, Target};
use super::{APP_BINARY, LLM_BINARY, NOTICES, build_version, sha256_hex, target_dir};

const DEB_DEPENDS: &[&str] = &[
    "libvulkan1",
    "mesa-vulkan-drivers | libgl1-mesa-dri",
    "libxkbcommon0",
    "libxkbcommon-x11-0",
    "libasound2t64 | libasound2",
    "libfontconfig1",
];
const RPM_REQUIRES: &[&str] = &[
    "vulkan-loader",
    "mesa-vulkan-drivers",
    "libxkbcommon-x11",
    "alsa-lib",
    "fontconfig",
];

fn capture(command: &mut Command, what: &str) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|e| format!("cannot start {what}: {e}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        Ok(text)
    } else {
        Err(format!("{what} failed ({}): {text}", output.status))
    }
}

fn need(problems: &mut Vec<String>, ok: bool, message: impl Into<String>) {
    if !ok {
        problems.push(message.into());
    }
}

fn scan_binaries(dir: &Path, problems: &mut Vec<String>) {
    for binary in [APP_BINARY, LLM_BINARY] {
        let path = dir.join(binary);
        if let Err(error) = crate::release_check::check_binary(&path) {
            problems.push(format!("{}: {error}", path.display()));
        }
    }
}

fn check_version_line(binary: &Path, version: &str, problems: &mut Vec<String>) {
    match capture(Command::new(binary).arg("--version"), "hushpen --version") {
        Ok(text) => need(
            problems,
            text.trim() == format!("hushpen {version}"),
            format!("{} --version printed {:?}", binary.display(), text.trim()),
        ),
        Err(error) => problems.push(error),
    }
}

fn fresh(dir: &Path) -> Result<(), String> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
    }
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))
}

fn verify_tar(
    artifact: &Path,
    scratch: &Path,
    version: &str,
    target: Target,
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let listing = capture(Command::new("tar").arg("-tzf").arg(artifact), "tar -tzf")?;
    let top = names::tar_top_folder(version, target);
    need(
        &mut problems,
        listing
            .lines()
            .all(|line| line.starts_with(&format!("{top}/"))),
        format!("tar.gz has files outside the single top folder {top}"),
    );
    capture(
        Command::new("tar")
            .arg("-xzf")
            .arg(artifact)
            .arg("-C")
            .arg(scratch),
        "tar -xzf",
    )?;
    let root = scratch.join(&top);
    for file in [
        APP_BINARY,
        LLM_BINARY,
        NOTICES,
        "install-kind",
        "icons/512x512.png",
    ] {
        need(
            &mut problems,
            root.join(file).is_file(),
            format!("tar.gz lacks {file}"),
        );
    }
    let kind = fs::read_to_string(root.join("install-kind")).unwrap_or_default();
    need(
        &mut problems,
        kind.trim() == "tar",
        format!("install-kind says {:?}, not tar", kind.trim()),
    );
    scan_binaries(&root, &mut problems);
    check_version_line(&root.join(APP_BINARY), version, &mut problems);
    Ok(problems)
}

fn verify_deb(
    artifact: &Path,
    scratch: &Path,
    version: &str,
    target: Target,
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let field = |name: &str| -> Result<String, String> {
        capture(
            Command::new("dpkg-deb").arg("-f").arg(artifact).arg(name),
            "dpkg-deb -f",
        )
        .map(|text| text.trim().to_string())
    };
    need(
        &mut problems,
        field("Package")? == "hushpen",
        "deb Package is not hushpen",
    );
    need(
        &mut problems,
        field("Version")? == version,
        "deb Version differs from the build version",
    );
    need(
        &mut problems,
        field("Architecture")? == target.deb_arch(),
        format!("deb Architecture is not {}", target.deb_arch()),
    );
    let depends = field("Depends")?;
    for want in DEB_DEPENDS {
        need(
            &mut problems,
            depends.split(", ").any(|d| d == *want),
            format!("deb Depends lacks {want}"),
        );
    }
    need(
        &mut problems,
        field("Recommends")?.contains("xdg-desktop-portal"),
        "deb Recommends lacks xdg-desktop-portal",
    );

    let listing = capture(
        Command::new("dpkg-deb").arg("-c").arg(artifact),
        "dpkg-deb -c",
    )?;
    for want in [
        "./usr/lib/hushpen/hushpen\n",
        "./usr/lib/hushpen/hushpen-llm\n",
        "./usr/lib/hushpen/THIRD-PARTY-NOTICES.html\n",
        "./usr/lib/hushpen/install-kind\n",
        "./usr/share/applications/hushpen.desktop\n",
        "./usr/share/icons/hicolor/512x512/apps/hushpen.png\n",
        "./usr/bin/hushpen -> ../lib/hushpen/hushpen\n",
    ] {
        need(
            &mut problems,
            listing.contains(want.trim_end()),
            format!("deb lacks {}", want.trim_end()),
        );
    }
    capture(
        Command::new("dpkg-deb")
            .arg("-x")
            .arg(artifact)
            .arg(scratch),
        "dpkg-deb -x",
    )?;
    let lib = scratch.join("usr/lib/hushpen");
    let kind = fs::read_to_string(lib.join("install-kind")).unwrap_or_default();
    need(
        &mut problems,
        kind.trim() == "deb",
        format!("install-kind says {:?}, not deb", kind.trim()),
    );
    scan_binaries(&lib, &mut problems);
    check_version_line(&scratch.join("usr/bin/hushpen"), version, &mut problems);
    Ok(problems)
}

fn verify_rpm(
    artifact: &Path,
    scratch: &Path,
    version: &str,
    target: Target,
) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let query = |args: &[&str]| {
        capture(
            Command::new("rpm").arg("-qp").args(args).arg(artifact),
            "rpm -qp",
        )
    };
    let info = query(&["--qf", "%{NAME}|%{VERSION}|%{LICENSE}|%{ARCH}"])?;
    let want = format!("hushpen|{version}|AGPL-3.0-or-later|{}", target.rpm_arch());
    need(
        &mut problems,
        info.trim() == want,
        format!("rpm info is {:?}, not {want:?}", info.trim()),
    );
    let requires = query(&["-R"])?;
    for want in RPM_REQUIRES {
        need(
            &mut problems,
            requires.lines().any(|l| l.trim() == *want),
            format!("rpm Requires lacks {want}"),
        );
    }
    need(
        &mut problems,
        query(&["--recommends"])?.contains("xdg-desktop-portal"),
        "rpm Recommends lacks xdg-desktop-portal",
    );
    let files = query(&["-l"])?;
    for want in [
        "/usr/lib/hushpen/hushpen",
        "/usr/lib/hushpen/hushpen-llm",
        "/usr/lib/hushpen/THIRD-PARTY-NOTICES.html",
        "/usr/bin/hushpen",
        "/usr/share/applications/hushpen.desktop",
        "/usr/share/icons/hicolor/512x512/apps/hushpen.png",
    ] {
        need(
            &mut problems,
            files.lines().any(|l| l == want),
            format!("rpm lacks {want}"),
        );
    }
    let script = format!(
        "rpm2archive - < {} | tar -xzf -",
        super::config::shell_quote(&artifact.display().to_string())
    );
    capture(
        Command::new("sh")
            .current_dir(scratch)
            .args(["-c", &script]),
        "rpm2archive | tar",
    )?;
    let lib = scratch.join("usr/lib/hushpen");
    let kind = fs::read_to_string(lib.join("install-kind")).unwrap_or_default();
    need(
        &mut problems,
        kind.trim() == "rpm",
        format!("install-kind says {:?}, not rpm", kind.trim()),
    );
    scan_binaries(&lib, &mut problems);
    check_version_line(&lib.join(APP_BINARY), version, &mut problems);
    Ok(problems)
}

fn verify_appimage(artifact: &Path, scratch: &Path, version: &str) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    capture(
        Command::new(artifact)
            .current_dir(scratch)
            .env("APPIMAGE_EXTRACT_AND_RUN", "1")
            .arg("--appimage-extract"),
        "AppImage --appimage-extract",
    )?;
    let root = scratch.join("squashfs-root");
    need(
        &mut problems,
        root.join("usr/lib/hushpen").join(NOTICES).is_file(),
        format!("AppImage lacks {NOTICES}"),
    );
    scan_binaries(&root.join("usr/bin"), &mut problems);
    check_version_line(
        &root.join("usr/bin").join(APP_BINARY),
        version,
        &mut problems,
    );
    Ok(problems)
}

fn plist_value(plist: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\" => ");
    plist
        .lines()
        .find_map(|line| line.trim().strip_prefix(&needle))
        .map(|value| value.trim_matches('"').to_string())
}

fn verify_zip(artifact: &Path, scratch: &Path, version: &str) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    capture(
        Command::new("ditto")
            .args(["-x", "-k"])
            .arg(artifact)
            .arg(scratch),
        "ditto -x -k",
    )?;
    let app = scratch.join("Hushpen.app");
    problems.extend(verify_app_bundle(&app, version)?);
    Ok(problems)
}

fn verify_app_bundle(app: &Path, version: &str) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let plist = capture(
        Command::new("plutil")
            .arg("-p")
            .arg(app.join("Contents/Info.plist")),
        "plutil -p",
    )?;
    for (key, want) in [
        ("CFBundleShortVersionString", version),
        ("CFBundleIdentifier", super::config::BUNDLE_ID),
        ("LSMinimumSystemVersion", super::config::MIN_MACOS),
    ] {
        let got = plist_value(&plist, key);
        need(
            &mut problems,
            got.as_deref() == Some(want),
            format!("Info.plist {key} is {got:?}, not {want:?}"),
        );
    }
    let mic = plist_value(&plist, "NSMicrophoneUsageDescription").unwrap_or_default();
    need(
        &mut problems,
        !mic.trim().is_empty(),
        "Info.plist has no NSMicrophoneUsageDescription",
    );
    need(
        &mut problems,
        app.join("Contents/Resources").join(NOTICES).is_file(),
        format!("the app lacks Resources/{NOTICES}"),
    );
    let macos = app.join("Contents/MacOS");
    scan_binaries(&macos, &mut problems);
    if let Err(error) = capture(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(app),
        "codesign --verify --deep --strict",
    ) {
        problems.push(error);
    }
    match capture(Command::new("codesign").arg("-dv").arg(app), "codesign -dv") {
        Ok(text) => need(
            &mut problems,
            text.contains("Signature=adhoc"),
            "the app is not ad-hoc signed",
        ),
        Err(error) => problems.push(error),
    }
    check_version_line(&macos.join(APP_BINARY), version, &mut problems);
    Ok(problems)
}

fn verify_dmg(artifact: &Path, scratch: &Path, version: &str) -> Result<Vec<String>, String> {
    let info = capture(
        Command::new("hdiutil").arg("imageinfo").arg(artifact),
        "hdiutil imageinfo",
    )?;
    println!("{info}");
    let on_runner = std::env::var("CI").as_deref() == Ok("true")
        && std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true");
    if !on_runner {
        println!("dmg contents are checked on the CI runner only; imageinfo is all that runs here");
        return Ok(Vec::new());
    }
    let mount = scratch.join("mnt");
    fs::create_dir_all(&mount).map_err(|e| format!("cannot create {}: {e}", mount.display()))?;
    capture(
        Command::new("hdiutil")
            .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
            .arg(&mount)
            .arg(artifact),
        "hdiutil attach",
    )?;
    let mut problems = Vec::new();
    need(
        &mut problems,
        mount.join("Applications").exists(),
        "the dmg has no Applications link",
    );
    match verify_app_bundle(&mount.join("Hushpen.app"), version) {
        Ok(found) => problems.extend(found),
        Err(error) => problems.push(error),
    }
    let _ = capture(
        Command::new("hdiutil").arg("detach").arg(&mount),
        "hdiutil detach",
    );
    Ok(problems)
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    target: Option<Target>,
    dir: Option<PathBuf>,
    formats: Option<Vec<Format>>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        target: None,
        dir: None,
        formats: None,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| iter.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--target" => options.target = Some(Target::parse(value("--target")?)?),
            "--dir" => options.dir = Some(PathBuf::from(value("--dir")?)),
            "--formats" => {
                options.formats = Some(
                    value("--formats")?
                        .split(',')
                        .map(|word| Format::parse(word.trim()))
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            other => return Err(format!("unknown package-verify argument {other:?}")),
        }
    }
    Ok(options)
}

/// File names in `dir` that are not in `planned`, and planned names that are missing.
fn name_differences(dir: &Path, planned: &[String]) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let present: Vec<String> = fs::read_dir(dir)
        .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name != "SHA256SUMS")
        .collect();
    for name in planned {
        need(
            &mut problems,
            present.contains(name),
            format!("missing artifact {name}"),
        );
    }
    for name in &present {
        need(
            &mut problems,
            planned.contains(name),
            format!("unplanned file {name}"),
        );
    }
    Ok(problems)
}

fn check_sums(dir: &Path, planned: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    let sums = fs::read_to_string(dir.join("SHA256SUMS")).unwrap_or_default();
    for name in planned {
        let line = sums
            .lines()
            .find(|line| line.ends_with(&format!("  {name}")));
        match (line, sha256_hex(&dir.join(name))) {
            (Some(line), Ok(hash)) => need(
                &mut problems,
                line.starts_with(&hash),
                format!("SHA256SUMS hash of {name} is wrong"),
            ),
            (None, _) => problems.push(format!("SHA256SUMS has no line for {name}")),
            (_, Err(error)) => problems.push(error),
        }
    }
    problems
}

pub fn run_verify(root: &Path, args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;
    let target = match options.target {
        Some(target) => target,
        None => Target::host()?,
    };
    let version = build_version();
    let dir = options
        .dir
        .unwrap_or_else(|| target_dir(root).join("dist").join(target.name()));
    let (formats, planned) = match options.formats {
        Some(formats) => {
            let planned = formats
                .iter()
                .filter_map(|format| names::artifact_name(&version, target, *format))
                .collect();
            (formats, planned)
        }
        None => (
            target.formats().to_vec(),
            names::planned_artifacts(&version, target),
        ),
    };

    let mut problems = name_differences(&dir, &planned)?;
    problems.extend(check_sums(&dir, &planned));
    for format in formats {
        let Some(name) = names::artifact_name(&version, target, format) else {
            continue;
        };
        let artifact = dir.join(&name);
        if !artifact.is_file() {
            continue;
        }
        let scratch = dir.join("verify").join(format.extension().unwrap_or("app"));
        fresh(&scratch)?;
        println!("==> {name}");
        let found = match format {
            Format::TarGz => verify_tar(&artifact, &scratch, &version, target)?,
            Format::Deb => verify_deb(&artifact, &scratch, &version, target)?,
            Format::Rpm => verify_rpm(&artifact, &scratch, &version, target)?,
            Format::AppImage => verify_appimage(&artifact, &scratch, &version)?,
            Format::Zip => verify_zip(&artifact, &scratch, &version)?,
            Format::Dmg => verify_dmg(&artifact, &scratch, &version)?,
            Format::App => Vec::new(),
        };
        for problem in &found {
            println!("PROBLEM {name}: {problem}");
        }
        problems.extend(found.into_iter().map(|p| format!("{name}: {p}")));
        let _ = fs::remove_dir_all(&scratch);
    }
    let _ = fs::remove_dir_all(dir.join("verify"));
    if problems.is_empty() {
        println!("package-verify passed");
        Ok(())
    } else {
        Err(format!(
            "{} problem(s):\n{}",
            problems.len(),
            problems.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_values_are_read_from_plutil_output() {
        let plist = "{\n  \"CFBundleShortVersionString\" => \"2.0.0\"\n  \"LSMinimumSystemVersion\" => \"13.0\"\n}\n";
        assert_eq!(
            plist_value(plist, "CFBundleShortVersionString").as_deref(),
            Some("2.0.0")
        );
        assert_eq!(plist_value(plist, "NSMicrophoneUsageDescription"), None);
    }

    #[test]
    fn missing_and_unplanned_files_are_both_reported() {
        let dir = std::env::temp_dir().join(format!("pkg-names-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Hushpen-2.0.0-linux-x64.deb"), b"x").unwrap();
        fs::write(dir.join("extra.deb"), b"x").unwrap();
        fs::write(dir.join("SHA256SUMS"), b"").unwrap();
        let planned = vec![
            "Hushpen-2.0.0-linux-x64.deb".to_string(),
            "Hushpen-2.0.0-linux-x64.rpm".to_string(),
        ];
        let problems = name_differences(&dir, &planned).unwrap();
        assert_eq!(
            problems,
            [
                "missing artifact Hushpen-2.0.0-linux-x64.rpm",
                "unplanned file extra.deb"
            ]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_wrong_hash_in_the_sums_file_is_a_problem() {
        let dir = std::env::temp_dir().join(format!("pkg-sums-bad-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.deb"), b"abc").unwrap();
        fs::write(dir.join("SHA256SUMS"), "00  a.deb\n").unwrap();
        assert_eq!(
            check_sums(&dir, &["a.deb".to_string()]),
            ["SHA256SUMS hash of a.deb is wrong"]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn arguments_are_parsed() {
        let args: Vec<String> = ["--target", "macos-arm64", "--formats", "zip"]
            .map(String::from)
            .to_vec();
        let options = parse_args(&args).unwrap();
        assert_eq!(options.target, Some(Target::MacosArm64));
        assert_eq!(options.formats, Some(vec![Format::Zip]));
        assert!(parse_args(&["--bogus".to_string()]).is_err());
    }
}
