//! Headless Linux archives for desktop SSH setup.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::names::{Target, host_artifact_name};
use super::{NOTICES, VAD_MODEL, archive_tool, copy, run_tool, verify_vad, write_sums};

const BINARIES: [&str; 3] = ["hushpen-host", "hushpen-asr", "hushpen-llm"];
const FILES: [&str; 7] = [
    "hushpen-host",
    "hushpen-asr",
    "hushpen-llm",
    VAD_MODEL,
    "LICENSE",
    NOTICES,
    "hushpen-host.service",
];

pub fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let options = super::parse_args(args)?;
    if options.formats.is_some() || options.test_feed {
        return Err("package-host accepts --target, --out, and --skip-build only".into());
    }
    let host = Target::host()?;
    let target = options.target.unwrap_or(host);
    if !target.is_linux() || host != target {
        return Err("build each Linux host archive on its own native Linux runner".into());
    }
    if !options.skip_build {
        run_tool(
            Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
                .current_dir(root)
                .args([
                    "build",
                    "--release",
                    "--locked",
                    "-p",
                    "hushpen-host",
                    "-p",
                    "hushpen-engine",
                    "-p",
                    "hushpen-llm",
                ])
                .env("GGML_NATIVE", "OFF"),
            "cargo build --release for the host",
        )?;
    }
    let output = super::target_dir(root);
    let version = super::build_version();
    let reported = Command::new(output.join("release/hushpen-host"))
        .arg("--version")
        .output()
        .map_err(|e| format!("cannot check the host version: {e}"))?;
    if !reported.status.success()
        || String::from_utf8_lossy(&reported.stdout).trim() != format!("hushpen-host {version}")
    {
        return Err(format!("the host binary version does not match {version}"));
    }
    let dist = options
        .out
        .unwrap_or_else(|| output.join("dist").join(format!("host-{}", target.name())));
    let archive = build(root, &output.join("release"), &dist, &version, target)?;
    write_sums(&dist, std::slice::from_ref(&archive))?;
    println!("host package passed: {}", archive.display());
    Ok(())
}

fn build(
    root: &Path,
    binaries: &Path,
    dist: &Path,
    version: &str,
    target: Target,
) -> Result<PathBuf, String> {
    let name = host_artifact_name(version, target).ok_or("host archives require Linux")?;
    let work = dist.join("work/host");
    if work.exists() {
        fs::remove_dir_all(&work).map_err(|e| format!("cannot clear {}: {e}", work.display()))?;
    }
    fs::create_dir_all(&work).map_err(|e| format!("cannot create {}: {e}", work.display()))?;
    for binary in BINARIES {
        crate::release_check::check_binary(&binaries.join(binary))?;
        copy(&binaries.join(binary), &work.join(binary))?;
    }
    let vad = root.join("assets").join(VAD_MODEL);
    verify_vad(&vad)?;
    for (source, name) in [
        (vad, VAD_MODEL),
        (root.join("LICENSE"), "LICENSE"),
        (root.join("assets").join(NOTICES), NOTICES),
        (
            root.join("packaging/hushpen-host.service"),
            "hushpen-host.service",
        ),
    ] {
        copy(&source, &work.join(name))?;
    }
    let artifact = dist.join(name);
    run_tool(
        archive_tool()?
            .arg("-czf")
            .arg(&artifact)
            .arg("-C")
            .arg(&work)
            .args(FILES),
        "tar the host archive",
    )?;
    let output = archive_tool()?
        .arg("-tzf")
        .arg(&artifact)
        .output()
        .map_err(|e| format!("cannot list the host archive: {e}"))?;
    if !output.status.success()
        || String::from_utf8_lossy(&output.stdout)
            .lines()
            .collect::<Vec<_>>()
            != FILES
    {
        return Err("host archive does not contain exactly the SSH setup files".into());
    }
    fs::remove_dir_all(&work).map_err(|e| format!("cannot clear {}: {e}", work.display()))?;
    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_archive_has_only_root_files_and_rejects_a_wrong_detector() {
        let root = std::env::temp_dir().join(format!("host-package-{}", std::process::id()));
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("packaging")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        for binary in BINARIES {
            fs::write(root.join("bin").join(binary), b"clean program").unwrap();
        }
        for file in [
            "LICENSE",
            "assets/THIRD-PARTY-NOTICES.html",
            "packaging/hushpen-host.service",
        ] {
            fs::copy(repo.join(file), root.join(file)).unwrap();
        }
        fs::copy(
            repo.join("assets").join(VAD_MODEL),
            root.join("assets").join(VAD_MODEL),
        )
        .unwrap();
        let artifact = build(
            &root,
            &root.join("bin"),
            &root.join("dist"),
            "2.4.0",
            Target::LinuxX64,
        )
        .unwrap();
        let unpacked = root.join("unpacked");
        fs::create_dir_all(&unpacked).unwrap();
        assert!(
            archive_tool()
                .unwrap()
                .arg("-xzf")
                .arg(&artifact)
                .arg("-C")
                .arg(&unpacked)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read(unpacked.join("hushpen-host")).unwrap(),
            b"clean program"
        );
        assert_eq!(
            fs::read(unpacked.join(NOTICES)).unwrap(),
            fs::read(repo.join("assets").join(NOTICES)).unwrap()
        );
        verify_vad(&unpacked.join(VAD_MODEL)).unwrap();
        assert!(!root.join("dist/work/host").exists());
        fs::write(root.join("assets").join(VAD_MODEL), b"wrong").unwrap();
        assert!(
            build(
                &root,
                &root.join("bin"),
                &root.join("dist"),
                "2.4.0",
                Target::LinuxX64
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
