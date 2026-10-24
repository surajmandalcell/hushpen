//! The pipe protocol between the app and the `hushpen-tts` worker.
//!
//! It uses the frame codec of [`crate::protocol`] and the message shape of
//! [`crate::llm_protocol`]. Audio is the one payload that is not JSON: an `audio` event is
//! followed at once by one binary frame of mono `f32` little-endian samples. [`write_event`] and
//! [`read_event`] keep the two frames together. The text to speak travels inside `Speak`; no log
//! line ever holds it.

use crate::llm_protocol::UnloadReason;
use crate::protocol::{
    DecodeError, Fields, Frame, FrameError, FrameKind, Params, parse_payload, read_frame,
    write_frame, write_json,
};
use serde_json::{Map, Value, json};
use std::fmt;
use std::io::{Read, Write};

/// Bumped when a message changes shape. `Hello` carries it and `Ready` answers with it.
pub const TTS_PROTOCOL_VERSION: u32 = 1;

/// Default idle time before the worker drops the model, in seconds.
pub const DEFAULT_IDLE_SECS: u64 = 300;

/// The most samples in one audio frame: 10 s at 24 kHz, under 1 MB.
pub const MAX_AUDIO_SAMPLES: usize = 240_000;

/// The speed range, as a factor of the voice's normal speed.
pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 2.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceInfo {
    /// What `Speak` takes, for example `af_heart`.
    pub id: String,
    pub name: String,
    /// A BCP 47 tag such as `en-US`.
    pub language: String,
}

impl VoiceInfo {
    fn to_json(&self) -> Value {
        json!({"id": self.id, "name": self.name, "language": self.language})
    }

    fn from_json(value: &Value) -> Result<Self, DecodeError> {
        let fields = Fields::of(value)?;
        Ok(Self {
            id: fields.string("id")?,
            name: fields.string("name")?,
            language: fields.string("language")?,
        })
    }
}

/// App to worker.
#[derive(Debug, Clone, PartialEq)]
pub enum TtsRequest {
    Hello {
        protocol: u32,
    },
    /// Loads (or reloads) the model in `dir` on the CPU. The worker keeps the settings, so after
    /// an idle unload the next job loads the same model again without a new `LoadModel`.
    LoadModel {
        dir: String,
        threads: u32,
        /// Seconds without a job before the model is dropped. `0` keeps it for good.
        idle_secs: u64,
    },
    ListVoices,
    /// Speaks one chunk of plain text. `voice` is a `VoiceInfo::id`, `None` the default voice.
    Speak {
        job: u64,
        text: String,
        voice: Option<String>,
        speed: f32,
    },
    Cancel {
        job: u64,
    },
    /// Drops the model now. A later job loads it again.
    Unload,
    Ping,
    Shutdown,
}

impl TtsRequest {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Hello { protocol } => json!({"type": "hello", "protocol": protocol}),
            Self::LoadModel {
                dir,
                threads,
                idle_secs,
            } => json!({
                "type": "load_model", "dir": dir, "threads": threads, "idle_secs": idle_secs,
            }),
            Self::ListVoices => json!({"type": "list_voices"}),
            Self::Speak {
                job,
                text,
                voice,
                speed,
            } => json!({
                "type": "speak", "job": job, "text": text, "voice": voice, "speed": speed,
            }),
            Self::Cancel { job } => json!({"type": "cancel", "job": job}),
            Self::Unload => json!({"type": "unload"}),
            Self::Ping => json!({"type": "ping"}),
            Self::Shutdown => json!({"type": "shutdown"}),
        }
    }

    pub fn from_json(value: &Value) -> Result<Self, DecodeError> {
        let fields = Fields::of(value)?;
        match fields.kind()?.as_str() {
            "hello" => Ok(Self::Hello {
                protocol: fields.u32("protocol")?,
            }),
            "load_model" => Ok(Self::LoadModel {
                dir: fields.string("dir")?,
                threads: fields.u32("threads")?,
                idle_secs: fields
                    .optional_u64("idle_secs")?
                    .unwrap_or(DEFAULT_IDLE_SECS),
            }),
            "list_voices" => Ok(Self::ListVoices),
            "speak" => {
                let speed = fields.f64("speed")? as f32;
                if !(MIN_SPEED..=MAX_SPEED).contains(&speed) {
                    return Err(Fields::wrong("speed", "between 0.5 and 2"));
                }
                Ok(Self::Speak {
                    job: fields.u64("job")?,
                    text: fields.string("text")?,
                    voice: fields.optional_string("voice")?,
                    speed,
                })
            }
            "cancel" => Ok(Self::Cancel {
                job: fields.u64("job")?,
            }),
            "unload" => Ok(Self::Unload),
            "ping" => Ok(Self::Ping),
            "shutdown" => Ok(Self::Shutdown),
            other => Err(DecodeError::UnknownType(other.to_owned())),
        }
    }

    pub fn from_frame(frame: &Frame) -> Result<Self, DecodeError> {
        Self::from_json(&parse_payload(frame)?)
    }
}

