//! Moving the data folder. The user picks a folder that is empty or already
//! carries our marker; the move copies everything, verifies count, sizes, and
//! the hashes of the two files the app cannot rebuild, writes the pointer in
//! the default folder, and only then clears the old folder. A failure before
//! the pointer write changes nothing; the old folder is never deleted without
//! a verified copy.

use crate::data_dir::{self, MARKER_NAME};
use crate::{Error, Result, atomic, time};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// What a finished move copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub files: u64,
    pub bytes: u64,
}

/// Top-level entries that never cross: the pointer, the runtime folder, and
/// the marker (the target keeps its own).
fn skipped_top(name: &str) -> bool {
    name == "location.json" || name == MARKER_NAME || name == "run"
}

/// SQLite journals stay behind at any depth; the app checkpoints first.
fn is_journal(name: &str) -> bool {
    name.ends_with(".db-wal") || name.ends_with(".db-shm")
}

/// Moves the data folder from `current` to `target`, recording the pointer in
/// `default`. `target` must be empty or hold our marker; `default` is where
/// `location.json` lives.
pub fn move_data(current: &Path, target: &Path, default: &Path) -> Result<Report> {
    let current = absolutize(current)?;
    let target = absolutize(target)?;
    let default = absolutize(default)?;
    guard_target(&current, &target)?;
    let kept = pre_existing(&target)?;
    match copy_verify_point(&current, &target, &default) {
        Ok(report) => {
            clear_old(&current, &default)?;
            Ok(report)
        }
        Err(error) => {
            clean_target(&target, &kept);
            Err(error)
        }
    }
}

fn copy_verify_point(current: &Path, target: &Path, default: &Path) -> Result<Report> {
    fs::create_dir_all(target).map_err(|e| failed(target, "could not create the folder", e))?;
    let report = copy_tree(current, target)?;
    ensure_marker(target)?;
    verify(current, target, &report)?;
    point(default, target)
        .map_err(|e| Error::MoveFailed(format!("could not write the pointer: {e}")))?;
    Ok(report)
}

/// Writes the pointer into the default folder: the absolute target, or no
/// pointer at all when the move comes back to the default.
fn point(default: &Path, target: &Path) -> Result<()> {
    match target == default {
        true => data_dir::write_location(default, None),
        false => data_dir::write_location(default, Some(target)),
    }
}

fn absolutize(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::MoveFailed(format!(
            "{} is not an absolute path",
            path.display()
        )));
    }
    // Canonicalize what exists so the overlap checks survive `..` and links.
    Ok(match fs::canonicalize(path) {
        Ok(real) => real,
        Err(_) => match path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            Some(parent) => fs::canonicalize(parent)
                .map(|parent| parent.join(path.file_name().unwrap_or_default()))
                .unwrap_or_else(|_| path.to_path_buf()),
            None => path.to_path_buf(),
        },
    })
}

fn guard_target(current: &Path, target: &Path) -> Result<()> {
    if target == current || target.starts_with(current) || current.starts_with(target) {
        return Err(Error::MoveFailed(format!(
            "{} and {} overlap",
            current.display(),
            target.display()
        )));
    }
    if !target.exists() {
        return Ok(());
    }
    if !target.is_dir() {
        return Err(Error::MoveFailed(format!(
            "{} is not a folder",
            target.display()
        )));
    }
    // Empty is fine, and so are the two files the move machinery owns: our
    // marker and the pointer (a move back to the default folder meets both).
    let entries: Vec<_> = fs::read_dir(target)
        .map_err(|e| failed(target, "could not read the folder", e))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name())
        .collect();
    if entries
        .iter()
        .any(|name| name != MARKER_NAME && name != "location.json")
    {
        return Err(Error::MoveFailed(format!(
            "{} is not empty; pick an empty folder",
            target.display()
        )));
    }
    match entries.iter().any(|name| name == MARKER_NAME) {
        true => marker_check(target),
        false => Ok(()),
    }
}

fn marker_check(target: &Path) -> Result<()> {
    let bytes = fs::read(target.join(MARKER_NAME))
        .map_err(|e| failed(target, "could not read the marker", e))?;
    match parse_product(&bytes) {
        Some(product) if product == "hushpen" => Ok(()),
        _ => Err(Error::ForeignDataFolder(target.to_path_buf())),
    }
}

fn parse_product(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    Some(value.get("product")?.as_str()?.to_string())
}

