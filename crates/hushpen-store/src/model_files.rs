//! Model files on disk: the `.verified` stamp, the hash check, and delete.
//!
//! A model file is used only when its stamp holds the pinned hash and still matches the file's
//! size and modification time. Any other file is hashed again before use.

use crate::{Error, Result, atomic};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const STAMP_ALGO: &str = "sha256";
const HASH_CHUNK: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub hash: String,
    pub size: u64,
    pub mtime_ms: u64,
}

/// What the file on disk is, judged without reading its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Missing,
    /// The stamp holds the pinned hash and still matches size and mtime.
    Ready,
    /// No usable stamp. The content must be hashed.
    NeedsHash,
    /// The size differs from the pinned size, so the file cannot be the model.
    WrongSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ready { stamped: bool },
    Mismatch,
}

pub fn stamp_path(model: &Path) -> PathBuf {
    let mut name = model.file_name().unwrap_or_default().to_os_string();
    name.push(".verified");
    model.with_file_name(name)
}

fn mtime_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(0))
}

pub fn read_stamp(model: &Path) -> Option<Stamp> {
    let bytes = fs::read(stamp_path(model)).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    if value.get("algo")?.as_str()? != STAMP_ALGO {
        return None;
    }
    Some(Stamp {
        hash: value.get("hash")?.as_str()?.to_owned(),
        size: value.get("size")?.as_u64()?,
        mtime_ms: value.get("mtime_ms")?.as_u64()?,
    })
}

pub fn write_stamp(model: &Path, hash: &str) -> Result<Stamp> {
    let context = |what: &str| format!("{what} {}", model.display());
    let meta = fs::metadata(model).map_err(|e| Error::io(context("could not read"), e))?;
    let stamp = Stamp {
        hash: hash.to_owned(),
        size: meta.len(),
        mtime_ms: mtime_ms(&meta),
    };
    let body = json!({
        "algo": STAMP_ALGO,
        "hash": stamp.hash,
        "size": stamp.size,
        "mtime_ms": stamp.mtime_ms,
    });
    atomic::write(&stamp_path(model), body.to_string().as_bytes())?;
    Ok(stamp)
}

pub fn check(model: &Path, bytes: u64, sha256: &str) -> Check {
    let Ok(meta) = fs::metadata(model) else {
        return Check::Missing;
    };
    if !meta.is_file() {
        return Check::Missing;
    }
    if meta.len() != bytes {
        return Check::WrongSize;
    }
    match read_stamp(model) {
        Some(stamp)
            if stamp.hash == sha256
                && stamp.size == meta.len()
                && stamp.mtime_ms == mtime_ms(&meta) =>
        {
            Check::Ready
        }
        _ => Check::NeedsHash,
    }
}

/// sha256 of a whole file, as lower-case hex. `on_chunk` gets the byte count so far and returns
/// `false` to stop; the result is then `ErrorKind::Interrupted`.
pub fn hash_file(path: &Path, mut on_chunk: impl FnMut(u64) -> bool) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_CHUNK];
    let mut done = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        done += read as u64;
        if !on_chunk(done) {
            return Err(io::ErrorKind::Interrupted.into());
        }
    }
    Ok(hex(&hasher.finalize()))
}

pub fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Hashes the file and stamps it when the hash is the pinned one. A stale stamp of a file that
/// fails is removed. A stamp that cannot be written (a read-only folder) still gives `Ready`;
/// the file is then hashed again at the next start.
pub fn verify(
    model: &Path,
    bytes: u64,
    sha256: &str,
    on_chunk: impl FnMut(u64) -> bool,
) -> io::Result<Verdict> {
    let size_ok = fs::metadata(model).is_ok_and(|meta| meta.len() == bytes);
    let matches = size_ok && hash_file(model, on_chunk)? == sha256;
    if !matches {
        let _ = fs::remove_file(stamp_path(model));
        return Ok(Verdict::Mismatch);
    }
    Ok(Verdict::Ready {
        stamped: write_stamp(model, sha256).is_ok(),
    })
}

