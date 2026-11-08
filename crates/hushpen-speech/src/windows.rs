#![allow(unsafe_code)]

use crate::{Pcm, SpeechEngine, SpeechError, Voice, VoiceQuality};
use hushpen_core::error::{SPEECH_UNAVAILABLE, SPEECH_VOICE_MISSING};
use windows::Media::SpeechSynthesis::SpeechSynthesizer;
use windows::Storage::Streams::DataReader;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows::core::HSTRING;

pub struct SystemVoice;

struct Apartment;

impl Apartment {
    fn new() -> Result<Self, SpeechError> {
        // SAFETY: calls run on the speech worker and every successful initialization is balanced.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|error| SpeechError::new(SPEECH_UNAVAILABLE, error.to_string()))?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: this thread owns one successful RoInitialize call.
        unsafe { RoUninitialize() };
    }
}

fn failed(error: windows::core::Error) -> SpeechError {
    SpeechError::failed(error.to_string())
}

impl SpeechEngine for SystemVoice {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        let _apartment = Apartment::new()?;
        let voices = SpeechSynthesizer::AllVoices()
            .map_err(|error| SpeechError::new(SPEECH_UNAVAILABLE, error.to_string()))?;
        voices
            .into_iter()
            .map(|voice| {
                Ok(Voice {
                    id: voice.Id().map_err(failed)?.to_string(),
                    name: voice.DisplayName().map_err(failed)?.to_string(),
                    language: voice.Language().map_err(failed)?.to_string(),
                    quality: VoiceQuality::Default,
                })
            })
            .collect()
    }

    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
        let _apartment = Apartment::new()?;
        let synth = SpeechSynthesizer::new().map_err(failed)?;
        if let Some(id) = voice {
            let voices = SpeechSynthesizer::AllVoices().map_err(failed)?;
            let voice = voices
                .into_iter()
                .find(|voice| voice.Id().is_ok_and(|value| value == id))
                .ok_or_else(|| {
                    SpeechError::new(
                        SPEECH_VOICE_MISSING,
                        "the system voice is no longer installed",
                    )
                })?;
            synth.SetVoice(&voice).map_err(failed)?;
        }
        synth
            .Options()
            .map_err(failed)?
            .SetSpeakingRate(f64::from(speed.clamp(crate::MIN_SPEED, crate::MAX_SPEED)))
            .map_err(failed)?;
        let operation = synth
            .SynthesizeTextToStreamAsync(&HSTRING::from(text))
            .map_err(failed)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while operation.Status().map_err(failed)?.0 == 0 {
            if std::time::Instant::now() >= deadline {
                let _ = operation.Cancel();
                return Err(SpeechError::failed(
                    "the system voice did not finish in time",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let stream = operation.GetResults().map_err(failed)?;
        let size = u32::try_from(stream.Size().map_err(failed)?)
            .map_err(|_| SpeechError::failed("the system voice returned too much audio"))?;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).map_err(failed)?)
            .map_err(failed)?;
        let load = reader.LoadAsync(size).map_err(failed)?;
        while load.Status().map_err(failed)?.0 == 0 {
            if std::time::Instant::now() >= deadline {
                let _ = load.Cancel();
                return Err(SpeechError::failed(
                    "the system voice audio did not arrive in time",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let loaded = load.GetResults().map_err(failed)?;
        if loaded != size {
            return Err(SpeechError::failed(
                "the system voice returned incomplete audio",
            ));
        }
        let mut bytes = vec![0; size as usize];
        reader.ReadBytes(&mut bytes).map_err(failed)?;
        decode(&bytes)
    }
}

fn decode(bytes: &[u8]) -> Result<Pcm, SpeechError> {
    let mut wav = hound::WavReader::new(std::io::Cursor::new(bytes))
        .map_err(|error| SpeechError::failed(error.to_string()))?;
    let spec = wav.spec();
    if spec.sample_format != hound::SampleFormat::Int
        || spec.bits_per_sample != 16
        || spec.channels == 0
        || spec.sample_rate == 0
    {
        return Err(SpeechError::failed(
            "the system voice returned an unsupported WAV format",
        ));
    }
    let samples = wav
        .samples::<i16>()
        .map(|sample| sample.map(|value| f32::from(value) / 32768.0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SpeechError::failed(error.to_string()))?;
    let mut mono = Vec::new();
    hushpen_audio::resample::downmix(&samples, usize::from(spec.channels), &mut mono);
    Ok(Pcm {
        rate: spec.sample_rate,
        samples: mono,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_wav_decodes_and_bad_audio_fails() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        {
            let mut wav = hound::WavWriter::new(
                &mut bytes,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 24000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            wav.write_sample(16384i16).unwrap();
            wav.finalize().unwrap();
        }
        assert_eq!(decode(bytes.get_ref()).unwrap().samples, [0.5]);
        assert!(decode(b"bad audio").is_err());
    }
}
