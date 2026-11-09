//! `cargo xtask package-verify --target <t> [--dir <dist>] [--formats ...]`:
//! opens every artifact in a dist folder and checks it against the plan: the
//! file set, `SHA256SUMS`, the metadata, the dependency lines, the notices, the
//! install kind, the ad-hoc signature, and the release-check scan of every
//! binary inside. The dmg is only read with `hdiutil imageinfo` unless this
//! is the CI runner, which may mount it.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::names::{self, Format, Target};
use super::target_dir;
use super::{
    APP_BINARY, ASR_BINARY, BINARIES, LLM_BINARY, NOTICES, TTS_BINARY, VAD_MODEL, build_version,
    sha256_hex,
};

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
    for binary in BINARIES {
        let path = dir.join(binary);
        if let Err(error) = crate::release_check::check_binary(&path) {
            problems.push(format!("{}: {error}", path.display()));
        }
    }
}

fn check_vad(dir: &Path, problems: &mut Vec<String>) {
    if let Err(error) = super::verify_vad(&dir.join(VAD_MODEL)) {
        problems.push(error);
    }
}

fn check_version_line(binary: &Path, version: &str, problems: &mut Vec<String>) {
    let name = binary
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(APP_BINARY)
        .to_owned();
    match capture(Command::new(binary).arg("--version"), "--version") {
        Ok(text) => need(
            problems,
            text.trim() == format!("{name} {version}"),
            format!("{} --version printed {:?}", binary.display(), text.trim()),
        ),
        Err(error) => problems.push(error),
    }
}

/// The sherpa-onnx libraries of the package are the pinned release files. A deb, rpm, or tar.gz
/// holds them byte for byte. The macOS bundle signs them again and the AppImage tool strips and
/// patches them, so there `rewritten` names the pinned copies, and their machine code must match.
fn check_tts_libs(
    dir: &Path,
    target: Target,
    rewritten: Option<&Path>,
    problems: &mut Vec<String>,
) {
    for lib in crate::tts_runtime::archive_for(target).libs {
        let path = dir.join(lib.name);
        let same = match rewritten {
            Some(pinned) => crate::tts_runtime::same_code(&path, &pinned.join(lib.name)),
            None => crate::tts_runtime::matches_pinned(&path, lib),
        };
        match same {
            Ok(true) => {}
            Ok(false) => problems.push(format!("{} is not the pinned library", path.display())),
            Err(error) => problems.push(error),
        }
    }
}

