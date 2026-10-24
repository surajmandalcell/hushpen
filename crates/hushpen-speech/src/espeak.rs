//! The Linux system voice: espeak-ng, run once for each chunk. It is a program the user may
//! not have, so it is found at run time and never linked.

use crate::{Pcm, SpeechEngine, SpeechError, Voice, VoiceQuality};
use hushpen_audio::resample::downmix;
use hushpen_core::error::{SPEECH_FAILED, SPEECH_UNAVAILABLE, SPEECH_VOICE_MISSING};
use std::io::{self, Write};
use std::process::{Command, Output, Stdio};

const INSTALL_HINT: &str = "espeak-ng is not installed. Install it with the package manager, \
                            for example: sudo apt install espeak-ng";
/// The speed of espeak-ng's voices at speed 1.0, in words per minute.
const NORMAL_WPM: f32 = 175.0;

pub struct Espeak {
    program: &'static str,
}

impl Default for Espeak {
    fn default() -> Self {
        Self {
            program: "espeak-ng",
        }
    }
}

impl Espeak {
    /// Runs espeak-ng with `stdin` as its input and returns its output once it exits.
    fn run(&self, args: &[&str], stdin: &str) -> Result<Output, SpeechError> {
        let spawned = Command::new(self.program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(SpeechError::new(SPEECH_UNAVAILABLE, INSTALL_HINT));
            }
            Err(error) => return Err(SpeechError::new(SPEECH_UNAVAILABLE, error.to_string())),
        };
        // A chunk is far smaller than the pipe buffer, so this write cannot block on a child
        // that waits for its output to be read.
        if let Some(mut input) = child.stdin.take() {
            input
                .write_all(stdin.as_bytes())
                .map_err(|error| SpeechError::failed(format!("espeak-ng input: {error}")))?;
        }
        child
            .wait_with_output()
            .map_err(|error| SpeechError::failed(format!("espeak-ng: {error}")))
    }
}

impl SpeechEngine for Espeak {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        let output = self.run(&["--voices"], "")?;
        if !output.status.success() {
            return Err(SpeechError::failed(stderr_line(&output)));
        }
        Ok(parse_voices(&String::from_utf8_lossy(&output.stdout)))
    }

    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
        let wpm = ((NORMAL_WPM * speed).round() as u32).to_string();
        // `-b 1`: the input is UTF-8 whatever the locale says.
        let mut args = vec!["--stdout", "--stdin", "-b", "1", "-s", &wpm];
        if let Some(voice) = voice {
            args.extend(["-v", voice]);
        }
        let output = self.run(&args, text)?;
        let stderr = stderr_line(&output);
        if !output.status.success() || output.stdout.is_empty() {
            let code = if voice.is_some() && stderr.contains("voice") {
                SPEECH_VOICE_MISSING
            } else {
                SPEECH_FAILED
            };
            return Err(SpeechError::new(code, stderr));
        }
        parse_wav(&output.stdout)
    }
}

fn stderr_line(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr.trim();
    if line.is_empty() {
        format!("espeak-ng exited with {}", output.status)
    } else {
        line.to_owned()
    }
}

/// Reads the table that `espeak-ng --voices` prints:
/// `Pty Language Age/Gender VoiceName File Other Languages`, one voice on each line.
fn parse_voices(table: &str) -> Vec<Voice> {
    table
        .lines()
        .skip(1)
        .filter_map(|line| {
            let columns: Vec<&str> = line.split_whitespace().collect();
            let [_, language, _, name, file, ..] = columns[..] else {
                return None;
            };
            Some(Voice {
                id: file.to_owned(),
                name: name.replace('_', " "),
                language: language.to_owned(),
                quality: VoiceQuality::Default,
            })
        })
        .collect()
}

/// Reads the 16-bit PCM WAV that espeak-ng writes to stdout. It cannot seek back on a pipe,
/// so the data chunk keeps a placeholder size: the samples run to the end of the output.
fn parse_wav(bytes: &[u8]) -> Result<Pcm, SpeechError> {
    let bad = |what: &str| SpeechError::failed(format!("espeak-ng wrote {what}"));
    let u16_at = |at: usize| {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(bad("no WAV header"));
    }
    let (mut at, mut format) = (12, None);
    while let (Some(id), Some(size)) = (bytes.get(at..at + 4), u32_at(at + 4)) {
        let body = at + 8;
        if id == b"fmt " {
            let read = (u16_at(body), u16_at(body + 2), u32_at(body + 4));
            let bits = u16_at(body + 14);
            match (read, bits) {
                ((Some(1), Some(channels @ 1..), Some(rate @ 1..)), Some(16)) => {
                    format = Some((usize::from(channels), rate));
                }
                _ => return Err(bad("audio that is not 16-bit PCM")),
            }
        } else if id == b"data" {
            let Some((channels, rate)) = format else {
                return Err(bad("data before its format"));
            };
            let interleaved: Vec<f32> = bytes[body..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32_768.0)
                .collect();
            let mut samples = Vec::with_capacity(interleaved.len() / channels);
            downmix(&interleaved, channels, &mut samples);
            return Ok(Pcm { rate, samples });
        }
        at = body + size as usize + size as usize % 2;
    }
    Err(bad("no audio"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(channels: u16, rate: u32, samples: &[i16], data_size: u32) -> Vec<u8> {
        let mut bytes = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(channels.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * 2 * u32::from(channels)).to_le_bytes());
        bytes.extend((2 * channels).to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_size.to_le_bytes());
        for sample in samples {
            bytes.extend(sample.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn the_voice_table_becomes_voices() {
        let table = "Pty Language       Age/Gender VoiceName          File                 Other Languages\n \
                     5  af              --/M      Afrikaans          gmw/af\n \
                     2  en-us           --/M      English_(America)  gmw/en-US            (en 3)\n\
                     broken line\n";
        let voices = parse_voices(table);
        assert_eq!(voices.len(), 2);
        assert_eq!(
            voices[1],
            Voice {
                id: "gmw/en-US".into(),
                name: "English (America)".into(),
                language: "en-us".into(),
                quality: VoiceQuality::Default,
            }
        );
    }

    #[test]
    fn a_streamed_wav_reads_to_the_end_of_the_output() {
        let pcm = parse_wav(&wav(1, 22_050, &[0, 16_384, -32_768], 0x7fff_f000)).unwrap();
        assert_eq!(pcm.rate, 22_050);
        assert_eq!(pcm.samples, vec![0.0, 0.5, -1.0]);

        let stereo = parse_wav(&wav(2, 16_000, &[16_384, 0, 0, 0], 8)).unwrap();
        assert_eq!(stereo.samples, vec![0.25, 0.0]);
    }

    #[test]
    fn output_that_is_not_a_pcm_wav_is_an_error() {
        assert!(parse_wav(b"").is_err());
        assert!(parse_wav(b"RIFF\0\0\0\0WAVE").is_err());
        let mut float = wav(1, 22_050, &[0], 2);
        float[20] = 3;
        assert!(parse_wav(&float).is_err());
    }

    #[test]
    fn a_missing_program_is_unavailable_with_the_install_hint() {
        let engine = Espeak {
            program: "hushpen-no-such-espeak",
        };
        let error = engine.voices().unwrap_err();
        assert_eq!(error.code, SPEECH_UNAVAILABLE);
        assert!(error.detail.contains("apt install espeak-ng"));
        assert_eq!(
            engine.speak("Hi.", None, 1.0).unwrap_err().code,
            SPEECH_UNAVAILABLE
        );
    }
}
