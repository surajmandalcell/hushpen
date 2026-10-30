//! The voice library: the voices the user cloned for Chatterbox, one folder each under
//! `voices/`. A folder holds `reference.wav`, the clip the voice is cloned from (24 kHz mono, 5
//! to 20 seconds of speech), and `voice.json`, its name and when it was added. The voice
//! worker keeps what it computes from a clip beside it (`reference.<model>.cond`), so deleting
//! the folder deletes that too.

use crate::{Error, Result};
use serde_json::json;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const CLIP_FILE: &str = "reference.wav";
const META_FILE: &str = "voice.json";
/// A folder that is being written. A crash leaves it behind; the next add or list removes it.
const NEW_PREFIX: &str = ".new-";
pub const MAX_NAME_CHARS: usize = 40;
const MAX_ID_CHARS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceEntry {
    /// The folder name: lowercase letters, digits, and dashes.
    pub id: String,
    pub name: String,
    /// When the voice was added, in Unix milliseconds.
    pub created_ms: u64,
    /// The length of the clip.
    pub duration_ms: u64,
}

/// Why a name cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProblem {
    Empty,
    TooLong,
    /// Another voice has this name, whatever the case of its letters.
    Taken,
}

#[derive(Debug, Clone)]
pub struct Library {
    dir: PathBuf,
}

impl Library {
    /// The library in `dir`. Nothing is created until the first voice is added.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The clip of voice `id`, or `None` for an id that cannot name a folder of the library.
    pub fn clip_path(&self, id: &str) -> Option<PathBuf> {
        is_id(id).then(|| self.dir.join(id).join(CLIP_FILE))
    }

    /// Every voice with a clip and a readable `voice.json`, oldest first.
    pub fn list(&self) -> Vec<VoiceEntry> {
        let Ok(folders) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut voices: Vec<VoiceEntry> = folders
            .filter_map(std::result::Result::ok)
            .filter_map(|folder| {
                let id = folder.file_name().to_str()?.to_owned();
                self.get(&id)
            })
            .collect();
        voices.sort_by(|a, b| a.created_ms.cmp(&b.created_ms).then(a.id.cmp(&b.id)));
        voices
    }

    pub fn get(&self, id: &str) -> Option<VoiceEntry> {
        let clip = self.clip_path(id)?;
        if !clip.is_file() {
            return None;
        }
        let bytes = fs::read(self.dir.join(id).join(META_FILE)).ok()?;
        let meta: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        Some(VoiceEntry {
            id: id.to_owned(),
            name: meta.get("name")?.as_str()?.to_owned(),
            created_ms: meta.get("created_ms")?.as_u64()?,
            duration_ms: meta.get("duration_ms")?.as_u64()?,
        })
    }