/// Deletes the model and its stamp. A file that is already gone is fine.
pub fn remove(model: &Path) -> io::Result<()> {
    for path in [stamp_path(model), model.to_path_buf()] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    const BODY: &[u8] = b"pretend this is a ggml model";

    fn sha_of(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    fn model(dir: &Path) -> PathBuf {
        let path = dir.join("ggml-test.bin");
        fs::write(&path, BODY).unwrap();
        path
    }

    #[test]
    fn a_missing_file_is_missing_and_a_short_one_has_the_wrong_size() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ggml-test.bin");
        let sha = sha_of(BODY);
        assert_eq!(check(&path, BODY.len() as u64, &sha), Check::Missing);
        fs::write(&path, &BODY[..5]).unwrap();
        assert_eq!(check(&path, BODY.len() as u64, &sha), Check::WrongSize);
    }

    #[test]
    fn verify_stamps_a_good_file_and_the_stamp_holds_algo_hash_size_and_mtime() {
        let tmp = tempfile::tempdir().unwrap();
        let path = model(tmp.path());
        let sha = sha_of(BODY);
        assert_eq!(check(&path, BODY.len() as u64, &sha), Check::NeedsHash);

        let verdict = verify(&path, BODY.len() as u64, &sha, |_| true).unwrap();
        assert_eq!(verdict, Verdict::Ready { stamped: true });
        assert_eq!(check(&path, BODY.len() as u64, &sha), Check::Ready);

        let raw: Value = serde_json::from_slice(&fs::read(stamp_path(&path)).unwrap()).unwrap();
        assert_eq!(raw["algo"], "sha256");
        assert_eq!(raw["hash"], sha);
        assert_eq!(raw["size"], BODY.len());
        assert!(raw["mtime_ms"].as_u64().unwrap() > 0);
    }

    #[test]
    fn a_flipped_byte_that_keeps_the_size_fails_once_the_mtime_moves() {
        let tmp = tempfile::tempdir().unwrap();
        let path = model(tmp.path());
        let sha = sha_of(BODY);
        verify(&path, BODY.len() as u64, &sha, |_| true).unwrap();

        let mut file = OpenOptions::new().write(true).open(&path).unwrap();
        file.seek(SeekFrom::Start(10)).unwrap();
        file.write_all(b"X").unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        drop(file);

        assert_eq!(check(&path, BODY.len() as u64, &sha), Check::NeedsHash);
        let verdict = verify(&path, BODY.len() as u64, &sha, |_| true).unwrap();
        assert_eq!(verdict, Verdict::Mismatch);
        assert!(!stamp_path(&path).exists(), "a stale stamp must go");
        assert_eq!(check(&path, BODY.len() as u64, &sha), Check::NeedsHash);
    }

    #[test]
    fn a_stamp_for_another_hash_is_not_trusted() {
        let tmp = tempfile::tempdir().unwrap();
        let path = model(tmp.path());
        write_stamp(&path, &"0".repeat(64)).unwrap();
        assert_eq!(
            check(&path, BODY.len() as u64, &sha_of(BODY)),
            Check::NeedsHash
        );
    }

    #[test]
    fn a_garbled_stamp_means_hash_again() {
        let tmp = tempfile::tempdir().unwrap();
        let path = model(tmp.path());
        fs::write(stamp_path(&path), b"not json").unwrap();
        assert_eq!(read_stamp(&path), None);
        assert_eq!(
            check(&path, BODY.len() as u64, &sha_of(BODY)),
            Check::NeedsHash
        );
    }

    #[test]
    fn hashing_can_be_stopped_between_chunks() {
        let tmp = tempfile::tempdir().unwrap();
        let path = model(tmp.path());
        let error = hash_file(&path, |_| false).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn remove_deletes_the_file_and_its_stamp_and_accepts_a_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = model(tmp.path());
        write_stamp(&path, &sha_of(BODY)).unwrap();
        remove(&path).unwrap();
        assert!(!path.exists());
        assert!(!stamp_path(&path).exists());
        remove(&path).unwrap();
    }
}
