//! Linux: AppImage and deb through cargo-packager, rpm through
//! cargo-generate-rpm, and the tar.gz from here.
//!
//! The deb and rpm keep both binaries together in `/usr/lib/hushpen/`, because
//! the app starts its engine and AI worker from the folder of its own
//! executable. `/usr/bin/hushpen` points there.

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::deps::{self, Deps};
use super::names::{self, Format};
use super::{APP_BINARY, Ctx, LLM_BINARY, NOTICES, config, copy, first_file_with_extension};
use super::{run_packager, run_tool};

const ICON_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256, 512];
const LIB_DIR: &str = "usr/lib/hushpen";

fn icon_paths(ctx: &Ctx) -> Vec<PathBuf> {
    ICON_SIZES
        .iter()
        .map(|size| {
            ctx.root
                .join(format!("assets/linux-icons/{size}x{size}.png"))
        })
        .collect()
}

fn derive_deps(ctx: &Ctx) -> Result<Deps, String> {
    let binary = ctx.binaries.join(APP_BINARY);
    let bytes = fs::read(&binary).map_err(|e| format!("cannot read {}: {e}", binary.display()))?;
    let deps = deps::derive(&deps::needed_of(&binary)?, &bytes)?;
    println!("deb Depends: {}", deps.deb_depends.join(", "));
    println!("rpm Requires: {}", deps.rpm_requires.join(", "));
    Ok(deps)
}

pub fn build(ctx: &Ctx, formats: &[Format]) -> Result<Vec<PathBuf>, String> {
    let deps = derive_deps(ctx)?;
    let mut artifacts = Vec::new();
    for format in formats {
        let artifact = match format {
            Format::AppImage => appimage(ctx)?,
            Format::Deb => deb(ctx, &deps)?,
            Format::Rpm => rpm(ctx, &deps)?,
            Format::TarGz => tar_gz(ctx)?,
            other => return Err(format!("{other:?} is not a Linux format")),
        };
        artifacts.push(artifact);
    }
    Ok(artifacts)
}

fn packager_run(
    ctx: &Ctx,
    name: &str,
    kind: &str,
    render: impl Fn(&config::Common) -> String,
    format: &str,
) -> Result<PathBuf, String> {
    let work = ctx.work_dir(name)?;
    let resources = ctx.resources(&work, kind)?;
    let icons = icon_paths(ctx);
    let input = config::Common {
        version: &ctx.version,
        work: &work,
        icons: &icons,
        resources: &resources,
    };
    let config_path = work.join("packager.toml");
    fs::write(&config_path, render(&input))
        .map_err(|e| format!("cannot write {}: {e}", config_path.display()))?;
    run_packager(&ctx.root, &config_path, format, &work)?;
    Ok(work)
}

fn appimage(ctx: &Ctx) -> Result<PathBuf, String> {
    let work = packager_run(ctx, "appimage", "appimage", config::appimage, "appimage")?;
    let made = first_file_with_extension(&work, ".AppImage")?;
    let artifact = ctx.artifact(Format::AppImage);
    fs::rename(&made, &artifact).map_err(|e| format!("cannot move the AppImage: {e}"))?;
    Ok(artifact)
}

fn deb(ctx: &Ctx, deps: &Deps) -> Result<PathBuf, String> {
    let work = packager_run(ctx, "deb", "deb", |input| config::deb(input, deps), "deb")?;
    let made = first_file_with_extension(&work, ".deb")?;
    let artifact = ctx.artifact(Format::Deb);
    relayout_deb(&made, &artifact, &work.join("tree"), deps)?;
    Ok(artifact)
}

/// cargo-packager puts every binary in `/usr/bin`. Unpack its deb, move the
/// binaries beside the notices in `/usr/lib/hushpen`, link `/usr/bin/hushpen`,
/// add the Recommends line it cannot write, and pack the result again.
fn relayout_deb(made: &Path, out: &Path, tree: &Path, deps: &Deps) -> Result<(), String> {
    run_tool(
        Command::new("dpkg-deb").arg("-R").arg(made).arg(tree),
        "dpkg-deb -R",
    )?;
    let bin = tree.join("usr/bin");
    let lib = tree.join(LIB_DIR);
    fs::create_dir_all(&lib).map_err(|e| format!("cannot create {}: {e}", lib.display()))?;
    for binary in [APP_BINARY, LLM_BINARY] {
        let to = lib.join(binary);
        fs::rename(bin.join(binary), &to)
            .map_err(|e| format!("cannot move {binary} into {}: {e}", lib.display()))?;
        fs::set_permissions(&to, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("cannot set the mode of {}: {e}", to.display()))?;
    }
    symlink("../lib/hushpen/hushpen", bin.join(APP_BINARY))
        .map_err(|e| format!("cannot link /usr/bin/hushpen: {e}"))?;

    let control_path = tree.join("DEBIAN/control");
    let control = fs::read_to_string(&control_path)
        .map_err(|e| format!("cannot read {}: {e}", control_path.display()))?;
    fs::write(
        &control_path,
        add_recommends(&control, &deps.deb_recommends),
    )
    .map_err(|e| format!("cannot write {}: {e}", control_path.display()))?;

    write_md5sums(tree)?;
    run_tool(
        Command::new("dpkg-deb")
            .args(["--root-owner-group", "-Zxz", "-b"])
            .arg(tree)
            .arg(out),
        "dpkg-deb -b",
    )
}