/// What a finished job cost, for `tts.log`. Times only, never text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpeakStats {
    /// Length of the audio the job wrote.
    pub audio_ms: u64,
    /// Time from the start of the job to its end, including a reload after an idle unload.
    pub ms: u64,
    /// Part of `ms` spent loading the model, `0` when it was warm.
    pub load_ms: u64,
    /// Time from the start of the job to its first audio.
    pub first_audio_ms: u64,
}

impl SpeakStats {
    fn put(&self, object: &mut Value) {
        object["audio_ms"] = json!(self.audio_ms);
        object["ms"] = json!(self.ms);
        object["load_ms"] = json!(self.load_ms);
        object["first_audio_ms"] = json!(self.first_audio_ms);
    }

    fn read(fields: &Fields<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            audio_ms: fields.u64("audio_ms")?,
            ms: fields.u64("ms")?,
            load_ms: fields.u64("load_ms")?,
            first_audio_ms: fields.u64("first_audio_ms")?,
        })
    }
}

/// Worker to app.
#[derive(Debug, Clone, PartialEq)]
pub enum TtsEvent {
    Ready {
        version: String,
        protocol: u32,
    },
    Loading {
        model: String,
    },
    Loaded {
        model: String,
        ms: u64,
    },
    Unloaded {
        reason: UnloadReason,
    },
    Voices {
        voices: Vec<VoiceInfo>,
    },
    /// The next piece of a job's audio, at most [`MAX_AUDIO_SAMPLES`] samples.
    Audio {
        job: u64,
        sample_rate: u32,
        samples: Vec<f32>,
    },
    Done {
        job: u64,
        stats: SpeakStats,
    },
    Cancelled {
        job: u64,
    },
    Error {
        /// `None` for an error that belongs to no job, such as a failed model load.
        job: Option<u64>,
        code: String,
        params: Params,
    },
    Pong,
}

impl TtsEvent {
    pub fn error(job: Option<u64>, code: &str, detail: impl Into<String>) -> Self {
        let mut params = Map::new();
        params.insert("detail".into(), Value::String(detail.into()));
        Self::Error {
            job,
            code: code.to_owned(),
            params,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Ready { version, protocol } => {
                json!({"type": "ready", "version": version, "protocol": protocol})
            }
            Self::Loading { model } => json!({"type": "loading", "model": model}),
            Self::Loaded { model, ms } => json!({"type": "loaded", "model": model, "ms": ms}),
            Self::Unloaded { reason } => json!({"type": "unloaded", "reason": reason.as_str()}),
            Self::Voices { voices } => json!({
                "type": "voices",
                "voices": voices.iter().map(VoiceInfo::to_json).collect::<Vec<_>>(),
            }),
            Self::Audio {
                job,
                sample_rate,
                samples,
            } => json!({
                "type": "audio", "job": job, "sample_rate": sample_rate,
                "samples": samples.len(),
            }),
            Self::Done { job, stats } => {
                let mut value = json!({"type": "done", "job": job});
                stats.put(&mut value);
                value
            }
            Self::Cancelled { job } => json!({"type": "cancelled", "job": job}),
            Self::Error { job, code, params } => {
                json!({"type": "error", "job": job, "code": code, "params": params})
            }
            Self::Pong => json!({"type": "pong"}),
        }
    }

    /// Reads every event but `audio`, which needs the binary frame after it.
    fn from_json(fields: &Fields<'_>, kind: &str) -> Result<Self, DecodeError> {
        match kind {
            "ready" => Ok(Self::Ready {
                version: fields.string("version")?,
                protocol: fields.u32("protocol")?,
            }),
            "loading" => Ok(Self::Loading {
                model: fields.string("model")?,
            }),
            "loaded" => Ok(Self::Loaded {
                model: fields.string("model")?,
                ms: fields.u64("ms")?,
            }),
            "unloaded" => Ok(Self::Unloaded {
                reason: UnloadReason::parse(&fields.string("reason")?)?,
            }),
            "voices" => Ok(Self::Voices {
                voices: fields
                    .array("voices")?
                    .iter()
                    .map(VoiceInfo::from_json)
                    .collect::<Result<_, _>>()?,
            }),
            "done" => Ok(Self::Done {
                job: fields.u64("job")?,
                stats: SpeakStats::read(fields)?,
            }),
            "cancelled" => Ok(Self::Cancelled {
                job: fields.u64("job")?,
            }),
            "error" => Ok(Self::Error {
                job: fields.optional_u64("job")?,
                code: fields.string("code")?,
                params: fields.object("params")?,
            }),
            "pong" => Ok(Self::Pong),
            other => Err(DecodeError::UnknownType(other.to_owned())),
        }
    }
}

