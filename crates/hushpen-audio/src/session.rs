//! The crash-safe session WAV: 16 kHz, mono, 16-bit, in `cache/sessions`.
//!
//! The header is patched about once a second, so a killed process leaves a
//! file that decoders read up to the last patch. [`recover_orphans`] repairs
//! the header of such a file at the next start.

use crate::resample::TARGET_RATE;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Samples between header patches: one second.
const CHECKPOINT_SAMPLES: u64 = TARGET_RATE as u64;

/// Orphans older than this are deleted instead of recovered.
pub const ORPHAN_MAX_AGE_MS: u64 = 24 * 60 * 60 * 1000;

const BYTES_PER_SAMPLE: u64 = 2;

pub fn spec() -> WavSpec {
    WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    }
}

pub struct SessionWriter {
    writer: WavWriter<BufWriter<File>>,
    samples: u64,
    since_checkpoint: u64,
}

impl SessionWriter {
    pub fn create(path: &Path) -> Result<Self, hound::Error> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(hound::Error::IoError)?;
        let mut writer = WavWriter::new(BufWriter::new(file), spec())?;
        // The header is valid from the first moment, even before any audio.
        writer.flush()?;
        Ok(Self {
            writer,
            samples: 0,
            since_checkpoint: 0,
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<(), hound::Error> {
        for sample in samples {
            let value = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            self.writer.write_sample(value)?;
        }
        self.samples += samples.len() as u64;
        self.since_checkpoint += samples.len() as u64;
        if self.since_checkpoint >= CHECKPOINT_SAMPLES {
            self.since_checkpoint = 0;
            self.writer.flush()?;
        }
        Ok(())
    }

    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// Fixes the header and closes the file. Returns the sample count.
    pub fn finalize(self) -> Result<u64, hound::Error> {
        let samples = self.samples;
        self.writer.finalize()?;
        Ok(samples)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredSession {
    pub path: PathBuf,
    /// File name without `.wav`.
    pub id: String,
    pub duration_ms: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Sweep {
    pub recovered: Vec<RecoveredSession>,
    /// Empty, unreadable, or older than 24 hours.
    pub removed: Vec<PathBuf>,
}

/// Looks at every `*.wav` in `dir`. Files with audio and an age under 24 hours
/// get their header fixed to the real data size and are returned. Header-only
/// files, files that are not 16 kHz mono 16-bit WAVs, and old files are deleted.
pub fn recover_orphans(dir: &Path, now_ms: u64) -> io::Result<Sweep> {
    let mut sweep = Sweep::default();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(sweep),
        Err(error) => return Err(error),
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "wav"))
        .collect();
    paths.sort();
    for path in paths {
        let modified_ms = fs::metadata(&path)?
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |age| age.as_millis() as u64);
        let too_old = now_ms.saturating_sub(modified_ms) > ORPHAN_MAX_AGE_MS;
        let repaired = if too_old { None } else { repair(&path)? };
        match repaired {
            Some(samples) if samples > 0 => {
                let id = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_default();
                sweep.recovered.push(RecoveredSession {
                    path,
                    id,
                    duration_ms: samples * 1000 / u64::from(TARGET_RATE),
                });
            }
            _ => {
                fs::remove_file(&path)?;
                sweep.removed.push(path);
            }
        }
    }
    Ok(sweep)
}

/// Rewrites the two size fields to the bytes that are really in the file and
/// cuts a half sample off the end. Returns the sample count, or `None` when
/// the file is not a 16 kHz mono 16-bit WAV.
fn repair(path: &Path) -> io::Result<Option<u64>> {
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let Some(layout) = read_layout(&mut file)? else {
        return Ok(None);
    };
    let length = file.metadata()?.len();
    let data_bytes = length.saturating_sub(layout.data_start) / BYTES_PER_SAMPLE * BYTES_PER_SAMPLE;
    let end = layout.data_start + data_bytes;
    if end != length {
        file.set_len(end)?;
    }
    let (Ok(riff), Ok(data)) = (u32::try_from(end - 8), u32::try_from(data_bytes)) else {
        return Ok(None);
    };
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&riff.to_le_bytes())?;
    file.seek(SeekFrom::Start(layout.data_size_at))?;
    file.write_all(&data.to_le_bytes())?;
    file.sync_all()?;
    Ok(Some(data_bytes / BYTES_PER_SAMPLE))
}

struct Layout {
    data_start: u64,
    data_size_at: u64,
}

fn read_layout(file: &mut File) -> io::Result<Option<Layout>> {
    let mut head = [0u8; 12];
    if file.read_exact(&mut head).is_err() || &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Ok(None);
    }
    let mut format_ok = false;
    let mut position = 12u64;
    // A session header has two chunks. Anything with many more is not ours.
    for _ in 0..8 {
        let mut chunk = [0u8; 8];
        if file.read_exact(&mut chunk).is_err() {
            return Ok(None);
        }
        let size = u64::from(u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]));
        match &chunk[0..4] {
            b"fmt " => {
                let mut body = vec![0u8; size.min(64) as usize];
                if file.read_exact(&mut body).is_err() || body.len() < 16 {
                    return Ok(None);
                }
                let field = |at: usize| u16::from_le_bytes([body[at], body[at + 1]]);
                let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                format_ok =
                    field(0) == 1 && field(2) == 1 && rate == TARGET_RATE && field(14) == 16;
            }
            b"data" => {
                return Ok(format_ok.then_some(Layout {
                    data_start: position + 8,
                    data_size_at: position + 4,
                }));
            }
            _ => {}
        }
        position += 8 + size + (size & 1);
        file.seek(SeekFrom::Start(position))?;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn write_session(path: &Path, samples: usize) {
        let mut writer = SessionWriter::create(path).unwrap();
        let tone: Vec<f32> = (0..samples)
            .map(|i| ((i % 100) as f32 / 100.0) - 0.5)
            .collect();
        writer.write(&tone).unwrap();
        writer.finalize().unwrap();
    }

    fn mtime_ms(path: &Path) -> u64 {
        fs::metadata(path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// What a `kill -9` leaves: the header from the last checkpoint, then more
    /// bytes the OS had already received.
    fn killed_session(path: &Path, checkpointed: usize, extra_bytes: usize) {
        write_session(path, checkpointed);
        let mut file = OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(&vec![7u8; extra_bytes]).unwrap();
    }

    fn header_sizes(path: &Path) -> (u32, u32) {
        let bytes = fs::read(path).unwrap();
        let riff = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let data = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        (riff, data)
    }

    #[test]
    fn a_finalized_file_is_16k_mono_16_bit_with_the_right_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_session(&path, 24_000);
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec(), spec());
        assert_eq!(reader.duration(), 24_000);
    }

    #[test]
    fn the_header_is_valid_before_any_audio_and_after_each_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let mut writer = SessionWriter::create(&path).unwrap();
        assert_eq!(hound::WavReader::open(&path).unwrap().duration(), 0);
        writer.write(&vec![0.1; 16_000]).unwrap();
        writer.write(&vec![0.1; 5_000]).unwrap();
        // Read while the writer is still open, as after a kill.
        assert_eq!(hound::WavReader::open(&path).unwrap().duration(), 16_000);
        drop(writer);
    }

    #[test]
    fn a_killed_session_is_repaired_to_its_real_data_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kill.wav");
        killed_session(&path, 16_000 * 9, 16_001);
        let now = mtime_ms(&path);

        let sweep = recover_orphans(dir.path(), now).unwrap();
        assert!(sweep.removed.is_empty());
        let [recovered] = sweep.recovered.as_slice() else {
            panic!("{sweep:?}")
        };
        assert_eq!(recovered.id, "kill");
        assert_eq!(recovered.duration_ms, 9_500);
        let (riff, data) = header_sizes(&path);
        assert_eq!(data as u64, 16_000 * 9 * 2 + 16_000);
        assert_eq!(riff, data + 36);
        assert_eq!(fs::metadata(&path).unwrap().len(), u64::from(data) + 44);
        assert_eq!(
            hound::WavReader::open(&path).unwrap().duration(),
            16_000 * 9 + 8_000
        );
    }

    #[test]
    fn a_header_only_file_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        SessionWriter::create(&path).unwrap();
        let sweep = recover_orphans(dir.path(), mtime_ms(&path)).unwrap();
        assert!(sweep.recovered.is_empty());
        assert_eq!(sweep.removed, vec![path.clone()]);
        assert!(!path.exists());
    }

    #[test]
    fn a_file_older_than_24_hours_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.wav");
        write_session(&path, 16_000);
        let young =
            recover_orphans(dir.path(), mtime_ms(&path) + ORPHAN_MAX_AGE_MS - 1000).unwrap();
        assert_eq!(young.recovered.len(), 1);
        let old = recover_orphans(dir.path(), mtime_ms(&path) + ORPHAN_MAX_AGE_MS + 1000).unwrap();
        assert_eq!(old.removed, vec![path.clone()]);
        assert!(!path.exists());
    }

    #[test]
    fn a_file_that_is_not_a_session_wav_is_deleted_and_other_files_are_left() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk.wav");
        fs::write(&junk, b"not a wav at all, just text").unwrap();
        let stereo = dir.path().join("stereo.wav");
        let mut w = WavWriter::create(
            &stereo,
            WavSpec {
                channels: 2,
                ..spec()
            },
        )
        .unwrap();
        w.write_sample(1i16).unwrap();
        w.write_sample(2i16).unwrap();
        w.finalize().unwrap();
        let note = dir.path().join("notes.txt");
        fs::write(&note, b"keep").unwrap();

        let sweep = recover_orphans(dir.path(), mtime_ms(&junk)).unwrap();
        assert_eq!(sweep.removed.len(), 2);
        assert!(!junk.exists() && !stereo.exists());
        assert!(note.exists());
    }

    #[test]
    fn a_clean_file_is_returned_unchanged_and_a_second_sweep_agrees() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ok.wav");
        write_session(&path, 32_000);
        let before = fs::read(&path).unwrap();
        let now = mtime_ms(&path);
        let first = recover_orphans(dir.path(), now).unwrap();
        let second = recover_orphans(dir.path(), now).unwrap();
        assert_eq!(first.recovered, second.recovered);
        assert_eq!(first.recovered[0].duration_ms, 2_000);
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_missing_folder_is_an_empty_sweep() {
        let dir = tempfile::tempdir().unwrap();
        let sweep = recover_orphans(&dir.path().join("none"), 0).unwrap();
        assert_eq!(sweep, Sweep::default());
    }
}