/// Inserts `Recommends:` after the `Depends:` line of a control file.
pub fn add_recommends(control: &str, recommends: &[String]) -> String {
    let line = format!("Recommends: {}\n", recommends.join(", "));
    let mut out = String::new();
    let mut placed = false;
    for existing in control.split_inclusive('\n') {
        if !placed && existing.starts_with("Description:") {
            out.push_str(&line);
            placed = true;
        }
        out.push_str(existing);
    }
    if !placed {
        out.push_str(&line);
    }
    out
}

fn write_md5sums(tree: &Path) -> Result<(), String> {
    let output = Command::new("sh")
        .current_dir(tree)
        .args([
            "-c",
            "find . -path ./DEBIAN -prune -o -type f -print | sed 's|^\\./||' | sort | xargs md5sum",
        ])
        .output()
        .map_err(|e| format!("cannot run md5sum: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "md5sum failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    fs::write(tree.join("DEBIAN/md5sums"), output.stdout)
        .map_err(|e| format!("cannot write md5sums: {e}"))
}

fn rpm(ctx: &Ctx, deps: &Deps) -> Result<PathBuf, String> {
    let work = ctx.work_dir("rpm")?;
    let resources = ctx.resources(&work, "rpm")?;
    let launcher = work.join("hushpen-launcher");
    let desktop = work.join("hushpen.desktop");
    fs::write(&launcher, config::launcher_script())
        .map_err(|e| format!("cannot write the launcher: {e}"))?;
    fs::write(&desktop, config::desktop_entry())
        .map_err(|e| format!("cannot write the desktop file: {e}"))?;

    let mut assets: Vec<(PathBuf, String, &str)> = vec![
        (
            work.join(APP_BINARY),
            "/usr/lib/hushpen/hushpen".into(),
            "755",
        ),
        (
            work.join(LLM_BINARY),
            "/usr/lib/hushpen/hushpen-llm".into(),
            "755",
        ),
        (launcher, "/usr/bin/hushpen".into(), "755"),
        (
            desktop,
            "/usr/share/applications/hushpen.desktop".into(),
            "644",
        ),
        (
            ctx.root.join("LICENSE"),
            "/usr/share/licenses/hushpen/LICENSE".into(),
            "644",
        ),
    ];
    for (source, target) in resources {
        assets.push((source, format!("/usr/lib/hushpen/{target}"), "644"));
    }
    for size in ICON_SIZES {
        assets.push((
            ctx.root
                .join(format!("assets/linux-icons/{size}x{size}.png")),
            format!("/usr/share/icons/hicolor/{size}x{size}/apps/hushpen.png"),
            "644",
        ));
    }

    let metadata = work.join("rpm-metadata.toml");
    fs::write(&metadata, config::rpm_metadata(&ctx.version, &assets, deps))
        .map_err(|e| format!("cannot write {}: {e}", metadata.display()))?;

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    run_tool(
        Command::new(cargo)
            .current_dir(&ctx.root)
            .args([
                "generate-rpm",
                "-p",
                "crates/hushpen-app",
                "--auto-req",
                "disabled",
            ])
            .args(["-a", ctx.target.rpm_arch()])
            .arg("--metadata-overwrite")
            .arg(&metadata)
            .arg("-o")
            .arg(&work),
        "cargo generate-rpm",
    )?;
    let made = first_file_with_extension(&work, ".rpm")?;
    let artifact = ctx.artifact(Format::Rpm);
    fs::rename(&made, &artifact).map_err(|e| format!("cannot move the rpm: {e}"))?;
    Ok(artifact)
}

fn tar_gz(ctx: &Ctx) -> Result<PathBuf, String> {
    let work = ctx.work_dir("tar")?;
    let top_name = names::tar_top_folder(&ctx.version, ctx.target);
    let top = work.join(&top_name);
    let icons = top.join("icons");
    fs::create_dir_all(&icons).map_err(|e| format!("cannot create {}: {e}", icons.display()))?;
    for binary in [APP_BINARY, LLM_BINARY] {
        copy(&work.join(binary), &top.join(binary))?;
    }
    copy(&ctx.notices(), &top.join(NOTICES))?;
    copy(&ctx.root.join("LICENSE"), &top.join("LICENSE"))?;
    fs::write(top.join("install-kind"), "tar\n")
        .map_err(|e| format!("cannot write install-kind: {e}"))?;
    for icon in icon_paths(ctx) {
        let name = icon.file_name().ok_or("icon has no name")?;
        copy(&icon, &icons.join(name))?;
    }

    let artifact = ctx.artifact(Format::TarGz);
    run_tool(
        Command::new("tar")
            .args(["--sort=name", "--owner=0", "--group=0", "--numeric-owner"])
            .arg("-czf")
            .arg(&artifact)
            .arg("-C")
            .arg(&work)
            .arg(&top_name),
        "tar -czf",
    )?;
    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommends_goes_before_the_description_after_depends() {
        let control = "Package: hushpen\nDepends: libvulkan1\nDescription: Private speech\n more\n";
        let out = add_recommends(control, &["xdg-desktop-portal".into(), "x-gtk".into()]);
        assert_eq!(
            out,
            "Package: hushpen\nDepends: libvulkan1\nRecommends: xdg-desktop-portal, x-gtk\nDescription: Private speech\n more\n"
        );
    }

    #[test]
    fn recommends_is_appended_when_there_is_no_description() {
        let out = add_recommends("Package: hushpen\n", &["a".into()]);
        assert!(out.ends_with("Recommends: a\n"));
    }
}