fn pre_existing(target: &Path) -> Result<BTreeSet<std::ffi::OsString>> {
    match fs::read_dir(target) {
        Ok(entries) => Ok(entries
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.file_name())
            .collect()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(error) => Err(failed(target, "could not read the folder", error)),
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<Report> {
    let mut report = Report { files: 0, bytes: 0 };
    copy_into(from, from, to, &mut report)?;
    Ok(report)
}

fn copy_into(dir: &Path, root: &Path, target_root: &Path, report: &mut Report) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|e| failed(dir, "could not read the folder", e))?;
    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        if is_journal(&name) || (rel.components().count() == 1 && skipped_top(&name)) {
            continue;
        }
        let dest = target_root.join(&rel);
        let kind = entry
            .file_type()
            .map_err(|e| failed(&path, "stat failed", e))?;
        if kind.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| failed(&dest, "could not create", e))?;
            copy_into(&path, root, target_root, report)?;
        } else {
            // fs::copy follows links, so a dangling link stops the move here.
            let bytes = fs::copy(&path, &dest).map_err(|e| failed(&path, "could not copy", e))?;
            report.files += 1;
            report.bytes += bytes;
        }
    }
    Ok(())
}

fn ensure_marker(target: &Path) -> Result<()> {
    let path = target.join(MARKER_NAME);
    if path.exists() {
        return Ok(());
    }
    let marker = serde_json::json!({
        "product": "hushpen",
        "created": time::iso_seconds(time::now_unix_ms()),
    });
    atomic::write(&path, marker.to_string().as_bytes())
}

/// Count and bytes must match; the two files the app cannot rebuild
/// (settings and the history database) must hash the same.
fn verify(current: &Path, target: &Path, report: &Report) -> Result<()> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    count_tree(target, target, &mut files, &mut bytes)?;
    if files != report.files || bytes != report.bytes {
        return Err(Error::MoveFailed(format!(
            "the copy in {} does not match: {} files and {} bytes against {} and {}",
            target.display(),
            files,
            bytes,
            report.files,
            report.bytes
        )));
    }
    for rel in ["config/settings.json", "history/history.db"] {
        let source = current.join(rel);
        if source.is_file() {
            hash_pair(&source, &target.join(rel))?;
        }
    }
    Ok(())
}

fn count_tree(dir: &Path, root: &Path, files: &mut u64, bytes: &mut u64) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|e| failed(dir, "could not read the folder", e))?;
    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_journal(&name) || (dir == root && skipped_top(&name)) {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|e| failed(&path, "stat failed", e))?;
        if kind.is_dir() {
            count_tree(&path, root, files, bytes)?;
        } else if kind.is_file() {
            *files += 1;
            *bytes += entry
                .metadata()
                .map_err(|e| failed(&path, "stat failed", e))?
                .len();
        }
    }
    Ok(())
}

fn hash_pair(source: &Path, copied: &Path) -> Result<()> {
    if hash(source)? != hash(copied)? {
        return Err(Error::MoveFailed(format!(
            "{} does not match its copy",
            copied.display()
        )));
    }
    Ok(())
}

fn hash(path: &Path) -> Result<[u8; 32]> {
    let bytes = fs::read(path).map_err(|e| failed(path, "could not read", e))?;
    Ok(Sha256::digest(&bytes).into())
}

/// Removes what the move copied into `target`, keeping what was there before.
fn clean_target(target: &Path, kept: &BTreeSet<std::ffi::OsString>) {
    if let Ok(entries) = fs::read_dir(target) {
        for entry in entries.filter_map(std::result::Result::ok) {
            if kept.contains(&entry.file_name()) {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

/// Clears the old folder after the copy verified and the pointer moved. The
/// pointer file itself stays when the old folder is the default folder.
fn clear_old(current: &Path, default: &Path) -> Result<()> {
    let keep_pointer = current == default;
    let entries =
        fs::read_dir(current).map_err(|e| failed(current, "could not read the old folder", e))?;
    for entry in entries.filter_map(std::result::Result::ok) {
        if keep_pointer && entry.file_name() == "location.json" {
            continue;
        }
        let path = entry.path();
        let outcome = match path.is_dir() {
            true => fs::remove_dir_all(&path),
            false => fs::remove_file(&path),
        };
        if let Err(error) = outcome {
            restore_pointer(current, default);
            return Err(failed(&path, "could not clear the old folder", error));
        }
    }
    Ok(())
}

/// A delete failure after the pointer moved puts the pointer back, so the
/// data is duplicated instead of stranded.
fn restore_pointer(current: &Path, default: &Path) {
    let result = match current == default {
        // The old folder is the default, so nothing pointed away before.
        true => data_dir::write_location(default, None),
        false => data_dir::write_location(default, Some(current)),
    };
    if let Err(error) = result {
        log::error!("could not restore the data folder pointer: {error}");
    }
}

fn failed(path: &Path, step: &str, error: impl std::fmt::Display) -> Error {
    Error::MoveFailed(format!("{}: {step}: {error}", path.display()))
}
