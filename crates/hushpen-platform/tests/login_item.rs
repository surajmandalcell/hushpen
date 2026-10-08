use hushpen_platform::login_item;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

fn os(value: &str) -> Option<&OsStr> {
    Some(OsStr::new(value))
}

#[test]
fn the_autostart_folder_follows_xdg_config_home() {
    let dir = login_item::autostart_dir(os("/tmp/xdg-config"), Some(Path::new("/home/u")));
    assert_eq!(dir, PathBuf::from("/tmp/xdg-config/autostart"));
}

#[test]
fn the_autostart_folder_falls_back_to_dot_config() {
    let home = Some(Path::new("/home/u"));
    assert_eq!(
        login_item::autostart_dir(None, home),
        PathBuf::from("/home/u/.config/autostart")
    );
    // An empty or relative XDG_CONFIG_HOME is ignored, like XDG_DATA_HOME.
    assert_eq!(
        login_item::autostart_dir(os(""), home),
        PathBuf::from("/home/u/.config/autostart")
    );
    assert_eq!(
        login_item::autostart_dir(os("relative"), home),
        PathBuf::from("/home/u/.config/autostart")
    );
}

#[test]
fn the_entry_exec_points_at_the_binary_with_the_hidden_flag() {
    let text = login_item::entry_text(Path::new("/opt/hushpen/hushpen"), &["--hidden"]);
    assert!(text.starts_with("[Desktop Entry]\n"), "{text}");
    assert!(text.contains("Type=Application\n"), "{text}");
    assert!(text.contains("Name=Hushpen\n"), "{text}");
    assert!(
        text.contains("Exec=/opt/hushpen/hushpen --hidden\n"),
        "{text}"
    );
    assert!(text.contains("Terminal=false\n"), "{text}");
}

#[cfg(target_os = "linux")]
#[test]
fn enable_writes_and_disable_removes_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("autostart").join("hushpen.desktop");
    let item = login_item::LinuxLoginItem::new(file.clone(), PathBuf::from("/bin/hushpen"));
    assert!(!item.is_enabled());
    item.set(true).unwrap();
    assert!(item.is_enabled());
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("Exec=/bin/hushpen --hidden\n"), "{text}");
    item.set(false).unwrap();
    assert!(!item.is_enabled());
    assert!(!file.exists());
    // Disabling twice is not an error.
    item.set(false).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn a_removed_file_reads_as_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("autostart").join("hushpen.desktop");
    let item = login_item::LinuxLoginItem::new(file.clone(), PathBuf::from("/bin/hushpen"));
    item.set(true).unwrap();
    std::fs::remove_file(&file).unwrap();
    assert!(!item.is_enabled());
}
