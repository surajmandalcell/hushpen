//! Reads a media file the user picked and turns it into 16 kHz mono samples, one packet at a
//! time.
//!
//! The files come from outside, so only the engine child decodes: a decoder fault then costs
//! one job and not the app. The child also reports the length from the headers
//! ([`FileDecoder::duration_ms`]) before it decodes, so the app never parses the file. WAV, MP3,
//! FLAC, Ogg Vorbis, M4A (AAC-LC and ALAC), and AIFF decode. Opus, WebM, and video are named in
//! the error instead of being tried.

use crate::resample::{MonoResampler, TARGET_RATE, downmix};
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

/// Longest file the import accepts. Memory does not grow with the length, so this bounds only
/// the time one job can run.
pub const MAX_SECONDS: u64 = 12 * 3600;

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
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(text) | Self::Failed(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for DecodeError {}

fn failed(text: impl Into<String>) -> DecodeError {
    DecodeError::Failed(text.into())
}

/// A media file opened for decoding. Each [`FileDecoder::read`] decodes one packet, so the caller
/// holds only the samples it keeps, whatever the length of the file.
pub struct FileDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    resampler: Option<MonoResampler>,
    interleaved: Vec<f32>,
    mono: Vec<f32>,
    consumed: Arc<AtomicU64>,
    length: u64,
    /// The length of the audio as the container states it, when it does.
    duration_ms: Option<u64>,
    produced: u64,
    bad_packets: u32,
    ended: bool,
}

impl FileDecoder {
    /// Opens `path` and finds its audio track. Formats Hushpen does not read fail here.
    pub fn open(path: &Path) -> Result<Self, DecodeError> {
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
        let format = symphonia::default::get_probe()
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
        let duration_ms = track
            .time_base
            .zip(track.duration)
            .and_then(|(base, duration)| base.calc_duration(duration))
            .and_then(|time| u64::try_from(time.as_millis()).ok());
        let Some(params) = track.codec_params.as_ref().and_then(CodecParameters::audio) else {
            return Err(failed("The file has no audio track."));
        };
        let decoder = symphonia::default::get_codecs()
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
        Ok(Self {
            format,
            decoder,
            track_id,
            resampler: None,
            interleaved: Vec::new(),
            mono: Vec::new(),
            consumed,
            length,
            duration_ms,
            produced: 0,
            bad_packets: 0,
            ended: false,
        })
    }

    /// Decodes the next packet and appends its 16 kHz mono samples to `out`. A packet can add
    /// none. Returns `Ok(false)` once the file has no more audio.
    pub fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, DecodeError> {
        if self.ended {
            return Ok(false);
        }
        let before = out.len();
        let more = self.read_packet(out)?;
        if !more {
            self.ended = true;
            if let Some(resampler) = self.resampler.as_mut() {
                resampler.finish(out);
            }
        }
        self.produced += (out.len() - before) as u64;
        if self.produced > MAX_SECONDS * u64::from(TARGET_RATE) {
            return Err(failed(format!(
                "The file is longer than {} hours.",
                MAX_SECONDS / 3600
            )));
        }
        if !more && self.produced == 0 {
            return Err(failed("The file has no audio."));
        }
        Ok(more)
    }

    /// The length of the audio from the file's headers, or `None` when they do not say, as in
    /// an MP3 with no Xing header.
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// How much of the file has been read, from 0 to 1.
    pub fn fraction_read(&self) -> f32 {
        (self.consumed.load(Ordering::Relaxed) as f64 / self.length as f64).min(1.0) as f32
    }

    fn read_packet(&mut self, out: &mut Vec<f32>) -> Result<bool, DecodeError> {
        let packet = match self.format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) | Err(Error::ResetRequired) => return Ok(false),
            Err(Error::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(false);
            }
            Err(_) if self.produced > 0 => return Ok(false),
            Err(error) => return Err(failed(error.to_string())),
        };
        if packet.track_id != self.track_id {
            return Ok(true);
        }
        let buffer = match self.decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(Error::DecodeError(_)) => {
                self.bad_packets += 1;
                if self.bad_packets > MAX_BAD_PACKETS && self.produced == 0 {
                    return Err(failed("The audio in the file is damaged."));
                }
                return Ok(true);
            }
            Err(Error::IoError(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(false);
            }
            Err(error) => return Err(failed(error.to_string())),
        };
        let channels = buffer.spec().channels().count().max(1);
        let rate = buffer.spec().rate();
        let resampler = match self.resampler.as_mut() {
            Some(resampler) => resampler,
            None => self
                .resampler
                .insert(MonoResampler::new(rate).map_err(|error| failed(error.to_string()))?),
        };
        self.interleaved.clear();
        buffer.copy_to_vec_interleaved::<f32>(&mut self.interleaved);
        self.mono.clear();
        downmix(&self.interleaved, channels, &mut self.mono);
        resampler.process(&self.mono, out);
        Ok(true)
    }
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

    fn run(path: &Path) -> Result<Vec<f32>, DecodeError> {
        let mut decoder = FileDecoder::open(path)?;
        let mut out = Vec::new();
        while decoder.read(&mut out)? {}
        Ok(out)
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
    fn the_headers_give_the_length_before_any_packet_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("two-seconds.wav");
        write_wav(&path, 44_100, 2, 88_200);
        let decoder = FileDecoder::open(&path).unwrap();
        assert_eq!(decoder.duration_ms(), Some(2_000));
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
    fn the_fraction_read_runs_up_to_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, 16_000, 1, 160_000);
        let mut decoder = FileDecoder::open(&path).unwrap();
        let mut all = vec![decoder.fraction_read()];
        while decoder.read(&mut Vec::new()).unwrap() {
            all.push(decoder.fraction_read());
        }
        assert!(all[0] < 0.1, "{all:?}");
        assert_eq!(all.last(), Some(&1.0));
        assert!(all.windows(2).all(|pair| pair[0] <= pair[1]), "{all:?}");
    }

    #[test]
    fn odd_rates_and_channel_counts_resample_to_the_exact_length() {
        let dir = tempfile::tempdir().unwrap();
        for (rate, channels) in [
            (8_000, 1),
            (11_025, 2),
            (22_050, 1),
            (32_000, 3),
            (48_000, 2),
        ] {
            let path = dir.path().join(format!("{rate}-{channels}.wav"));
            let frames = rate as usize * 3 + 77;
            write_wav(&path, rate, channels, frames);
            let samples = run(&path).unwrap();
            let expected = (frames as u64 * 16_000 + u64::from(rate) / 2) / u64::from(rate);
            assert_eq!(
                samples.len() as u64,
                expected,
                "{rate} Hz, {channels} channels"
            );
            assert!(samples.iter().any(|sample| sample.abs() > 0.1));
        }
    }

    /// Five minutes at 44.1 kHz come out in reads of a few milliseconds each, so a caller that
    /// keeps one window holds one window, however long the file is.
    #[test]
    fn a_long_file_streams_in_small_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long.wav");
        write_wav(&path, 44_100, 1, 44_100 * 300);
        let mut decoder = FileDecoder::open(&path).unwrap();
        let (mut total, mut largest) = (0, 0);
        let mut out = Vec::new();
        loop {
            out.clear();
            let more = decoder.read(&mut out).unwrap();
            total += out.len();
            largest = largest.max(out.len());
            if !more {
                break;
            }
        }
        assert_eq!(total, 16_000 * 300);
        assert!(largest <= 16_000 / 4, "one read gave {largest} samples");
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