/// The packaged worker really loads the bundled libraries (dlopen and the version pin).
fn check_tts_runtime(dir: &Path, problems: &mut Vec<String>) {
    match capture(
        Command::new(dir.join(format!("{TTS_BINARY}{}", std::env::consts::EXE_SUFFIX)))
            .arg("--check-runtime"),
        "hushpen-tts --check-runtime",
    ) {
        Ok(text) => need(
            problems,
            text.trim()
                == format!(
                    "sherpa-onnx {}\nonnxruntime {}",
                    crate::tts_runtime::SHERPA_VERSION,
                    crate::tts_runtime::ONNXRUNTIME_VERSION
                ),
            format!("hushpen-tts --check-runtime printed {:?}", text.trim()),
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
    let mut wanted = vec![
        APP_BINARY,
        ASR_BINARY,
        LLM_BINARY,
        TTS_BINARY,
        NOTICES,
        "install-kind",
        "icons/512x512.png",
    ];
    for lib in crate::tts_runtime::archive_for(target).libs {
        wanted.push(lib.name);
    }
    for file in wanted {
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
    check_vad(&root, &mut problems);
    check_version_line(&root.join(APP_BINARY), version, &mut problems);
    check_version_line(&root.join(ASR_BINARY), version, &mut problems);
    check_version_line(&root.join(TTS_BINARY), version, &mut problems);
    check_tts_libs(&root, target, None, &mut problems);
    check_tts_runtime(&root, &mut problems);
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
        "./usr/lib/hushpen/hushpen-asr\n",
        "./usr/lib/hushpen/hushpen-llm\n",
        "./usr/lib/hushpen/hushpen-tts\n",
        "./usr/lib/hushpen/libsherpa-onnx-c-api.so\n",
        "./usr/lib/hushpen/libonnxruntime.so\n",
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
    check_vad(&lib, &mut problems);
    check_version_line(&scratch.join("usr/bin/hushpen"), version, &mut problems);
    check_version_line(&lib.join(ASR_BINARY), version, &mut problems);
    check_version_line(&lib.join(TTS_BINARY), version, &mut problems);
    check_tts_libs(&lib, target, None, &mut problems);
    check_tts_runtime(&lib, &mut problems);
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
    let want = format!(
        "hushpen|{}|AGPL-3.0-or-later|{}",
        super::config::rpm_version(version),
        target.rpm_arch()
    );
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
        "/usr/lib/hushpen/hushpen-asr",
        "/usr/lib/hushpen/hushpen-llm",
        "/usr/lib/hushpen/hushpen-tts",
        "/usr/lib/hushpen/libsherpa-onnx-c-api.so",
        "/usr/lib/hushpen/libonnxruntime.so",
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
    check_vad(&lib, &mut problems);
    check_version_line(&lib.join(APP_BINARY), version, &mut problems);
    check_version_line(&lib.join(ASR_BINARY), version, &mut problems);
    check_version_line(&lib.join(TTS_BINARY), version, &mut problems);
    check_tts_libs(&lib, target, None, &mut problems);
    check_tts_runtime(&lib, &mut problems);
    Ok(problems)
}

fn verify_appimage(
    artifact: &Path,
    scratch: &Path,
    version: &str,
    target: Target,
    pinned: &Path,
) -> Result<Vec<String>, String> {
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
    let bin = root.join("usr/bin");
    check_vad(&root.join("usr/lib/hushpen"), &mut problems);
    scan_binaries(&bin, &mut problems);
    check_version_line(&bin.join(APP_BINARY), version, &mut problems);
    check_version_line(&bin.join(ASR_BINARY), version, &mut problems);
    check_version_line(&bin.join(TTS_BINARY), version, &mut problems);
    check_tts_libs(&bin, target, Some(pinned), &mut problems);
    check_tts_runtime(&bin, &mut problems);
    Ok(problems)
}

fn plist_value(plist: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\" => ");
    plist
        .lines()
        .find_map(|line| line.trim().strip_prefix(&needle))
        .map(|value| value.trim_matches('"').to_string())
}

/// macOS 26 and later find the icon in `Assets.car` by `CFBundleIconName`; with no icon of that
/// name they draw the .icns inside a gray tile. The renditions of an icon named `Hushpen` are
/// stored as `Hushpen/...`.
fn car_has_icon(car: &[u8], name: &str) -> bool {
    let needle = format!("{name}/");
    !name.is_empty()
        && car
            .windows(needle.len())
            .any(|window| window == needle.as_bytes())
}

fn verify_zip(
    artifact: &Path,
    scratch: &Path,
    version: &str,
    target: Target,
    pinned: &Path,
) -> Result<Vec<String>, String> {
    if target == Target::WindowsX64 {
        let mut signature = [0_u8; 4];
        fs::File::open(artifact)
            .and_then(|mut file| file.read_exact(&mut signature))
            .map_err(|e| format!("cannot read ZIP signature: {e}"))?;
        if signature != *b"PK\x03\x04" {
            return Err(format!("{} is not a ZIP archive", artifact.display()));
        }
        capture(
            super::archive_tool()?
                .arg("-xf")
                .arg(artifact)
                .arg("-C")
                .arg(scratch),
            "tar (portable zip)",
        )?;
        return verify_windows_folder(scratch, version, "windows-zip");
    }
    let mut problems = Vec::new();
    capture(
        Command::new("ditto")
            .args(["-x", "-k"])
            .arg(artifact)
            .arg(scratch),
        "ditto -x -k",
    )?;
    let app = scratch.join("Hushpen.app");
    problems.extend(verify_app_bundle(&app, version, target, pinned)?);
    Ok(problems)
}

fn verify_windows_folder(dir: &Path, version: &str, kind: &str) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    for binary in BINARIES {
        let path = dir.join(super::binary_name(Target::WindowsX64, binary));
        if let Err(error) = crate::release_check::check_binary(&path) {
            problems.push(error);
        }
        check_version_line(&path, version, &mut problems);
    }
    need(
        &mut problems,
        dir.join(NOTICES).is_file(),
        format!("Windows package lacks {NOTICES}"),
    );
    need(
        &mut problems,
        fs::read_to_string(dir.join("install-kind"))
            .unwrap_or_default()
            .trim()
            == kind,
        format!("Windows install-kind is not {kind}"),
    );
    check_vad(dir, &mut problems);
    check_tts_libs(dir, Target::WindowsX64, None, &mut problems);
    check_tts_runtime(dir, &mut problems);
    let binary = dir.join("hushpen.exe");
    let manifest = dir.join("verified-manifest.xml");
    check_windows_manifest(&binary, &manifest, true, &mut problems)?;
    let metadata = capture(Command::new("pwsh").args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg("$v = (Get-Item -LiteralPath $env:HUSHPEN_VERIFY_EXE).VersionInfo; $v.ProductVersion; $v.FileVersion")
        .env("HUSHPEN_VERIFY_EXE", &binary), "PowerShell (version resource)")?;
    need(
        &mut problems,
        metadata.lines().collect::<Vec<_>>() == [version, version],
        "Windows version resource differs from the build version",
    );
    Ok(problems)
}

fn check_windows_manifest(
    binary: &Path,
    manifest: &Path,
    dpi: bool,
    problems: &mut Vec<String>,
) -> Result<(), String> {
    let result = capture(
        Command::new("mt.exe")
            .arg(format!("-inputresource:{};#1", binary.display()))
            .arg(format!("-out:{}", manifest.display())),
        "mt (app manifest)",
    );
    match result {
        Ok(_) => {
            let text = fs::read_to_string(manifest)
                .map_err(|e| format!("cannot read Windows manifest: {e}"));
            fs::remove_file(manifest)
                .map_err(|e| format!("cannot remove extracted manifest: {e}"))?;
            let text = text?;
            if dpi {
                need(
                    problems,
                    text.contains("PerMonitorV2"),
                    "Windows manifest lacks PerMonitorV2",
                );
            }
            need(
                problems,
                text.contains("level=\"asInvoker\""),
                format!("{} requests elevation", binary.display()),
            );
        }
        Err(error) => problems.push(error),
    }
    if manifest.exists() {
        fs::remove_file(manifest).map_err(|e| format!("cannot remove extracted manifest: {e}"))?;
    }
    Ok(())
}

fn verify_nsis(artifact: &Path, scratch: &Path, version: &str) -> Result<Vec<String>, String> {
    if Target::host()? != Target::WindowsX64 {
        return Err("Windows setup must be verified on Windows".into());
    }
    let mut problems = Vec::new();
    check_windows_manifest(
        artifact,
        &scratch.join("setup-manifest.xml"),
        false,
        &mut problems,
    )?;
    if !problems.is_empty() {
        return Ok(problems);
    }
    let setup = std::path::absolute(artifact)
        .map_err(|e| format!("cannot resolve Windows setup path: {e}"))?;
    let scratch = std::path::absolute(scratch)
        .map_err(|e| format!("cannot resolve setup check folder: {e}"))?;
    let blocked = scratch.join("blocked-install");
    fs::write(&blocked, b"blocked installation")
        .map_err(|e| format!("cannot create blocked install destination: {e}"))?;
    let mut refused = Command::new(&setup);
    refused.arg("/S").current_dir(&scratch);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        refused.raw_arg(format!("/D={}", blocked.display()));
    }
    let refused = refused
        .output()
        .map_err(|e| format!("cannot check refused installation: {e}"))?;
    need(
        &mut problems,
        !refused.status.success(),
        "Windows setup accepted a file as its destination",
    );
    need(
        &mut problems,
        !scratch.join("hushpen.exe").exists(),
        "Windows setup wrote outside its refused destination",
    );
    if !problems.is_empty() {
        return Ok(problems);
    }
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is missing")?;
    let install = PathBuf::from(local).join("Programs").join("Hushpen");
    if install.exists() {
        return Err(format!(
            "refuse to replace an existing install at {}",
            install.display()
        ));
    }
    capture(Command::new(&setup).arg("/S"), "Windows setup /S")?;
    let result = verify_windows_folder(&install, version, "windows-setup");
    let helper = scratch.join("uninstall.exe");
    super::copy(&install.join("uninstall.exe"), &helper)?;
    let mut uninstall = Command::new(&helper);
    uninstall.arg("/S");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // NSIS reads the final directory verbatim, including spaces, without argument quotes.
        uninstall.raw_arg(format!("_?={}", install.display()));
    }
    let uninstall = capture(&mut uninstall, "Windows uninstall /S");
    let mut problems = result?;
    if let Err(error) = uninstall {
        problems.push(error);
    }
    need(
        &mut problems,
        !install.join("hushpen.exe").exists(),
        "Windows uninstall left the app executable",
    );
    Ok(problems)
}

