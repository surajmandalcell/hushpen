use super::*;
use std::os::unix::fs::PermissionsExt;

fn job() -> Job {
    Job {
        pid: 4242,
        kind: InstallKind::AppImage,
        target: "/opt/Hushpen.AppImage".into(),
        staged: "/opt/.Hushpen.AppImage.update".into(),
        relaunch: true,
        cleanup: vec!["/opt/.stage".into(), "/opt/.helper".into()],
    }
}

fn args(job: &Job) -> Vec<String> {
    job.to_args()
}

fn file(path: &Path, text: &str, mode: u32) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn a_job_survives_the_trip_through_the_command_line() {
    let job = job();
    assert_eq!(parse_args(&args(&job)).unwrap(), job);
    let plain = Job {
        relaunch: false,
        cleanup: vec![],
        kind: InstallKind::App,
        ..job
    };
    assert_eq!(parse_args(&args(&plain)).unwrap(), plain);
}

#[test]
fn a_command_line_with_a_missing_or_wrong_part_is_refused() {
    let words = |list: &[&str]| list.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
    assert!(parse_args(&[]).is_err());
    assert!(
        parse_args(&words(&[
            "--pid", "1", "--kind", "deb", "--target", "a", "--staged", "b"
        ]))
        .is_err()
    );
    assert!(
        parse_args(&words(&[
            "--pid", "x", "--kind", "app", "--target", "a", "--staged", "b"
        ]))
        .is_err()
    );
    assert!(parse_args(&words(&["--pid", "1", "--kind", "app", "--target", "a"])).is_err());
    assert!(parse_args(&words(&["--bogus"])).is_err());
}

#[test]
fn a_swapped_file_is_the_new_one_with_the_mode_of_the_old_and_no_leftover() {
    let dir = tempfile::tempdir().unwrap();
    let (target, staged) = (dir.path().join("Hushpen.AppImage"), dir.path().join("new"));
    file(&target, "old", 0o755);
    file(&staged, "new", 0o600);
    swap_file(&target, &staged).unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert!(!staged.exists());
    assert!(!backup_path(&target).exists());
}

#[test]
fn a_kill_in_the_middle_of_a_file_swap_leaves_the_whole_old_file_on_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let (target, staged) = (dir.path().join("Hushpen.AppImage"), dir.path().join("new"));
    file(&target, "old version", 0o755);
    file(&staged, "new version", 0o755);
    let mut seen = None;
    swap_file_with(&target, &staged, || {
        // The helper could be killed right here: this is everything the folder holds then.
        let mut names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        seen = Some((fs::read_to_string(&target).unwrap(), names));
    })
    .unwrap();
    let (content, names) = seen.unwrap();
    assert_eq!(
        content, "old version",
        "the path holds the old file until the rename"
    );
    assert_eq!(
        names,
        ["Hushpen.AppImage", "Hushpen.AppImage.old", "new"],
        "no partial file, only the whole files"
    );
}

#[test]
fn a_file_swap_that_fails_keeps_the_old_file_and_leaves_no_second_copy() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Hushpen.AppImage");
    file(&target, "old", 0o755);
    let missing = dir.path().join("never-downloaded");
    let error = swap_file(&target, &missing).unwrap_err();
    assert!(!error.0.is_empty());
    assert_eq!(fs::read_to_string(&target).unwrap(), "old");
    assert!(!backup_path(&target).exists());
}

#[test]
fn a_file_swap_whose_final_rename_fails_after_the_old_file_is_kept_restores_it() {
    let dir = tempfile::tempdir().unwrap();
    let (target, staged) = (dir.path().join("Hushpen.AppImage"), dir.path().join("new"));
    file(&target, "old", 0o755);
    file(&staged, "new", 0o755);
    let error = swap_file_with(&target, &staged, || {
        // The new file disappears before the rename, as if the disk lost it.
        fs::remove_file(&staged).unwrap();
    })
    .unwrap_err();
    assert!(error.0.contains("cannot put the new file in place"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "old");
    assert!(!backup_path(&target).exists(), "the second name is removed");
}

fn bundle(path: &Path, version: &str) {
    fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
    file(&path.join("Contents/MacOS/hushpen"), version, 0o755);
}

fn version_of(path: &Path) -> String {
    fs::read_to_string(path.join("Contents/MacOS/hushpen")).unwrap()
}

#[test]
fn a_bundle_swap_puts_the_new_folder_on_the_path_and_removes_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (target, staged) = (dir.path().join("Hushpen.app"), dir.path().join("stage.app"));
    bundle(&target, "rc.1");
    bundle(&staged, "rc.2");
    swap_bundle(&target, &staged).unwrap();
    assert_eq!(version_of(&target), "rc.2");
    assert!(!staged.exists());
    assert!(!backup_path(&target).exists());
}