    /// `name` as it is kept: trimmed, with single spaces. `except` is the voice being renamed,
    /// whose own name is not taken.
    pub fn check_name(
        &self,
        name: &str,
        except: Option<&str>,
    ) -> std::result::Result<String, NameProblem> {
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return Err(NameProblem::Empty);
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(NameProblem::TooLong);
        }
        let taken = self.list().into_iter().any(|voice| {
            Some(voice.id.as_str()) != except && voice.name.to_lowercase() == name.to_lowercase()
        });
        if taken {
            return Err(NameProblem::Taken);
        }
        Ok(name)
    }

    /// `Voice 1`, or the first `Voice <n>` no voice has.
    pub fn next_name(&self) -> String {
        (1..)
            .map(|n| format!("Voice {n}"))
            .find(|name| self.check_name(name, None).is_ok())
            .unwrap_or_else(|| "Voice".to_owned())
    }

    /// Adds a voice. `write_clip` writes the clip to the path it is given; the voice shows in
    /// the list only once the clip and its name are both on disk.
    pub fn add(
        &self,
        name: &str,
        duration_ms: u64,
        now_ms: u64,
        write_clip: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<VoiceEntry> {
        let name = self.check_name(name, None).map_err(invalid_name)?;
        fs::create_dir_all(&self.dir)
            .map_err(|e| Error::io(format!("could not create {}", self.dir.display()), e))?;
        self.sweep_unfinished();
        let id = self.free_id(now_ms);
        let staging = self.dir.join(format!("{NEW_PREFIX}{id}"));
        let entry = VoiceEntry {
            id: id.clone(),
            name,
            created_ms: now_ms,
            duration_ms,
        };
        let written = fs::create_dir(&staging)
            .and_then(|()| write_clip(&staging.join(CLIP_FILE)))
            .and_then(|()| write_meta(&staging, &entry))
            .and_then(|()| fs::rename(&staging, self.dir.join(&id)));
        if let Err(error) = written {
            let _ = fs::remove_dir_all(&staging);
            return Err(Error::io(
                format!("could not add a voice to {}", self.dir.display()),
                error,
            ));
        }
        Ok(entry)
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<VoiceEntry> {
        let mut entry = self.get(id).ok_or_else(|| missing(id))?;
        entry.name = self.check_name(name, Some(id)).map_err(invalid_name)?;
        write_meta(&self.dir.join(id), &entry).map_err(|e| {
            Error::io(
                format!("could not rename the voice in {}", self.dir.display()),
                e,
            )
        })?;
        Ok(entry)
    }

    /// Deletes the voice, its clip, and what the worker computed from it.
    pub fn delete(&self, id: &str) -> Result<()> {
        if self.get(id).is_none() {
            return Err(missing(id));
        }
        fs::remove_dir_all(self.dir.join(id)).map_err(|e| {
            Error::io(
                format!("could not delete the voice in {}", self.dir.display()),
                e,
            )
        })
    }

    fn free_id(&self, now_ms: u64) -> String {
        let base = format!("v{}", base36(now_ms));
        (0..)
            .map(|n| {
                if n == 0 {
                    base.clone()
                } else {
                    format!("{base}-{n}")
                }
            })
            .find(|id| {
                !self.dir.join(id).exists() && !self.dir.join(format!("{NEW_PREFIX}{id}")).exists()
            })
            .unwrap_or(base)
    }

    fn sweep_unfinished(&self) {
        let Ok(folders) = fs::read_dir(&self.dir) else {
            return;
        };
        for folder in folders.filter_map(std::result::Result::ok) {
            if folder.file_name().to_string_lossy().starts_with(NEW_PREFIX) {
                let _ = fs::remove_dir_all(folder.path());
            }
        }
    }
}

/// Whether `id` can be a folder name of the library, so an id from outside never names a path
/// elsewhere.
pub fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_CHARS
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
}

fn write_meta(folder: &Path, entry: &VoiceEntry) -> io::Result<()> {
    let meta = json!({
        "name": entry.name,
        "created_ms": entry.created_ms,
        "duration_ms": entry.duration_ms,
    });
    crate::atomic::write(&folder.join(META_FILE), meta.to_string().as_bytes())
        .map_err(|error| io::Error::other(error.to_string()))
}

fn base36(mut n: u64) -> String {
    let mut digits = Vec::new();
    loop {
        let digit = (n % 36) as u8;
        digits.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        });
        n /= 36;
        if n == 0 {
            break;
        }
    }
    digits.reverse();
    String::from_utf8(digits).unwrap_or_default()
}

fn invalid_name(problem: NameProblem) -> Error {
    let why = match problem {
        NameProblem::Empty => "the voice name is empty",
        NameProblem::TooLong => "the voice name is too long",
        NameProblem::Taken => "another voice has this name",
    };
    Error::io(why, io::ErrorKind::InvalidInput.into())
}

