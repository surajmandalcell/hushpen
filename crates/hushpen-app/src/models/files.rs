//! Where each model's files live, the start-up check of those files, and the download of a
//! model that has several.

use super::{ModelEvent, ModelState, Models, Transfer, failure_text};
use crate::net::download::{Failure, Outcome, Progress, Spec};
use hushpen_core::catalog::{ModelEntry, ModelFile, ModelKind};
use hushpen_core::error::{DOWNLOAD_FAILED, MODEL_HASH_MISMATCH};
use hushpen_store::data_dir::DataDir;
use hushpen_store::model_files::{self, Check, Verdict};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

/// The folder that holds the files of `entry`.
pub(super) fn folder(data: &DataDir, entry: &ModelEntry) -> PathBuf {
    match entry.kind {
        ModelKind::Speech => data.whisper_models_dir(),
        ModelKind::Cleanup => data.llm_models_dir(),
        ModelKind::Voice => data.tts_models_dir().join(&entry.id),
    }
}

/// Each file of `entry` with its place on disk.
pub(super) fn paths<'a>(
    data: &DataDir,
    entry: &'a ModelEntry,
) -> impl Iterator<Item = (&'a ModelFile, PathBuf)> {
    let folder = folder(data, entry);
    entry.files.iter().map(move |file| {
        let path = folder.join(&file.file);
        (file, path)
    })
}

/// A download for each file of `entry` that is not already on disk and stamped.
pub(super) fn missing_specs(data: &DataDir, entry: &ModelEntry) -> Vec<Spec> {
    paths(data, entry)
        .filter(|(file, path)| model_files::check(path, file.bytes, &file.sha256) != Check::Ready)
        .map(|(file, path)| Spec {
            url: file.url.clone(),
            bytes: file.bytes,
            sha256: file.sha256.clone(),
            partial: data.downloads_dir().join(format!("{}.download", file.file)),
            dest: path,
        })
        .collect()
}

/// The bytes still to come: each file, less what its partial already holds.
pub(super) fn bytes_to_fetch(specs: &[Spec]) -> u64 {
    specs
        .iter()
        .map(|spec| {
            let held = std::fs::metadata(&spec.partial).map_or(0, |meta| meta.len());
            spec.bytes.saturating_sub(held)
        })
        .sum()
}

/// Fetches `specs` one after the other and reports them as one download. Only the last file's
/// hash check shows as checking, so a folder of files does not flicker between the two.
pub(super) fn run_download(
    specs: &[Spec],
    transfer: &Transfer,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Progress),
) -> Result<Outcome, Failure> {
    let total = specs.iter().map(|spec| spec.bytes).sum();
    let mut before = 0;
    for (index, spec) in specs.iter().enumerate() {
        let last = index + 1 == specs.len();
        let mut step = |progress| {
            report(match progress {
                Progress::Bytes { done, .. } => Progress::Bytes {
                    done: before + done,
                    total,
                },
                Progress::Verifying if !last => Progress::Bytes {
                    done: before + spec.bytes,
                    total,
                },
                Progress::Verifying => Progress::Verifying,
            });
        };
        if transfer(spec, cancel, &mut step)? == Outcome::Cancelled {
            return Ok(Outcome::Cancelled);
        }
        before += spec.bytes;
    }
    Ok(Outcome::Done)
}

/// Deletes every file of `entry` with its stamp, then a voice model's emptied folder.
pub(super) fn remove(data: &DataDir, entry: &ModelEntry) -> std::io::Result<()> {
    for (_, path) in paths(data, entry) {
        model_files::remove(&path)?;
    }
    if entry.kind == ModelKind::Voice {
        // Only an empty folder goes; anything the user put there stays.
        let _ = std::fs::remove_dir(folder(data, entry));
    }
    Ok(())
}

impl Models {
    /// Reads every model's state. A model whose files all have stamps that still match is ready
    /// at once; one with an unstamped file is hashed on a worker thread and stays `Verifying`
    /// until the hashes are known.
    pub(super) fn scan(&mut self) {
        let mut to_hash = Vec::new();
        let mut failed = Vec::new();
        for row in &mut self.rows {
            if matches!(row.state, ModelState::Downloading { .. }) {
                continue;
            }
            let checks: Vec<_> = paths(&self.storage.data, &row.entry)
                .map(|(file, path)| {
                    let check = model_files::check(&path, file.bytes, &file.sha256);
                    (check, (path, file.bytes, file.sha256.clone()))
                })
                .collect();
            let any = |wanted: Check| checks.iter().any(|(check, _)| *check == wanted);
            row.state = if any(Check::WrongSize) {
                failed.push(failure_text(&row.entry));
                ModelState::Failed
            } else if any(Check::Missing) {
                ModelState::NotDownloaded
            } else if any(Check::NeedsHash) {
                let files = checks
                    .into_iter()
                    .filter(|(check, _)| *check == Check::NeedsHash)
                    .map(|(_, file)| file)
                    .collect::<Vec<_>>();
                to_hash.push((row.entry.id.clone(), files));
                ModelState::Verifying
            } else {
                ModelState::Ready
            };
        }
        if let Some(message) = failed.pop() {
            self.set_notice(MODEL_HASH_MISMATCH, message);
        }
        if to_hash.is_empty() {
            return;
        }
        let events = self.events.clone();
        let spawned = (self.spawn)(Box::new(move || {
            for (id, files) in to_hash {
                let ready = files.iter().all(|(path, bytes, sha256)| {
                    matches!(
                        model_files::verify(path, *bytes, sha256, |_| true),
                        Ok(Verdict::Ready { .. })
                    )
                });
                if !ready {
                    log::warn!("{MODEL_HASH_MISMATCH} model={id} failed the hash check");
                }
                let _ = events.unbounded_send(ModelEvent::Verified { id, ready });
            }
        }));
        if let Err(error) = spawned {
            log::warn!("{DOWNLOAD_FAILED} could not start the hash check: {error}");
            for row in &mut self.rows {
                if row.state == ModelState::Verifying {
                    row.state = ModelState::Failed;
                }
            }
        }
    }
}