#[test]
fn a_bundle_swap_that_fails_puts_the_old_bundle_back_whole() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Hushpen.app");
    bundle(&target, "rc.1");
    let error = swap_bundle(&target, &dir.path().join("missing.app")).unwrap_err();
    assert!(error.0.contains("cannot put the new app in place"));
    assert_eq!(version_of(&target), "rc.1");
    assert!(!backup_path(&target).exists());
}

#[test]
fn only_an_app_or_an_appimage_is_swapped() {
    let dir = tempfile::tempdir().unwrap();
    let error = swap(InstallKind::Deb, dir.path(), dir.path()).unwrap_err();
    assert!(error.0.contains("deb"));
}

#[test]
fn this_process_is_alive_and_a_collected_child_is_not() {
    assert!(process_alive(std::process::id()));
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    assert!(!process_alive(pid));
}

#[cfg(target_os = "linux")]
#[test]
fn a_child_nobody_collected_counts_as_gone() {
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    let start = Instant::now();
    while process_alive(pid) && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!process_alive(pid), "a zombie is not a running app");
    child.wait().unwrap();
}

fn exited_pid() -> u32 {
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

#[test]
fn a_job_swaps_after_the_app_quit_and_removes_what_it_was_told_to() {
    let dir = tempfile::tempdir().unwrap();
    let (target, staged) = (dir.path().join("Hushpen.AppImage"), dir.path().join("new"));
    let extra = dir.path().join("stage-folder");
    fs::create_dir_all(extra.join("inner")).unwrap();
    file(&target, "old", 0o755);
    file(&staged, "new", 0o755);
    run(&Job {
        pid: exited_pid(),
        kind: InstallKind::AppImage,
        target: target.clone(),
        staged,
        relaunch: false,
        cleanup: vec![extra.clone()],
    })
    .unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    assert!(!extra.exists());
}

#[test]
fn a_job_that_cannot_swap_reports_the_error_keeps_the_old_file_and_still_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Hushpen.AppImage");
    let extra = dir.path().join("stage-folder");
    fs::create_dir_all(&extra).unwrap();
    file(&target, "old", 0o755);
    let result = run(&Job {
        pid: exited_pid(),
        kind: InstallKind::AppImage,
        target: target.clone(),
        staged: dir.path().join("not-there"),
        relaunch: false,
        cleanup: vec![extra.clone()],
    });
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "old");
    assert!(!extra.exists());
}

/// Needs the real `codesign`, `ditto`, and `plutil`; it opens no window and runs no bundle.
#[cfg(target_os = "macos")]
mod mac {
    use super::*;

    fn run_tool(program: &str, args: &[&str]) -> std::process::Output {
        let output = Command::new(program).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{program} {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn signed_bundle(path: &Path, version: &str) {
        fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
        fs::copy("/usr/bin/true", path.join("Contents/MacOS/hushpen")).unwrap();
        fs::write(
            path.join("Contents/Info.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
<key>CFBundleExecutable</key><string>hushpen</string>\
<key>CFBundleIdentifier</key><string>app.hushpen.swaptest</string>\
<key>CFBundlePackageType</key><string>APPL</string>\
<key>CFBundleShortVersionString</key><string>{version}</string>\
<key>CFBundleVersion</key><string>{version}</string></dict></plist>\n"
            ),
        )
        .unwrap();
        run_tool(
            "codesign",
            &["--force", "--deep", "-s", "-", path.to_str().unwrap()],
        );
    }

    fn version_text(path: &Path) -> String {
        let plist = path.join("Contents/Info.plist");
        let output = run_tool(
            "plutil",
            &[
                "-extract",
                "CFBundleShortVersionString",
                "raw",
                "-o",
                "-",
                plist.to_str().unwrap(),
            ],
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn verifies(path: &Path) -> bool {
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict", path.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    }

    #[test]
    fn a_signed_rc2_bundle_swaps_over_a_signed_rc1_bundle_and_still_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let (target, staged) = (
            dir.path().join("Hushpen.app"),
            dir.path().join("stage/Hushpen.app"),
        );
        fs::create_dir_all(staged.parent().unwrap()).unwrap();
        signed_bundle(&target, "2.0.0-rc.1");
        signed_bundle(&staged, "2.0.0-rc.2");
        assert_eq!(version_text(&target), "2.0.0-rc.1");
        swap(InstallKind::App, &target, &staged).unwrap();
        assert_eq!(version_text(&target), "2.0.0-rc.2");
        assert!(verifies(&target), "codesign --verify --deep --strict");
        assert!(!backup_path(&target).exists());
    }

    #[test]
    fn a_failed_bundle_swap_leaves_the_rc1_bundle_intact_and_valid() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Hushpen.app");
        signed_bundle(&target, "2.0.0-rc.1");
        let error = swap(
            InstallKind::App,
            &target,
            &dir.path().join("missing/Hushpen.app"),
        )
        .unwrap_err();
        assert!(!error.0.is_empty());
        assert_eq!(version_text(&target), "2.0.0-rc.1");
        assert!(verifies(&target));
        assert!(!backup_path(&target).exists());
    }
}
