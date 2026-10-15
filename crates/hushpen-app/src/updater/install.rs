//! How a new version gets onto the disk: what this copy is, where its staged replacement goes,
//! and the helper that swaps it in after the app quits.
//!
//! The staged file or bundle lives beside the install, on the same volume, so the swap is a
//! rename and never a copy. Nothing here replaces the running version; that is the helper's job
//! after a restart.

use crate::net::download::Progress;
use crate::net::update::{CheckError, UpdateFeed};
use crate::update_apply::Job;
use hushpen_core::error::UPDATE_SWAP_FAILED;
use hushpen_core::update::{Artifact, InstallKind};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

/// What staging produced: the complete file or bundle, and what to remove after the swap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    pub path: PathBuf,
    pub cleanup: Vec<PathBuf>,
}

/// The part of an update that touches the disk and the process list. The real one is
/// [`SystemInstaller`]; tests pass a fake.
pub trait Installer: Send + Sync {
    fn kind(&self) -> InstallKind;

    /// Downloads, checks, and stages the artifact. Blocks; the caller runs it off the UI thread.
    fn stage(
        &self,
        artifact: &Artifact,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Staged, CheckError>;

    /// Starts the helper that swaps `staged` in once this process has quit.
    fn restart(&self, staged: &Staged) -> Result<(), CheckError>;
}

pub struct SystemInstaller {
    kind: InstallKind,
    /// The AppImage file or the `.app` bundle that runs now.
    target: Option<PathBuf>,
    /// The helper writes its steps here.
    log: PathBuf,
}

/// Where an `install-kind` file can sit relative to the folder of the binary: beside it (tar.gz,
/// rpm), in `../lib/hushpen` (AppImage, deb), or in `../Resources` (macOS bundle).
const KIND_FILES: [&str; 3] = [
    "install-kind",
    "../lib/hushpen/install-kind",
    "../Resources/install-kind",
];

fn kind_beside(exe: &Path) -> InstallKind {
    let Some(dir) = exe.parent() else {
        return InstallKind::Unknown;
    };
    KIND_FILES
        .iter()
        .find_map(|name| fs::read_to_string(dir.join(name)).ok())
        .map_or(InstallKind::Unknown, |text| InstallKind::parse(&text))
}

/// The `.app` folder that holds `exe`.
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

fn swap_failed(detail: impl Into<String>) -> CheckError {
    CheckError::new(UPDATE_SWAP_FAILED, detail)
}

impl SystemInstaller {
    pub fn detect(logs_dir: PathBuf) -> Self {
        let exe = std::env::current_exe().unwrap_or_default();
        let kind = kind_beside(&exe);
        let target = match kind {
            InstallKind::AppImage => std::env::var_os("APPIMAGE").map(PathBuf::from),
            InstallKind::App => bundle_of(&exe),
            _ => None,
        };
        // An AppImage run through the runtime has APPIMAGE; without it there is no file to
        // replace, so this copy is told what it is only as far as showing a notice.
        let kind = match (kind, &target) {
            (InstallKind::AppImage | InstallKind::App, None) => InstallKind::Tar,
            (kind, _) => kind,
        };
        Self {
            kind,
            target,
            log: logs_dir.join("update-apply.log"),
        }
    }

    fn target(&self) -> Result<&Path, CheckError> {
        self.target
            .as_deref()
            .ok_or_else(|| swap_failed("this copy has no install to replace"))
    }

    /// The folder beside the install where staged files and the helper copy live.
    fn work_dir(target: &Path) -> Result<PathBuf, CheckError> {
        let parent = target
            .parent()
            .ok_or_else(|| swap_failed("the install has no folder"))?;
        let mut name = std::ffi::OsString::from(".");
        name.push(target.file_name().unwrap_or_default());
        name.push(".update");
        let dir = parent.join(name);
        fs::create_dir_all(&dir).map_err(|error| {
            swap_failed(format!(
                "cannot write beside the install ({}): {error}",
                parent.display()
            ))
        })?;
        Ok(dir)
    }
}

impl Installer for SystemInstaller {
    fn kind(&self) -> InstallKind {
        self.kind
    }

