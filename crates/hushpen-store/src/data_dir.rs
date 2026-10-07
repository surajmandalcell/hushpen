//! Where the data folder is and what it holds.
//!
//! `HUSHPEN_DATA_DIR` wins. Otherwise macOS uses
//! `~/Library/Application Support/Hushpen` and Linux uses
//! `$XDG_DATA_HOME/hushpen`, or `~/.local/share/hushpen` when `XDG_DATA_HOME`
//! is unset, empty, or not an absolute path.

use crate::{Error, Result, atomic, time};
use directories::BaseDirs;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PRODUCT: &str = "hushpen";
const MARKER_NAME: &str = ".hushpen-data";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
}

impl Os {
    pub const CURRENT: Os = if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Linux
    };
}

pub fn resolve(
    os: Os,
    data_dir_override: Option<&OsStr>,
    xdg_data_home: Option<&OsStr>,
    home: Option<&Path>,
) -> PathBuf {
    if let Some(dir) = data_dir_override.filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = home.unwrap_or_else(|| Path::new("."));
    match os {
        Os::MacOs => home.join("Library/Application Support/Hushpen"),
        Os::Linux => xdg_data_home
            .map(Path::new)
            .filter(|dir| dir.is_absolute())
            .map_or_else(|| home.join(".local/share"), Path::to_path_buf)
            .join("hushpen"),
    }
}

/// Resolves from the process environment.
pub fn resolve_from_env() -> PathBuf {
    let base_dirs = BaseDirs::new();
    resolve(
        Os::CURRENT,
        std::env::var_os("HUSHPEN_DATA_DIR").as_deref(),
        std::env::var_os("XDG_DATA_HOME").as_deref(),
        base_dirs.as_ref().map(BaseDirs::home_dir),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub product: String,
    /// ISO 8601 UTC time of the first start.
    pub created: String,
}

/// An opened data folder: it exists, carries the marker, and has the
/// subfolders the store crate writes to.
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let data = Self { root: root.into() };
        for dir in [
            data.root.clone(),
            data.config_dir(),
            data.history_dir(),
            data.logs_dir(),
        ] {
            fs::create_dir_all(&dir)
                .map_err(|e| Error::io(format!("could not create {}", dir.display()), e))?;
        }
        data.ensure_marker()?;
        Ok(data)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.config_dir().join("settings.json")
    }

    pub fn history_dir(&self) -> PathBuf {
        self.root.join("history")
    }

    pub fn database_path(&self) -> PathBuf {
        self.history_dir().join("history.db")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn log_path(&self) -> PathBuf {
        self.logs_dir().join("hushpen.log")
    }

    pub fn engine_log_path(&self) -> PathBuf {
        self.logs_dir().join("engine.log")
    }

    /// Where `ggml-<id>.bin` speech models live. The folder is created by the model manager.
    pub fn whisper_models_dir(&self) -> PathBuf {
        self.root.join("models").join("whisper")
    }

    /// `cache/downloads/<file>.download`: model downloads in progress. Created by the first download.
    pub fn downloads_dir(&self) -> PathBuf {
        self.root.join("cache").join("downloads")
    }

    /// `cache/sessions/<id>.wav`: audio being recorded. Created by the first capture.
    pub fn sessions_dir(&self) -> PathBuf {
        self.root.join("cache").join("sessions")
    }

    pub fn marker_path(&self) -> PathBuf {
        self.root.join(MARKER_NAME)
    }

    pub fn marker(&self) -> Result<Marker> {
        let path = self.marker_path();
        let bytes = fs::read(&path)
            .map_err(|e| Error::io(format!("could not read {}", path.display()), e))?;
        parse_marker(&bytes).ok_or_else(|| {
            Error::io(
                format!("unreadable marker {}", path.display()),
                io::ErrorKind::InvalidData.into(),
            )
        })
    }

    fn ensure_marker(&self) -> Result<()> {
        match self.marker() {
            Ok(marker) if marker.product == PRODUCT => Ok(()),
            Ok(_) => Err(Error::ForeignDataFolder(self.root.clone())),
            Err(_) => {
                let marker = serde_json::json!({
                    "product": PRODUCT,
                    "created": time::iso_seconds(time::now_unix_ms()),
                });
                atomic::write(&self.marker_path(), marker.to_string().as_bytes())
            }
        }
    }
}

fn parse_marker(bytes: &[u8]) -> Option<Marker> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    Some(Marker {
        product: value.get("product")?.as_str()?.to_string(),
        created: value.get("created")?.as_str()?.to_string(),
    })
}
