//! Launch at login. macOS registers the app with SMAppService; Linux writes
//! an XDG autostart file. The file's `Exec` line carries `--hidden`, so a
//! login start lands in the tray without a window.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The flag a login start carries.
pub const HIDDEN_ARG: &str = "--hidden";

/// Where the autostart file goes: `$XDG_CONFIG_HOME/autostart`, falling back
/// to `~/.config/autostart`. An empty or relative XDG value is ignored.
pub fn autostart_dir(xdg_config_home: Option<&OsStr>, home: Option<&Path>) -> PathBuf {
    let config = xdg_config_home
        .map(Path::new)
        .filter(|dir| !dir.as_os_str().is_empty() && dir.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| home.map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    config.join("autostart")
}

/// The desktop entry text. `Exec` is the binary plus the hidden flag.
pub fn entry_text(exec: &Path, args: &[&str]) -> String {
    let mut line = exec.display().to_string();
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=Hushpen\n\
         Comment=Private dictation, ready at login\n\
         Exec={line}\n\
         Icon=hushpen\n\
         StartupNotify=false\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

/// Launch at login for this build. `exec` is the running binary.
#[cfg(target_os = "linux")]
pub type LoginItem = LinuxLoginItem;

/// Launch at login for this build. `exec` is the running binary.
#[cfg(target_os = "macos")]
pub type LoginItem = MacLoginItem;

/// The XDG autostart file.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub struct LinuxLoginItem {
    file: PathBuf,
    exec: PathBuf,
}

#[cfg(target_os = "linux")]
impl LinuxLoginItem {
    pub fn new(file: PathBuf, exec: PathBuf) -> Self {
        Self { file, exec }
    }

    /// The login item of the running binary.
    pub fn current() -> std::io::Result<Self> {
        let exec = std::env::current_exe()?;
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let dir = autostart_dir(
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            home.as_deref(),
        );
        Ok(Self::new(dir.join("hushpen.desktop"), exec))
    }

    pub fn is_enabled(&self) -> bool {
        self.file.exists()
    }

    pub fn set(&self, enabled: bool) -> std::io::Result<()> {
        if enabled {
            if let Some(dir) = self.file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&self.file, entry_text(&self.exec, &[HIDDEN_ARG]))
        } else {
            match std::fs::remove_file(&self.file) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos;

/// SMAppService on macOS 13+. An unsigned debug build may fail to register;
/// the error text then tells the user to allow the item in System Settings.
#[cfg(target_os = "macos")]
pub use macos::MacLoginItem;
