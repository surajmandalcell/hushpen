//! The macOS system voice: AVSpeechSynthesizer writes each chunk into PCM buffers instead of
//! playing it, so the same audio can be played, paused, and exported.

#![allow(unsafe_code)]

use crate::{Pcm, SpeechEngine, SpeechError, Voice, VoiceQuality};
use block2::RcBlock;
use hushpen_core::error::SPEECH_VOICE_MISSING;
use objc2::MainThreadMarker;
use objc2_avf_audio::{
    AVAudioBuffer, AVAudioCommonFormat, AVAudioPCMBuffer, AVSpeechBoundary, AVSpeechSynthesisVoice,
    AVSpeechSynthesisVoiceQuality, AVSpeechSynthesizer, AVSpeechUtterance,
    AVSpeechUtteranceDefaultSpeechRate, AVSpeechUtteranceMaximumSpeechRate,
    AVSpeechUtteranceMinimumSpeechRate,
};
use objc2_foundation::NSString;
use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::Duration;

/// How long the synthesizer may go without a buffer before the chunk fails. A chunk takes well
/// under a second.
const STALL: Duration = Duration::from_secs(15);

pub struct SystemVoice;

impl SpeechEngine for SystemVoice {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        // SAFETY: class methods and getters with no arguments.
        let voices = unsafe { AVSpeechSynthesisVoice::speechVoices() };
        Ok(voices
            .iter()
            .map(|voice| unsafe {
                Voice {
                    id: voice.identifier().to_string(),
                    name: voice.name().to_string(),
                    language: voice.language().to_string(),
                    quality: quality(voice.quality()),
                }
            })
            .collect())
    }

    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
        if MainThreadMarker::new().is_some() {
            return Err(SpeechError::failed(
                "the system voice was asked on the main thread",
            ));
        }
        // SAFETY: the utterance and the voice are valid objects that this thread owns.
        let utterance =
            unsafe { AVSpeechUtterance::speechUtteranceWithString(&NSString::from_str(text)) };
        if let Some(id) = voice {
            let found =
                unsafe { AVSpeechSynthesisVoice::voiceWithIdentifier(&NSString::from_str(id)) }
                    .ok_or_else(|| SpeechError::new(SPEECH_VOICE_MISSING, id))?;
            unsafe { utterance.setVoice(Some(&found)) };
        }
        unsafe { utterance.setRate(rate(speed)) };

        let (sender, buffers) = mpsc::channel();
        // The synthesizer calls this on the main queue, once for each buffer and last with an
        // empty buffer. So this waits on another thread while the main run loop runs, as it
        // does in the app; on the main thread it would wait forever.
        let callback = RcBlock::new(move |buffer: NonNull<AVAudioBuffer>| {
            // SAFETY: the buffer is valid for the duration of the call.
            let _ = sender.send(read_buffer(unsafe { buffer.as_ref() }));
        });
        let synthesizer = unsafe { AVSpeechSynthesizer::new() };
        // SAFETY: the synthesizer copies the block, and both live until the last buffer.
        unsafe {
            synthesizer.writeUtterance_toBufferCallback(&utterance, RcBlock::as_ptr(&callback))
        };

        let mut pcm = Pcm {
            rate: 0,
            samples: Vec::new(),
        };
        loop {
            match buffers.recv_timeout(STALL) {
                Ok(Some((_, samples))) if samples.is_empty() => break,
                Ok(Some((rate, samples))) => {
                    pcm.rate = rate;
                    pcm.samples.extend(samples);
                }
                Ok(None) => {
                    unsafe { synthesizer.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
                    return Err(SpeechError::failed("the voice wrote an unsupported format"));
                }
                Err(_) => {
                    unsafe { synthesizer.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
                    return Err(SpeechError::failed("the system voice stopped answering"));
                }
            }
        }
        if pcm.rate == 0 {
            return Err(SpeechError::failed("the system voice wrote no audio"));
        }
        Ok(pcm)
    }
}

fn quality(quality: AVSpeechSynthesisVoiceQuality) -> VoiceQuality {
    match quality {
        AVSpeechSynthesisVoiceQuality::Premium => VoiceQuality::Premium,
        AVSpeechSynthesisVoiceQuality::Enhanced => VoiceQuality::Enhanced,
        _ => VoiceQuality::Default,
    }
}

/// AVSpeech rates run from 0 to 1 with 0.5 as normal, so a speed factor scales the normal rate.
fn rate(speed: f32) -> f32 {
    // SAFETY: constant statics of the framework.
    let (normal, min, max) = unsafe {
        (
            AVSpeechUtteranceDefaultSpeechRate,
            AVSpeechUtteranceMinimumSpeechRate,
            AVSpeechUtteranceMaximumSpeechRate,
        )
    };
    (normal * speed).clamp(min, max)
}

/// The first channel of a PCM buffer as f32 with its rate, an empty list for the last buffer,
/// or `None` for a format this does not read.
fn read_buffer(buffer: &AVAudioBuffer) -> Option<(u32, Vec<f32>)> {
    let pcm = buffer.downcast_ref::<AVAudioPCMBuffer>()?;
    // SAFETY: the channel pointers hold `frameLength` frames `stride` samples apart, in the
    // sample type that `commonFormat` names.
    unsafe {
        let format = pcm.format();
        let rate = format.sampleRate() as u32;
        let frames = pcm.frameLength() as usize;
        let stride = pcm.stride().max(1);
        if frames == 0 {
            return Some((rate, Vec::new()));
        }
        let samples = match format.commonFormat() {
            AVAudioCommonFormat::PCMFormatFloat32 => {
                first_channel(pcm.floatChannelData(), frames, stride, |sample| sample)
            }
            AVAudioCommonFormat::PCMFormatInt16 => {
                first_channel(pcm.int16ChannelData(), frames, stride, |sample| {
                    f32::from(sample) / 32_768.0
                })
            }
            AVAudioCommonFormat::PCMFormatInt32 => {
                first_channel(pcm.int32ChannelData(), frames, stride, |sample| {
                    sample as f32 / 2_147_483_648.0
                })
            }
            _ => None,
        }?;
        Some((rate, samples))
    }
}

/// # Safety
///
/// `channels`, when not null, points at a channel pointer with `frames * stride` samples.
unsafe fn first_channel<T: Copy>(
    channels: *mut NonNull<T>,
    frames: usize,
    stride: usize,
    convert: impl Fn(T) -> f32,
) -> Option<Vec<f32>> {
    if channels.is_null() {
        return None;
    }
    // SAFETY: the caller's contract.
    let samples = unsafe { std::slice::from_raw_parts((*channels).as_ptr(), frames * stride) };
    Some(
        samples
            .iter()
            .step_by(stride)
            .map(|sample| convert(*sample))
            .collect(),
    )
}
