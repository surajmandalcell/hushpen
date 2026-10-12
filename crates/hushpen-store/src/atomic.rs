use crate::{Error, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to a unique temp file next to `path`, fsyncs it, and renames
/// it over `path`. A reader sees the old file or the new one, never a mix.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    write_with_mode(path, bytes, None)
}

/// Like [`write`], but the file is created with `mode` (Unix permission bits) from the start,
/// so the bytes are never readable through a wider mode, whatever the process umask says.
pub(crate) fn write_private(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    write_with_mode(path, bytes, Some(mode))
}

fn write_with_mode(path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = dir.join(format!(
        ".{name}.tmp-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = write_and_rename(&temp, path, dir, bytes, mode);
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn write_and_rename(
    temp: &Path,
    path: &Path,
    dir: &Path,
    bytes: &[u8],
    mode: Option<u32>,
) -> Result<()> {
    let context = |what: &str| format!("{what} {}", path.display());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(mode);
    }
    let mut file = options
        .open(temp)
        .map_err(|e| Error::io(context("could not create a temp file for"), e))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| Error::io(context("could not write"), e))?;
    drop(file);
    fs::rename(temp, path).map_err(|e| Error::io(context("could not replace"), e))?;
    // The rename is durable only once the directory entry is flushed.
    if let Ok(dir_handle) = File::open(dir) {
        let _ = dir_handle.sync_all();
    }
    Ok(())
}