fn verify_app_bundle(
    app: &Path,
    version: &str,
    target: Target,
    pinned: &Path,
) -> Result<Vec<String>, String> {
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
    let icon = plist_value(&plist, "CFBundleIconName").unwrap_or_default();
    let car = fs::read(app.join("Contents/Resources/Assets.car")).unwrap_or_default();
    need(
        &mut problems,
        car_has_icon(&car, &icon),
        format!("Resources/Assets.car has no icon named {icon:?} (CFBundleIconName)"),
    );
    let macos = app.join("Contents/MacOS");
    check_vad(&app.join("Contents/Resources"), &mut problems);
    scan_binaries(&macos, &mut problems);
    check_tts_libs(
        &app.join("Contents/Frameworks"),
        target,
        Some(pinned),
        &mut problems,
    );
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
    check_version_line(&macos.join(ASR_BINARY), version, &mut problems);
    check_version_line(&macos.join(LLM_BINARY), version, &mut problems);
    check_version_line(&macos.join(TTS_BINARY), version, &mut problems);
    check_tts_runtime(&macos, &mut problems);
    Ok(problems)
}

fn verify_dmg(
    artifact: &Path,
    scratch: &Path,
    version: &str,
    target: Target,
    pinned: &Path,
) -> Result<Vec<String>, String> {
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
    match verify_app_bundle(&mount.join("Hushpen.app"), version, target, pinned) {
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

    let pinned = crate::tts_runtime::dir(&target_dir(root), target);
    crate::tts_runtime::ensure(&pinned, target)?;
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
            Format::AppImage => verify_appimage(&artifact, &scratch, &version, target, &pinned)?,
            Format::Zip => verify_zip(&artifact, &scratch, &version, target, &pinned)?,
            Format::Dmg => verify_dmg(&artifact, &scratch, &version, target, &pinned)?,
            Format::Nsis => verify_nsis(&artifact, &scratch, &version)?,
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
    fn a_renamed_tar_is_not_a_portable_zip() {
        let dir = std::env::temp_dir().join(format!("not-zip-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let artifact = dir.join("portable.zip");
        fs::write(&artifact, b"./\0\0").unwrap();
        let error = verify_zip(&artifact, &dir, "2.4.0", Target::WindowsX64, &dir).unwrap_err();
        assert!(error.contains("is not a ZIP archive"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_shipped_icon_catalog_holds_the_icon_the_plist_names() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let plist = fs::read_to_string(root.join("packaging/Info.plist")).unwrap();
        let name = plist
            .split("<key>CFBundleIconName</key>")
            .nth(1)
            .and_then(|rest| rest.split("<string>").nth(1))
            .and_then(|rest| rest.split("</string>").next())
            .unwrap();
        let car = fs::read(root.join("assets/hushpen-app-icon.car")).unwrap();
        assert!(car_has_icon(&car, name), "Assets.car has no icon {name:?}");
        assert!(!car_has_icon(&car, "hushpen-app-icon"));
        assert!(!car_has_icon(b"", ""));
    }

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
