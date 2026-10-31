//! The voices files of Kokoro mixes. sherpa-onnx speaks one speaker of the voices file it
//! loads, so a mix is a copy of `voices.bin` whose [`MIX_SPEAKER`] holds the weighted average of
//! the two voices' style vectors. The pinned `voices.bin` is only read: its checksum must stay
//! valid.

use hushpen_core::kokoro::{MIX_SPEAKER, SPEAKER_BYTES, VOICES_BYTES, VoiceMix};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The mix files a folder keeps. Each is the size of `voices.bin` (28 MB).
pub const KEPT: usize = 3;

/// The bytes of one speaker in a voices file.
fn speaker(voices: &[u8], index: usize) -> &[u8] {
    &voices[index * SPEAKER_BYTES..(index + 1) * SPEAKER_BYTES]
}

/// `voices` with the speaker [`MIX_SPEAKER`] replaced by the blend of `mix`.
pub fn blend(voices: &[u8], mix: &VoiceMix) -> Result<Vec<u8>, String> {
    if voices.len() != VOICES_BYTES {
        return Err(format!(
            "the voices file holds {} bytes, not {VOICES_BYTES}",
            voices.len()
        ));
    }
    let index = |speaker: i32| usize::try_from(speaker).unwrap_or(0);
    let (base, with) = (
        speaker(voices, index(mix.base.speaker)),
        speaker(voices, index(mix.with.speaker)),
    );
    let (base_weight, with_weight) = mix.weights();
    let mut out = voices.to_vec();
    let start = index(MIX_SPEAKER) * SPEAKER_BYTES;
    let blended = base
        .as_chunks::<4>()
        .0
        .iter()
        .zip(with.as_chunks::<4>().0)
        .map(|(a, b)| f32::from_le_bytes(*a) * base_weight + f32::from_le_bytes(*b) * with_weight);
    for (slot, value) in out[start..start + SPEAKER_BYTES]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(blended)
    {
        *slot = value.to_le_bytes();
    }
    Ok(out)
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

/// The voices file of `mix` in `dir`, written from `source` unless a file of the right size
/// that is not older than `source` is there. Writing one removes all but the newest [`KEPT`].
pub fn ensure(source: &Path, dir: &Path, mix: &VoiceMix) -> Result<PathBuf, String> {
    let path = dir.join(mix.file_name());
    let fresh = fs::metadata(&path).is_ok_and(|meta| meta.len() == VOICES_BYTES as u64)
        && modified(&path) >= modified(source);
    if fresh {
        return Ok(path);
    }
    let voices =
        fs::read(source).map_err(|error| format!("cannot read {}: {error}", source.display()))?;
    let mixed = blend(&voices, mix)?;
    fs::create_dir_all(dir).map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    let partial = path.with_extension("partial");
    fs::write(&partial, &mixed)
        .and_then(|()| fs::rename(&partial, &path))
        .map_err(|error| {
            let _ = fs::remove_file(&partial);
            format!("cannot write {}: {error}", path.display())
        })?;
    prune(dir, &path);
    Ok(path)
}

/// Removes the oldest mix files past [`KEPT`]; `keep` always stays.
fn prune(dir: &Path, keep: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(Option<SystemTime>, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path != keep && path.extension().is_some_and(|ext| ext == "bin"))
        .map(|path| (modified(&path), path))
        .collect();
    files.sort_by_key(|file| std::cmp::Reverse(file.0));
    for (_, path) in files.into_iter().skip(KEPT - 1) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::kokoro::{SPEAKERS, STYLE_ROWS, STYLE_WIDTH};

    /// A voices file whose every float is its speaker number plus its place in the speaker
    /// divided by a million.
    fn voices() -> Vec<u8> {
        (0..SPEAKERS)
            .flat_map(|speaker| {
                (0..STYLE_ROWS * STYLE_WIDTH)
                    .flat_map(move |i| (speaker as f32 + i as f32 / 1e6).to_le_bytes())
            })
            .collect()
    }

    /// A folder of its own under the temp folder, removed at the end of the test.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("hushpen-tts-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn float(bytes: &[u8], speaker: usize, index: usize) -> f32 {
        let at = speaker * SPEAKER_BYTES + index * 4;
        f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn the_blend_is_the_weighted_average_in_the_mix_speaker_only() {
        let source = voices();
        let mix = VoiceMix::new("af_heart", "am_adam", 25).unwrap();
        let mixed = blend(&source, &mix).unwrap();
        assert_eq!(mixed.len(), source.len());
        let slot = usize::try_from(MIX_SPEAKER).unwrap();
        let last = STYLE_ROWS * STYLE_WIDTH - 1;
        for index in [0, 1, STYLE_WIDTH, last] {
            let want = float(&source, 3, index) * 0.75 + float(&source, 11, index) * 0.25;
            assert!((float(&mixed, slot, index) - want).abs() < 1e-5, "{index}");
        }
        assert!((float(&mixed, slot, 0) - 5.0).abs() < 1e-5);
        let end = slot * SPEAKER_BYTES;
        assert_eq!(
            mixed[..end],
            source[..end],
            "every other speaker is unchanged"
        );
    }

    #[test]
    fn a_voices_file_of_another_size_is_refused() {
        let mix = VoiceMix::new("af_heart", "am_adam", 50).unwrap();
        let problem = blend(&[0; 16], &mix).unwrap_err();
        assert!(problem.contains("16 bytes"), "{problem}");
    }

    #[test]
    fn a_mix_file_is_written_once_reused_and_the_oldest_ones_go() {
        let root = Scratch::new("mix-cache");
        let source = root.0.join("voices.bin");
        fs::write(&source, voices()).unwrap();
        let dir = root.0.join("mixes");
        let mix = |percent| VoiceMix::new("af_heart", "am_adam", percent).unwrap();
        let first = ensure(&source, &dir, &mix(50)).unwrap();
        assert_eq!(first, dir.join("af_heart+am_adam-50.bin"));
        let written = modified(&first);
        assert_eq!(ensure(&source, &dir, &mix(50)).unwrap(), first);
        assert_eq!(
            modified(&first),
            written,
            "a fresh file is not written again"
        );
        assert_eq!(
            fs::read(&source).unwrap(),
            voices(),
            "the source stays as it is"
        );
        for percent in [10, 20, 30] {
            std::thread::sleep(std::time::Duration::from_millis(20));
            ensure(&source, &dir, &mix(percent)).unwrap();
        }
        let mut left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "af_heart+am_adam-10.bin",
                "af_heart+am_adam-20.bin",
                "af_heart+am_adam-30.bin"
            ]
        );
    }

    #[test]
    fn a_cut_mix_file_is_written_again() {
        let root = Scratch::new("mix-cut");
        let source = root.0.join("voices.bin");
        fs::write(&source, voices()).unwrap();
        let mix = VoiceMix::new("bf_emma", "bm_george", 40).unwrap();
        let path = root.0.join(mix.file_name());
        fs::write(&path, b"cut").unwrap();
        assert_eq!(ensure(&source, &root.0, &mix).unwrap(), path);
        assert_eq!(fs::metadata(&path).unwrap().len(), VOICES_BYTES as u64);
    }
}
