//! Reads a media file the user picked and turns it into 16 kHz mono samples.
//!
//! The files come from outside, so only the engine child calls this: a decoder fault then costs
//! one job and not the app. WAV, MP3, FLAC, Ogg Vorbis, M4A (AAC-LC and ALAC), and AIFF decode.
//! Opus, WebM, and video are named in the error instead of being tried.

use crate::resample::{MonoResampler, TARGET_RATE, downmix};
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

/// Longest file the import accepts. The decoded samples stay in memory, 64 KB for each second.
pub const MAX_SECONDS: u64 = 5 * 3600;

/// How many bytes at the start of a file tell Opus, WebM, and AVI from the rest.
const SNIFF_BYTES: usize = 512;

/// Packets that fail to decode are skipped. A file where this many fail before any sound is
/// treated as corrupt.
const MAX_BAD_PACKETS: u32 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A format Hushpen does not read. The text is a sentence for the user that names it.
    Unsupported(String),
    /// The file is empty, damaged, or not audio. The text is for the log and the detail line.
    Failed(String),
    Cancelled,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(text) | Self::Failed(text) => f.write_str(text),
            Self::Cancelled => f.write_str("the import was cancelled"),
        }
    }
}

impl std::error::Error for DecodeError {}

fn failed(text: impl Into<String>) -> DecodeError {
    DecodeError::Failed(text.into())
}

/// Decodes `path` to 16 kHz mono. `cancelled` is asked between packets. `progress` gets the
/// fraction of the file read, from 0 to 1, whenever it moves by a percent.
pub fn decode_file(
    path: &Path,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(f32),
) -> Result<Vec<f32>, DecodeError> {
    let (mut file, length) = open_regular(path)?;
    if length == 0 {
        return Err(failed("The file is empty."));
    }
    let mut head = vec![0; SNIFF_BYTES];
    let got = read_up_to(&mut file, &mut head).map_err(|error| failed(read_message(&error)))?;
    head.truncate(got);
    if let Some(text) = sniff_unsupported(&head) {
        return Err(DecodeError::Unsupported(text.to_owned()));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| failed(read_message(&error)))?;

    let consumed = Arc::new(AtomicU64::new(0));
    let source = Counted {
        file,
        length,
        consumed: Arc::clone(&consumed),
    };
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|text| text.to_str()) {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| match error {
            Error::IoError(error) => failed(read_message(&error)),
            _ => failed("The file is not an audio file Hushpen can read, or it is damaged."),
        })?;

    if format
        .tracks()
        .iter()
        .any(|track| matches!(track.codec_params, Some(CodecParameters::Video(_))))
    {
        return Err(DecodeError::Unsupported(VIDEO.to_owned()));
    }
    let Some(track) = format
        .default_track(TrackType::Audio)
        .or_else(|| format.first_track_known_codec(TrackType::Audio))
    else {
        return Err(failed("The file has no audio track."));
    };
    let track_id = track.id;
    let Some(params) = track.codec_params.as_ref().and_then(CodecParameters::audio) else {
        return Err(failed("The file has no audio track."));
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|error| match error {
            Error::Unsupported(_) if params.codec == CODEC_ID_OPUS => {
                DecodeError::Unsupported(OPUS.to_owned())
            }
            Error::Unsupported(_) => {
                DecodeError::Unsupported("This audio format is not supported yet.".to_owned())
            }
            other => failed(other.to_string()),
        })?;

    let mut out: Vec<f32> = Vec::new();
    let mut resampler: Option<MonoResampler> = None;
    let mut interleaved: Vec<f32> = Vec::new();
    let mut mono: Vec<f32> = Vec::new();
    let mut bad_packets = 0_u32;
    let mut last_reported = 0.0_f32;
    let limit = MAX_SECONDS * u64::from(TARGET_RATE);
    progress(0.0);
    loop {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(Error::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(_) if !out.is_empty() => break,
            Err(error) => return Err(failed(error.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(Error::DecodeError(_)) => {
                bad_packets += 1;
                if bad_packets > MAX_BAD_PACKETS && out.is_empty() {
                    return Err(failed("The audio in the file is damaged."));
                }
                continue;
            }
            Err(Error::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(failed(error.to_string())),
        };
        let channels = buffer.spec().channels().count().max(1);
        let rate = buffer.spec().rate();
        if resampler.is_none() {
            resampler = Some(MonoResampler::new(rate).map_err(|error| failed(error.to_string()))?);
        }
        interleaved.clear();
        buffer.copy_to_vec_interleaved::<f32>(&mut interleaved);
        mono.clear();
        downmix(&interleaved, channels, &mut mono);
        if let Some(resampler) = resampler.as_mut() {
            resampler.process(&mono, &mut out);
        }
        if out.len() as u64 > limit {
            return Err(failed(format!(
                "The file is longer than {} hours.",
                MAX_SECONDS / 3600
            )));
        }
        let done = (consumed.load(Ordering::Relaxed) as f64 / length as f64).min(1.0) as f32;
        if done - last_reported >= 0.01 {
            last_reported = done;
            progress(done);
        }
    }
    if let Some(resampler) = resampler.as_mut() {
        resampler.finish(&mut out);
    }
    if out.is_empty() {
        return Err(failed("The file has no audio."));
    }
    progress(1.0);
    Ok(out)
}

const OPUS: &str = "Opus is not supported yet.";
const VIDEO: &str = "Video files are not supported yet. Import the audio as a separate file.";

/// Names the formats that must not reach the decoders: they could not be read anyway, and a
/// clear name is better than "damaged".
fn sniff_unsupported(head: &[u8]) -> Option<&'static str> {
    if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        let webm = head.windows(4).any(|window| window == b"webm");
        return Some(if webm {
            "WebM is not supported yet."
        } else {
            "Matroska video is not supported yet."
        });
    }
    if head.starts_with(b"OggS") && head.windows(8).any(|window| window == b"OpusHead") {
        return Some(OPUS);
    }
    if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"AVI ") {
        return Some(VIDEO);
    }
    None
}