    fn stage(
        &self,
        artifact: &Artifact,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Staged, CheckError> {
        let target = self.target()?;
        let dir = Self::work_dir(target)?;
        let feed = UpdateFeed::current()?;
        let cancel = AtomicBool::new(false);
        match self.kind {
            InstallKind::AppImage => {
                let staged = dir.join("staged.AppImage");
                download_once(&feed, artifact, &staged, &cancel, progress)?;
                Ok(Staged {
                    path: staged,
                    cleanup: vec![dir],
                })
            }
            InstallKind::App => {
                let zip = dir.join("update.zip");
                download_once(&feed, artifact, &zip, &cancel, progress)?;
                let bundle = unpack_bundle(&zip, &dir)?;
                Ok(Staged {
                    path: bundle,
                    cleanup: vec![dir],
                })
            }
            other => Err(swap_failed(format!(
                "{} installs are not replaced by the app",
                other.key()
            ))),
        }
    }

    fn restart(&self, staged: &Staged) -> Result<(), CheckError> {
        let target = self.target()?;
        let exe = std::env::current_exe()
            .map_err(|error| swap_failed(format!("cannot find this program: {error}")))?;
        let helper_dir = Self::work_dir(target)?.join(format!("helper-{}", std::process::id()));
        let helper = copy_helper(self.kind, &exe, &helper_dir)?;
        let mut cleanup = staged.cleanup.clone();
        cleanup.push(helper_dir);
        let job = Job {
            pid: std::process::id(),
            kind: self.kind,
            target: target.to_path_buf(),
            staged: staged.path.clone(),
            relaunch: true,
            cleanup,
        };
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
            .map(Stdio::from)
            .unwrap_or_else(|_| Stdio::null());
        let mut command = Command::new(helper);
        command
            .arg("update-apply")
            .args(job.to_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // Its own process group, so the end of the app takes nothing with it.
            command.process_group(0);
        }
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| swap_failed(format!("cannot start the update helper: {error}")))
    }
}

/// Downloads into `dest` unless a file with the right hash is already there from an earlier run.
fn download_once(
    feed: &UpdateFeed,
    artifact: &Artifact,
    dest: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), CheckError> {
    if dest.is_file() {
        let same = hushpen_store::model_files::hash_file(dest, |_| true)
            .is_ok_and(|hash| hash == artifact.sha256);
        if same {
            return Ok(());
        }
        let _ = fs::remove_file(dest);
    }
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    feed.download(artifact, Path::new(&part), dest, cancel, progress)
        .map(|_| ())
}

/// Unzips the macOS zip beside it and returns the verified `.app` inside.
fn unpack_bundle(zip: &Path, dir: &Path) -> Result<PathBuf, CheckError> {
    let out = dir.join("unpacked");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).map_err(|error| swap_failed(format!("cannot unpack: {error}")))?;
    run(
        Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(zip)
            .arg(&out),
        "ditto",
    )?;
    let bundle = fs::read_dir(&out)
        .map_err(|error| swap_failed(format!("cannot read the unpacked update: {error}")))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .ok_or_else(|| swap_failed("the update holds no app"))?;
    run(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&bundle),
        "codesign",
    )?;
    Ok(bundle)
}

