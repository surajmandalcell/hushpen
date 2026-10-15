//! macOS: the `.app` through cargo-packager, an ad-hoc signature, the zip with
//! `ditto`, and (on the CI runner only) the dmg.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::names::Format;
use super::{Ctx, config, first_file_with_extension, run_packager, run_tool};

const APP_NAME: &str = "Hushpen.app";

/// Ad-hoc only: the identity `-` needs no certificate and no Keychain.
pub fn sign_adhoc(app: &Path) -> Result<(), String> {
    run_tool(
        Command::new("codesign")
            .args(["--force", "--deep", "-s", "-"])
            .arg(app),
        "codesign (ad-hoc)",
    )?;
    run_tool(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(app),
        "codesign --verify --deep --strict",
    )
}

fn make_zip(app: &Path, zip: &Path) -> Result<(), String> {
    if zip.exists() {
        fs::remove_file(zip).map_err(|e| format!("cannot replace {}: {e}", zip.display()))?;
    }
    run_tool(
        Command::new("ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(app)
            .arg(zip),
        "ditto -c -k --keepParent",
    )
}

pub fn build(ctx: &Ctx, formats: &[Format]) -> Result<Vec<PathBuf>, String> {
    let work = ctx.work_dir("mac")?;
    let resources = ctx.resources(&work, "app")?;
    let icons = [ctx.root.join("assets/asrpro-app-icon.icns")];
    let info_plist = ctx.root.join("packaging/Info.plist");
    let sign_script = work.join("sign-before-dmg.sh");
    fs::write(&sign_script, config::sign_script(&work.join(APP_NAME)))
        .map_err(|e| format!("cannot write {}: {e}", sign_script.display()))?;

    let input = config::Common {
        version: &ctx.version,
        work: &work,
        icons: &icons,
        resources: &resources,
    };
    let config_path = work.join("packager.toml");
    fs::write(&config_path, config::mac(&input, &info_plist, &sign_script))
        .map_err(|e| format!("cannot write {}: {e}", config_path.display()))?;

    let want_dmg = formats.contains(&Format::Dmg);
    run_packager(
        &ctx.root,
        &config_path,
        if want_dmg { "app,dmg" } else { "app" },
        &work,
    )?;

    let app = work.join(APP_NAME);
    sign_adhoc(&app)?;

    let mut artifacts = Vec::new();
    if formats.contains(&Format::Zip) {
        let zip = ctx.artifact(Format::Zip);
        make_zip(&app, &zip)?;
        artifacts.push(zip);
    }
    if want_dmg {
        let made = first_file_with_extension(&work, ".dmg")?;
        let dmg = ctx.artifact(Format::Dmg);
        fs::rename(&made, &dmg).map_err(|e| format!("cannot move the dmg: {e}"))?;
        artifacts.push(dmg);
    }
    Ok(artifacts)
}