/// Opens a regular file. A device or a FIFO is refused before it is opened: opening a FIFO
/// for reading waits for a writer, and a device such as `/dev/zero` never ends.
pub fn open_regular(path: &Path) -> Result<(File, u64), DecodeError> {
    let metadata = std::fs::metadata(path).map_err(|error| failed(read_message(&error)))?;
    if !metadata.is_file() {
        return Err(failed("That is not a file that can be opened."));
    }
    let file = File::open(path).map_err(|error| failed(read_message(&error)))?;
    let opened = file
        .metadata()
        .map_err(|error| failed(read_message(&error)))?;
    if !opened.is_file() {
        return Err(failed("That is not a file that can be opened."));
    }
    Ok((file, opened.len()))
}

fn read_message(error: &io::Error) -> String {
    format!("The file could not be read: {error}.")
}

fn read_up_to(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..])? {
            0 => break,
            count => filled += count,
        }
    }
    Ok(filled)
}

/// A file that counts the bytes read, so the decoder loop can report how far it is.
struct Counted {
    file: File,
    length: u64,
    consumed: Arc<AtomicU64>,
}

impl Read for Counted {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.file.read(buffer)?;
        self.consumed.fetch_add(count as u64, Ordering::Relaxed);
        Ok(count)
    }
}

impl Seek for Counted {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let position = self.file.seek(to)?;
        self.consumed.store(position, Ordering::Relaxed);
        Ok(position)
    }
}