fn run(command: &mut Command, what: &str) -> Result<(), CheckError> {
    let output = command
        .output()
        .map_err(|error| swap_failed(format!("cannot run {what}: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(swap_failed(format!(
            "{what} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// A copy of this program that can run after the AppImage mount or the old bundle is gone. An
/// AppImage binary finds its own libraries through `$ORIGIN/../lib`, so those come along.
fn copy_helper(kind: InstallKind, exe: &Path, dir: &Path) -> Result<PathBuf, CheckError> {
    let bin = dir.join("bin");
    fs::create_dir_all(&bin)
        .map_err(|error| swap_failed(format!("cannot make the helper: {error}")))?;
    let helper = bin.join(exe.file_name().unwrap_or_default());
    fs::copy(exe, &helper)
        .map_err(|error| swap_failed(format!("cannot copy the helper: {error}")))?;
    if kind == InstallKind::AppImage {
        let lib = dir.join("lib");
        fs::create_dir_all(&lib)
            .map_err(|error| swap_failed(format!("cannot make the helper: {error}")))?;
        let from = exe.parent().map(|bin| bin.join("../lib"));
        for entry in from
            .and_then(|from| fs::read_dir(from).ok())
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.is_file() {
                let _ = fs::copy(&path, lib.join(entry.file_name()));
            }
        }
    }
    Ok(helper)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn the_install_kind_is_found_where_each_package_puts_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(&root.join("tar/hushpen"), "");
        write(&root.join("tar/install-kind"), "tar\n");
        assert_eq!(kind_beside(&root.join("tar/hushpen")), InstallKind::Tar);

        write(&root.join("image/usr/bin/hushpen"), "");
        write(
            &root.join("image/usr/lib/hushpen/install-kind"),
            "appimage\n",
        );
        assert_eq!(
            kind_beside(&root.join("image/usr/bin/hushpen")),
            InstallKind::AppImage
        );

        write(&root.join("Hushpen.app/Contents/MacOS/hushpen"), "");
        write(
            &root.join("Hushpen.app/Contents/Resources/install-kind"),
            "app\n",
        );
        assert_eq!(
            kind_beside(&root.join("Hushpen.app/Contents/MacOS/hushpen")),
            InstallKind::App
        );

        write(&root.join("source/hushpen"), "");
        assert_eq!(
            kind_beside(&root.join("source/hushpen")),
            InstallKind::Unknown,
            "no file means a source tree"
        );
    }

    #[test]
    fn the_bundle_is_the_app_folder_around_the_binary() {
        let exe = Path::new("/Applications/Hushpen.app/Contents/MacOS/hushpen");
        assert_eq!(
            bundle_of(exe),
            Some(PathBuf::from("/Applications/Hushpen.app"))
        );
        assert_eq!(bundle_of(Path::new("/usr/bin/hushpen")), None);
    }

    #[test]
    fn the_helper_copy_of_an_appimage_binary_brings_the_libraries_it_loads_by_origin() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("mount/usr/bin/hushpen");
        write(&exe, "binary");
        write(&dir.path().join("mount/usr/lib/libxkbcommon.so.0"), "lib");
        write(
            &dir.path().join("mount/usr/lib/hushpen/install-kind"),
            "appimage\n",
        );

        let helper_dir = dir.path().join("helper");
        let helper = copy_helper(InstallKind::AppImage, &exe, &helper_dir).unwrap();
        assert_eq!(helper, helper_dir.join("bin/hushpen"));
        assert_eq!(fs::read_to_string(&helper).unwrap(), "binary");
        assert_eq!(
            fs::read_to_string(helper_dir.join("lib/libxkbcommon.so.0")).unwrap(),
            "lib"
        );
        assert!(
            !helper_dir.join("lib/hushpen").exists(),
            "folders stay behind"
        );

        let bundle_helper = dir.path().join("bundle-helper");
        copy_helper(InstallKind::App, &exe, &bundle_helper).unwrap();
        assert!(!bundle_helper.join("lib").exists());
    }

    #[test]
    fn staging_next_to_a_folder_that_cannot_be_written_fails_with_the_swap_code() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        write(&blocker, "x");
        // The folder of the install is a file, so nothing can be staged beside it.
        let error = SystemInstaller::work_dir(&blocker.join("Hushpen.AppImage")).unwrap_err();
        assert_eq!(error.code, UPDATE_SWAP_FAILED);
    }

    #[test]
    fn a_copy_without_a_file_to_replace_cannot_stage_or_restart() {
        let installer = SystemInstaller {
            kind: InstallKind::AppImage,
            target: None,
            log: PathBuf::from("/nonexistent/update-apply.log"),
        };
        let artifact = Artifact {
            url: "http://127.0.0.1:1/x".into(),
            sha256: "00".repeat(32),
            size: 1,
        };
        assert_eq!(
            installer.stage(&artifact, &mut |_| {}).unwrap_err().code,
            UPDATE_SWAP_FAILED
        );
        let staged = Staged {
            path: "/x".into(),
            cleanup: vec![],
        };
        assert_eq!(
            installer.restart(&staged).unwrap_err().code,
            UPDATE_SWAP_FAILED
        );
    }
}