fn missing(id: &str) -> Error {
    Error::io(
        format!("there is no voice `{id}`"),
        io::ErrorKind::NotFound.into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(bytes: &'static [u8]) -> impl FnOnce(&Path) -> io::Result<()> {
        move |path| fs::write(path, bytes)
    }

    #[test]
    fn an_added_voice_lists_with_its_clip_and_name_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path().join("voices"));
        assert!(library.list().is_empty(), "no folder yet");
        let anna = library
            .add("  Anna   Lee ", 7_000, 2_000, clip(b"a"))
            .unwrap();
        let ben = library.add("Ben", 12_500, 1_000, clip(b"b")).unwrap();
        assert_eq!(anna.name, "Anna Lee");
        let listed = library.list();
        assert_eq!(listed, [ben.clone(), anna.clone()]);
        let path = library.clip_path(&anna.id).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"a");
        assert_eq!(path.file_name().unwrap(), CLIP_FILE);
        assert_eq!(library.get(&ben.id).unwrap().duration_ms, 12_500);
    }

    #[test]
    fn two_voices_added_in_the_same_millisecond_get_their_own_folders() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path());
        let first = library.add("One", 6_000, 5, clip(b"1")).unwrap();
        let second = library.add("Two", 6_000, 5, clip(b"2")).unwrap();
        assert_ne!(first.id, second.id);
        assert!(is_id(&first.id) && is_id(&second.id));
        assert_eq!(library.list().len(), 2);
    }

    #[test]
    fn names_are_trimmed_unique_and_short() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path());
        assert_eq!(library.next_name(), "Voice 1");
        let one = library.add("Voice 1", 6_000, 1, clip(b"1")).unwrap();
        assert_eq!(library.next_name(), "Voice 2");
        assert_eq!(library.check_name("  ", None), Err(NameProblem::Empty));
        assert_eq!(
            library.check_name(&"x".repeat(MAX_NAME_CHARS + 1), None),
            Err(NameProblem::TooLong)
        );
        assert_eq!(library.check_name("voice 1", None), Err(NameProblem::Taken));
        assert_eq!(
            library.check_name("VOICE 1", Some(&one.id)),
            Ok("VOICE 1".to_owned()),
            "a voice may keep its own name in another case"
        );
        assert!(library.add("voice 1", 6_000, 2, clip(b"2")).is_err());
        assert_eq!(library.list().len(), 1);
    }

    #[test]
    fn rename_and_delete_change_only_that_voice() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path());
        let anna = library.add("Anna", 6_000, 1, clip(b"a")).unwrap();
        let ben = library.add("Ben", 6_000, 2, clip(b"b")).unwrap();
        // What the worker computed from the clip goes with it.
        let cond = dir
            .path()
            .join(&anna.id)
            .join("reference.chatterbox-turbo.cond");
        fs::write(&cond, b"c").unwrap();
        assert_eq!(library.rename(&anna.id, "Anne").unwrap().name, "Anne");
        assert!(library.rename(&anna.id, "ben").is_err());
        assert_eq!(library.get(&anna.id).unwrap().name, "Anne");
        library.delete(&anna.id).unwrap();
        assert!(!cond.exists());
        assert_eq!(library.list(), [ben]);
        assert!(library.delete(&anna.id).is_err());
        assert!(library.rename("gone", "X").is_err());
    }

    #[test]
    fn a_failed_or_unfinished_add_leaves_no_voice() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path());
        let failed = library.add("Anna", 6_000, 1, |_| Err(io::Error::other("disk full")));
        assert!(failed.is_err());
        assert!(library.list().is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        // A crash in the middle of an add.
        fs::create_dir(dir.path().join(".new-v1")).unwrap();
        fs::write(dir.path().join(".new-v1").join(CLIP_FILE), b"a").unwrap();
        assert!(library.list().is_empty());
        library.add("Ben", 6_000, 2, clip(b"b")).unwrap();
        assert!(!dir.path().join(".new-v1").exists());
    }

    #[test]
    fn an_id_from_outside_never_names_a_path_out_of_the_library() {
        let library = Library::new("/data/voices");
        let long = "a".repeat(40);
        for bad in [
            "",
            "..",
            "../x",
            "a/b",
            "A1",
            "-x",
            long.as_str(),
            "built-in ",
        ] {
            assert!(library.clip_path(bad).is_none(), "{bad:?}");
        }
        assert_eq!(
            library.clip_path("v1a2-3").unwrap(),
            Path::new("/data/voices/v1a2-3/reference.wav")
        );
        assert_eq!(base36(0), "0");
        assert_eq!(base36(36 * 36 + 35), "10z");
    }
}
