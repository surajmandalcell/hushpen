//! Per-user NSIS setup and the portable ZIP carry the same native programs.

use std::fs;
use std::path::PathBuf;

use super::names::Format;
use super::{BINARIES, Ctx, archive_tool, config, copy, run_packager, run_tool};

#[path = "../../../packaging/windows/icon.rs"]
mod icon;

pub fn build(ctx: &Ctx, formats: &[Format]) -> Result<Vec<PathBuf>, String> {
    let mut artifacts = Vec::new();
    for format in formats {
        let kind = match format {
            Format::Nsis => "windows-setup",
            Format::Zip => "windows-zip",
            _ => return Err(format!("unsupported Windows format {format:?}")),
        };
        let work = ctx.work_dir(kind)?;
        let mut resources = ctx.resources(&work, kind)?;
        for lib in ctx.tts_libs() {
            let name = lib
                .file_name()
                .ok_or("runtime library has no name")?
                .to_string_lossy()
                .into_owned();
            resources.push((lib, name));
        }
        let artifact = ctx.artifact(*format);
        if artifact.exists() {
            fs::remove_file(&artifact)
                .map_err(|e| format!("cannot replace {}: {e}", artifact.display()))?;
        }
        if *format == Format::Nsis {
            let icon_path = work.join("Hushpen.ico");
            icon::write(&ctx.root.join("assets"), &icon_path)?;
            let icons = [icon_path];
            let input = config::Common {
                version: &ctx.version,
                work: &work,
                icons: &icons,
                resources: &resources,
                tts_libs: &[],
            };
            let config_path = work.join("packager.toml");
            fs::write(
                &config_path,
                config::windows(&input, &ctx.root.join("packaging/windows/installer.nsi")),
            )
            .map_err(|e| format!("cannot write Windows packager config: {e}"))?;
            run_packager(&ctx.root, &config_path, "nsis", &work)?;
            let made = super::first_file_with_extension(&work, "-setup.exe")?;
            fs::rename(made, &artifact).map_err(|e| format!("cannot move Windows setup: {e}"))?;
        } else {
            let portable = work.join("portable");
            fs::create_dir_all(&portable)
                .map_err(|e| format!("cannot create portable folder: {e}"))?;
            for binary in BINARIES {
                let name = ctx.binary_name(binary);
                copy(&work.join(&name), &portable.join(&name))?;
            }
            for (source, name) in &resources {
                copy(source, &portable.join(name))?;
            }
            run_tool(
                archive_tool()?
                    .args(["-a", "-cf"])
                    .arg(&artifact)
                    .arg("-C")
                    .arg(&portable)
                    .arg("."),
                "tar (portable zip)",
            )?;
        }
        artifacts.push(artifact);
    }
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn ico_wraps_every_existing_png_without_changing_it() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let output = std::env::temp_dir().join(format!("hushpen-icon-{}.ico", std::process::id()));
        icon::write(&root.join("assets"), &output).unwrap();
        let ico = fs::read(&output).unwrap();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 7, 0]);
        for (index, size) in [16_u16, 24, 32, 48, 64, 128, 256].into_iter().enumerate() {
            let entry = &ico[6 + index * 16..6 + (index + 1) * 16];
            let len = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            let png = fs::read(root.join(format!("assets/linux-icons/{size}x{size}.png"))).unwrap();
            assert_eq!(&ico[offset..offset + len], png);
        }
        fs::remove_file(output).unwrap();
    }
}