impl MediaSource for Counted {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavSpec, WavWriter};
    use std::cell::Cell;

    fn run(path: &Path) -> Result<Vec<f32>, DecodeError> {
        decode_file(path, &|| false, &mut |_| {})
    }

    fn write_wav(path: &Path, rate: u32, channels: u16, frames: usize) {
        let spec = WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(path, spec).unwrap();
        for frame in 0..frames {
            let value = ((frame as f32 * 0.05).sin() * 8000.0) as i16;
            for _ in 0..channels {
                writer.write_sample(value).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    /// A 16-bit AIFF with `frames` mono frames at 16 kHz. The rate is the 80-bit float of 16000.
    fn aiff_bytes(frames: u32) -> Vec<u8> {
        let mut data = Vec::new();
        for frame in 0..frames {
            let value = ((frame as f32 * 0.05).sin() * 8000.0) as i16;
            data.extend_from_slice(&value.to_be_bytes());
        }
        let mut comm = Vec::new();
        comm.extend_from_slice(&1_i16.to_be_bytes());
        comm.extend_from_slice(&frames.to_be_bytes());
        comm.extend_from_slice(&16_i16.to_be_bytes());
        comm.extend_from_slice(&[0x40, 0x0C, 0xFA, 0, 0, 0, 0, 0, 0, 0]);
        let mut ssnd = vec![0; 8];
        ssnd.extend_from_slice(&data);
        let mut body = b"AIFF".to_vec();
        body.extend_from_slice(b"COMM");
        body.extend_from_slice(&(comm.len() as u32).to_be_bytes());
        body.extend_from_slice(&comm);
        body.extend_from_slice(b"SSND");
        body.extend_from_slice(&(ssnd.len() as u32).to_be_bytes());
        body.extend_from_slice(&ssnd);
        let mut file = b"FORM".to_vec();
        file.extend_from_slice(&(body.len() as u32).to_be_bytes());
        file.extend_from_slice(&body);
        file
    }

    #[test]
    fn a_stereo_44k_wav_becomes_mono_16k_with_the_right_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, 44_100, 2, 44_100);
        let samples = run(&path).unwrap();
        assert_eq!(samples.len(), 16_000);
        assert!(samples.iter().any(|sample| sample.abs() > 0.1));
    }

    #[test]
    fn a_16k_wav_passes_through_untouched_in_length() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, 16_000, 1, 8_000);
        assert_eq!(run(&path).unwrap().len(), 8_000);
    }

    #[test]
    fn an_aiff_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.aiff");
        std::fs::write(&path, aiff_bytes(8_000)).unwrap();
        let samples = run(&path).unwrap();
        assert_eq!(samples.len(), 8_000);
        assert!(samples.iter().any(|sample| sample.abs() > 0.1));
    }

    #[test]
    fn the_extension_does_not_decide_the_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("really-a-wav.mp3");
        write_wav(&path, 16_000, 1, 4_000);
        assert_eq!(run(&path).unwrap().len(), 4_000);
    }

    #[test]
    fn progress_runs_from_zero_to_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, 16_000, 1, 160_000);
        let seen = Cell::new(Vec::new());
        let mut report = |fraction: f32| {
            let mut all = seen.take();
            all.push(fraction);
            seen.set(all);
        };
        decode_file(&path, &|| false, &mut report).unwrap();
        let all = seen.take();
        assert_eq!(all.first(), Some(&0.0));
        assert_eq!(all.last(), Some(&1.0));
        assert!(all.windows(2).all(|pair| pair[0] <= pair[1]), "{all:?}");
    }

    #[test]
    fn an_empty_file_is_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(
            run(&path),
            Err(DecodeError::Failed("The file is empty.".into()))
        );
    }

    #[test]
    fn a_wav_header_with_no_samples_is_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("none.wav");
        write_wav(&path, 16_000, 1, 0);
        assert!(matches!(run(&path), Err(DecodeError::Failed(_))));
    }

    #[test]
    fn text_and_random_bytes_are_failures_not_panics() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("corrupt-text-as.wav");
        std::fs::write(&text, "this is not audio at all, only words\n".repeat(50)).unwrap();
        assert!(matches!(run(&text), Err(DecodeError::Failed(_))));
        let noise = dir.path().join("corrupt-random.mp3");
        let bytes: Vec<u8> = (0..4096_u32)
            .map(|n| (n.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        std::fs::write(&noise, bytes).unwrap();
        assert!(matches!(run(&noise), Err(DecodeError::Failed(_))));
    }

    #[test]
    fn webm_opus_and_avi_are_named_as_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let cases: [(&str, Vec<u8>, &str); 4] = [
            (
                "a.webm",
                [&[0x1A, 0x45, 0xDF, 0xA3][..], &[0; 8], b"webm", &[0; 32]].concat(),
                "WebM is not supported yet.",
            ),
            (
                "a.mkv",
                [&[0x1A, 0x45, 0xDF, 0xA3][..], &[0; 8], b"matroska"].concat(),
                "Matroska video is not supported yet.",
            ),
            (
                "a.opus",
                [&b"OggS"[..], &[0; 24], b"OpusHead", &[0; 16]].concat(),
                "Opus is not supported yet.",
            ),
            (
                "a.avi",
                [&b"RIFF"[..], &[0; 4], b"AVI LIST"].concat(),
                VIDEO,
            ),
        ];
        for (name, bytes, text) in cases {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            assert_eq!(
                run(&path),
                Err(DecodeError::Unsupported(text.into())),
                "{name}"
            );
        }
    }

    #[test]
    fn a_cancel_stops_the_decode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, 16_000, 1, 16_000);
        assert_eq!(
            decode_file(&path, &|| true, &mut |_| {}),
            Err(DecodeError::Cancelled)
        );
    }

    #[test]
    fn a_folder_and_a_missing_file_are_failures() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(run(dir.path()), Err(DecodeError::Failed(_))));
        assert!(matches!(
            run(&dir.path().join("missing.wav")),
            Err(DecodeError::Failed(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_device_is_refused_without_reading_it() {
        let started = std::time::Instant::now();
        assert!(matches!(
            run(Path::new("/dev/zero")),
            Err(DecodeError::Failed(_))
        ));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