/// Why [`read_event`] found no event.
#[derive(Debug)]
pub enum ReadError {
    Frame(FrameError),
    Decode(DecodeError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => error.fmt(f),
            Self::Decode(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ReadError {}

/// Writes one event: one JSON frame, and for `audio` the binary frame of its samples. An
/// `audio` event over [`MAX_AUDIO_SAMPLES`] is refused; the writer splits longer audio.
pub fn write_event<W: Write>(writer: &mut W, event: &TtsEvent) -> Result<(), FrameError> {
    if let TtsEvent::Audio { samples, .. } = event {
        if samples.len() > MAX_AUDIO_SAMPLES {
            return Err(FrameError::TooLarge((samples.len() * 4) as u64));
        }
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        write_json(writer, &event.to_json())?;
        return write_frame(writer, FrameKind::Binary, &bytes);
    }
    write_json(writer, &event.to_json())
}

/// Reads one event. `Ok(None)` means the stream ended cleanly between events.
pub fn read_event<R: Read>(reader: &mut R) -> Result<Option<TtsEvent>, ReadError> {
    let Some(frame) = read_frame(reader).map_err(ReadError::Frame)? else {
        return Ok(None);
    };
    let value = parse_payload(&frame).map_err(ReadError::Decode)?;
    let fields = Fields::of(&value).map_err(ReadError::Decode)?;
    let kind = fields.kind().map_err(ReadError::Decode)?;
    if kind != "audio" {
        return TtsEvent::from_json(&fields, &kind)
            .map(Some)
            .map_err(ReadError::Decode);
    }
    let (job, sample_rate, count) = audio_head(&fields).map_err(ReadError::Decode)?;
    let audio = match read_frame(reader).map_err(ReadError::Frame)? {
        Some(audio) if audio.kind == FrameKind::Binary => audio,
        Some(_) => {
            return Err(ReadError::Decode(DecodeError::Invalid(
                "an audio event must be followed by a binary frame".into(),
            )));
        }
        None => return Err(ReadError::Frame(FrameError::Truncated)),
    };
    if audio.payload.len() as u64 != count.saturating_mul(4) {
        return Err(ReadError::Decode(DecodeError::Invalid(format!(
            "an audio frame of {} bytes does not hold {count} samples",
            audio.payload.len()
        ))));
    }
    let samples = audio
        .payload
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect();
    Ok(Some(TtsEvent::Audio {
        job,
        sample_rate,
        samples,
    }))
}

/// The job, the rate, and the sample count of an `audio` event.
fn audio_head(fields: &Fields<'_>) -> Result<(u64, u32, u64), DecodeError> {
    Ok((
        fields.u64("job")?,
        fields.u32("sample_rate")?,
        fields.u64("samples")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn through_the_pipe(events: &[TtsEvent]) -> Vec<TtsEvent> {
        let mut bytes = Vec::new();
        for event in events {
            write_event(&mut bytes, event).unwrap();
        }
        let mut reader = &bytes[..];
        let mut read = Vec::new();
        while let Some(event) = read_event(&mut reader).unwrap() {
            read.push(event);
        }
        read
    }

    #[test]
    fn requests_round_trip_through_json() {
        let requests = [
            TtsRequest::Hello { protocol: 1 },
            TtsRequest::LoadModel {
                dir: "/m/tts/kokoro-82m-int8".into(),
                threads: 4,
                idle_secs: 300,
            },
            TtsRequest::ListVoices,
            TtsRequest::Speak {
                job: 3,
                text: "Hello there.".into(),
                voice: Some("af_heart".into()),
                speed: 1.25,
            },
            TtsRequest::Speak {
                job: 4,
                text: "Hi.".into(),
                voice: None,
                speed: 1.0,
            },
            TtsRequest::Cancel { job: 3 },
            TtsRequest::Unload,
            TtsRequest::Ping,
            TtsRequest::Shutdown,
        ];
        for request in requests {
            assert_eq!(TtsRequest::from_json(&request.to_json()), Ok(request));
        }
    }

    #[test]
    fn a_speed_out_of_range_is_refused_and_the_idle_time_has_a_default() {
        for speed in [0.1, 2.5] {
            let value = json!({"type": "speak", "job": 1, "text": "a", "speed": speed});
            assert!(matches!(
                TtsRequest::from_json(&value),
                Err(DecodeError::Invalid(_))
            ));
        }
        let value = json!({"type": "load_model", "dir": "/m", "threads": 2});
        let Ok(TtsRequest::LoadModel { idle_secs, .. }) = TtsRequest::from_json(&value) else {
            panic!("a load request");
        };
        assert_eq!(idle_secs, DEFAULT_IDLE_SECS);
    }

    #[test]
    fn events_and_audio_round_trip_through_the_frame_codec() {
        let events = vec![
            TtsEvent::Ready {
                version: "2.0.0".into(),
                protocol: 1,
            },
            TtsEvent::Loading {
                model: "kokoro-82m-int8".into(),
            },
            TtsEvent::Loaded {
                model: "kokoro-82m-int8".into(),
                ms: 700,
            },
            TtsEvent::Voices {
                voices: vec![VoiceInfo {
                    id: "af_heart".into(),
                    name: "Heart".into(),
                    language: "en-US".into(),
                }],
            },
            TtsEvent::Audio {
                job: 3,
                sample_rate: 24_000,
                samples: vec![0.0, -0.5, 0.25, 1.0],
            },
            TtsEvent::Audio {
                job: 3,
                sample_rate: 24_000,
                samples: Vec::new(),
            },
            TtsEvent::Done {
                job: 3,
                stats: SpeakStats {
                    audio_ms: 2400,
                    ms: 600,
                    load_ms: 0,
                    first_audio_ms: 250,
                },
            },
            TtsEvent::Cancelled { job: 4 },
            TtsEvent::Unloaded {
                reason: UnloadReason::Idle,
            },
            TtsEvent::error(Some(5), "SPEECH_VOICE_MISSING", "zz_none"),
            TtsEvent::error(None, "TTS_LOAD_FAILED", "no file"),
            TtsEvent::Pong,
        ];
        assert_eq!(through_the_pipe(&events), events);
    }

    #[test]
    fn audio_over_the_frame_limit_is_refused_by_the_writer() {
        let event = TtsEvent::Audio {
            job: 1,
            sample_rate: 24_000,
            samples: vec![0.0; MAX_AUDIO_SAMPLES + 1],
        };
        assert!(matches!(
            write_event(&mut Vec::new(), &event),
            Err(FrameError::TooLarge(_))
        ));
        let largest = TtsEvent::Audio {
            job: 1,
            sample_rate: 24_000,
            samples: vec![0.5; MAX_AUDIO_SAMPLES],
        };
        assert_eq!(through_the_pipe(std::slice::from_ref(&largest)), [largest]);
    }

    #[test]
    fn an_audio_event_without_its_samples_is_an_error() {
        let mut bytes = Vec::new();
        write_json(
            &mut bytes,
            &json!({"type": "audio", "job": 1, "sample_rate": 24000, "samples": 2}),
        )
        .unwrap();
        assert!(matches!(
            read_event(&mut &bytes[..]),
            Err(ReadError::Frame(FrameError::Truncated))
        ));
        write_json(&mut bytes, &json!({"type": "pong"})).unwrap();
        assert!(matches!(
            read_event(&mut &bytes[..]),
            Err(ReadError::Decode(_))
        ));
        let mut short = Vec::new();
        write_json(
            &mut short,
            &json!({"type": "audio", "job": 1, "sample_rate": 24000, "samples": 2}),
        )
        .unwrap();
        write_frame(&mut short, FrameKind::Binary, &[0, 0, 0, 0]).unwrap();
        assert!(matches!(
            read_event(&mut &short[..]),
            Err(ReadError::Decode(_))
        ));
    }

    #[test]
    fn an_unknown_event_type_is_told_apart_from_a_malformed_one() {
        let mut bytes = Vec::new();
        write_json(&mut bytes, &json!({"type": "dance"})).unwrap();
        assert!(matches!(
            read_event(&mut &bytes[..]),
            Err(ReadError::Decode(DecodeError::UnknownType(kind))) if kind == "dance"
        ));
        assert_eq!(
            TtsRequest::from_json(&json!({"type": "dance"})),
            Err(DecodeError::UnknownType("dance".into()))
        );
        assert!(matches!(
            TtsRequest::from_json(&json!({"type": "speak", "job": 1})),
            Err(DecodeError::Invalid(_))
        ));
    }
}
