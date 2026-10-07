//! The pipe protocol between the app and the engine child.
//!
//! A frame is a `u32` little-endian payload length, a `u8` kind (`0` JSON control, `1` binary),
//! then the payload. A payload over [`MAX_FRAME`] is refused before any byte of it is read.
//! Audio never crosses the pipe; the app sends file paths inside the data folder.
//!
//! Control messages are JSON objects with a `type` field. Errors carry a code from
//! [`crate::error`] and a `params` object; the UI never matches English text.

use serde_json::{Map, Value, json};
use std::fmt;
use std::io::{self, Read, Write};

/// Bumped when a message changes shape. `Hello` carries it and `Ready` answers with it.
pub const PROTOCOL_VERSION: u32 = 1;

/// Largest payload, in bytes.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Json = 0,
    Binary = 1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameKind,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    /// The announced length is over [`MAX_FRAME`].
    TooLarge(u64),
    UnknownKind(u8),
    /// The stream ended inside a frame.
    Truncated,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "pipe error: {error}"),
            Self::TooLarge(length) => write!(f, "frame of {length} bytes is over the limit"),
            Self::UnknownKind(kind) => write!(f, "unknown frame kind {kind}"),
            Self::Truncated => f.write_str("the stream ended inside a frame"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Writes one frame as a single `write_all`, then flushes.
pub fn write_frame<W: Write>(
    writer: &mut W,
    kind: FrameKind,
    payload: &[u8],
) -> Result<(), FrameError> {
    if payload.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(payload.len() as u64));
    }
    let mut bytes = Vec::with_capacity(payload.len() + 5);
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.push(kind as u8);
    bytes.extend_from_slice(payload);
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

/// Writes a JSON control frame.
pub fn write_json<W: Write>(writer: &mut W, message: &Value) -> Result<(), FrameError> {
    write_frame(writer, FrameKind::Json, message.to_string().as_bytes())
}

/// Reads one frame. `Ok(None)` means the stream ended cleanly between frames.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Frame>, FrameError> {
    let mut header = [0_u8; 5];
    let mut filled = 0;
    while filled < header.len() {
        match reader.read(&mut header[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(FrameError::Truncated),
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    let length = u64::from(u32::from_le_bytes([
        header[0], header[1], header[2], header[3],
    ]));
    if length > MAX_FRAME as u64 {
        return Err(FrameError::TooLarge(length));
    }
    let kind = match header[4] {
        0 => FrameKind::Json,
        1 => FrameKind::Binary,
        other => return Err(FrameError::UnknownKind(other)),
    };
    let mut payload = vec![0_u8; length as usize];
    reader.read_exact(&mut payload).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            FrameError::Truncated
        } else {
            error.into()
        }
    })?;
    Ok(Some(Frame { kind, payload }))
}

/// Why a JSON payload is not a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Valid JSON with a `type` this side does not know.
    UnknownType(String),
    /// Not JSON, not an object, or a field is missing or has the wrong type.
    Invalid(String),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(kind) => write!(f, "unknown message type '{kind}'"),
            Self::Invalid(detail) => write!(f, "invalid message: {detail}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// One piece of recognized speech with its time span in the audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

impl Segment {
    fn to_json(&self) -> Value {
        json!({"start_ms": self.start_ms, "end_ms": self.end_ms, "text": self.text})
    }

    fn from_json(value: &Value) -> Result<Self, DecodeError> {
        let fields = Fields::of(value)?;
        Ok(Self {
            start_ms: fields.u64("start_ms")?,
            end_ms: fields.u64("end_ms")?,
            text: fields.string("text")?,
        })
    }
}

/// App to engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Hello {
        protocol: u32,
    },
    LoadModel {
        path: String,
        gpu: bool,
        threads: u32,
    },
    Transcribe {
        job: u64,
        wav_path: String,
        /// `None` or `auto` detects the language.
        language: Option<String>,
        prompt: Option<String>,
    },
    Cancel {
        job: u64,
    },
    Ping,
    Shutdown,
}

impl Request {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Hello { protocol } => json!({"type": "hello", "protocol": protocol}),
            Self::LoadModel { path, gpu, threads } => {
                json!({"type": "load_model", "path": path, "gpu": gpu, "threads": threads})
            }
            Self::Transcribe {
                job,
                wav_path,
                language,
                prompt,
            } => json!({
                "type": "transcribe", "job": job, "wav_path": wav_path,
                "language": language, "prompt": prompt,
            }),
            Self::Cancel { job } => json!({"type": "cancel", "job": job}),
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
                path: fields.string("path")?,
                gpu: fields.bool("gpu")?,
                threads: fields.u32("threads")?,
            }),
            "transcribe" => Ok(Self::Transcribe {
                job: fields.u64("job")?,
                wav_path: fields.string("wav_path")?,
                language: fields.optional_string("language")?,
                prompt: fields.optional_string("prompt")?,
            }),
            "cancel" => Ok(Self::Cancel {
                job: fields.u64("job")?,
            }),
            "ping" => Ok(Self::Ping),
            "shutdown" => Ok(Self::Shutdown),
            other => Err(DecodeError::UnknownType(other.to_owned())),
        }
    }

    pub fn from_frame(frame: &Frame) -> Result<Self, DecodeError> {
        Self::from_json(&parse_payload(frame)?)
    }
}

/// Engine to app.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Ready {
        version: String,
        protocol: u32,
        /// True when this engine can use the GPU (a GPU backend is built in and not disabled).
        gpu: bool,
    },
    /// A model load started.
    Loading {
        model: String,
    },
    Loaded {
        model: String,
        ms: u64,
        /// True when the model runs on the GPU.
        gpu: bool,
    },
    Segment {
        job: u64,
        segment: Segment,
    },
    Result {
        job: u64,
        text: String,
        language: String,
        segments: Vec<Segment>,
        audio_ms: u64,
        /// Time spent recognizing, without reading the file.
        ms: u64,
    },
    Cancelled {
        job: u64,
    },
    Error {
        /// `None` for an error that belongs to no job, such as a failed model load.
        job: Option<u64>,
        code: String,
        params: Map<String, Value>,
    },
    Pong,
}

/// The `reason` of an `ENGINE_LOAD_FAILED` error when the CPU lacks an instruction set.
pub const REASON_CPU: &str = "cpu";

/// The `params` object of an error event.
pub type Params = Map<String, Value>;

/// The `detail` and `reason` texts of an error event's `params`.
pub fn error_texts(params: &Params) -> (String, Option<String>) {
    let text = |key: &str| params.get(key).and_then(Value::as_str).map(str::to_owned);
    (text("detail").unwrap_or_default(), text("reason"))
}

impl Event {
    pub fn error(job: Option<u64>, code: &str, detail: impl Into<String>) -> Self {
        let mut params = Map::new();
        params.insert("detail".into(), Value::String(detail.into()));
        Self::Error {
            job,
            code: code.to_owned(),
            params,
        }
    }

    /// An error with a `reason` parameter next to the `detail`.
    pub fn error_with_reason(
        job: Option<u64>,
        code: &str,
        detail: impl Into<String>,
        reason: &str,
    ) -> Self {
        let mut event = Self::error(job, code, detail);
        if let Self::Error { params, .. } = &mut event {
            params.insert("reason".into(), Value::String(reason.to_owned()));
        }
        event
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Ready {
                version,
                protocol,
                gpu,
            } => json!({"type": "ready", "version": version, "protocol": protocol, "gpu": gpu}),
            Self::Loading { model } => json!({"type": "loading", "model": model}),
            Self::Loaded { model, ms, gpu } => {
                json!({"type": "loaded", "model": model, "ms": ms, "gpu": gpu})
            }
            Self::Segment { job, segment } => {
                let mut value = segment.to_json();
                value["type"] = json!("segment");
                value["job"] = json!(job);
                value
            }
            Self::Result {
                job,
                text,
                language,
                segments,
                audio_ms,
                ms,
            } => json!({
                "type": "result", "job": job, "text": text, "language": language,
                "segments": segments.iter().map(Segment::to_json).collect::<Vec<_>>(),
                "audio_ms": audio_ms, "ms": ms,
            }),
            Self::Cancelled { job } => json!({"type": "cancelled", "job": job}),
            Self::Error { job, code, params } => {
                json!({"type": "error", "job": job, "code": code, "params": params})
            }
            Self::Pong => json!({"type": "pong"}),
        }
    }

    pub fn from_json(value: &Value) -> Result<Self, DecodeError> {
        let fields = Fields::of(value)?;
        match fields.kind()?.as_str() {
            "ready" => Ok(Self::Ready {
                version: fields.string("version")?,
                protocol: fields.u32("protocol")?,
                gpu: fields.bool("gpu")?,
            }),
            "loading" => Ok(Self::Loading {
                model: fields.string("model")?,
            }),
            "loaded" => Ok(Self::Loaded {
                model: fields.string("model")?,
                ms: fields.u64("ms")?,
                gpu: fields.bool("gpu")?,
            }),
            "segment" => Ok(Self::Segment {
                job: fields.u64("job")?,
                segment: Segment::from_json(value)?,
            }),
            "result" => Ok(Self::Result {
                job: fields.u64("job")?,
                text: fields.string("text")?,
                language: fields.string("language")?,
                segments: fields
                    .array("segments")?
                    .iter()
                    .map(Segment::from_json)
                    .collect::<Result<_, _>>()?,
                audio_ms: fields.u64("audio_ms")?,
                ms: fields.u64("ms")?,
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

    pub fn from_frame(frame: &Frame) -> Result<Self, DecodeError> {
        Self::from_json(&parse_payload(frame)?)
    }
}

fn parse_payload(frame: &Frame) -> Result<Value, DecodeError> {
    if frame.kind != FrameKind::Json {
        return Err(DecodeError::Invalid("not a JSON frame".into()));
    }
    serde_json::from_slice(&frame.payload).map_err(|error| DecodeError::Invalid(error.to_string()))
}

/// Typed access to the fields of a JSON object.
struct Fields<'a>(&'a Map<String, Value>);

impl<'a> Fields<'a> {
    fn of(value: &'a Value) -> Result<Self, DecodeError> {
        value
            .as_object()
            .map(Self)
            .ok_or_else(|| DecodeError::Invalid("not an object".into()))
    }

    fn kind(&self) -> Result<String, DecodeError> {
        self.string("type")
    }

    fn get(&self, name: &str) -> Result<&'a Value, DecodeError> {
        self.0
            .get(name)
            .ok_or_else(|| DecodeError::Invalid(format!("missing field '{name}'")))
    }

    fn wrong(name: &str, expected: &str) -> DecodeError {
        DecodeError::Invalid(format!("field '{name}' is not {expected}"))
    }

    fn string(&self, name: &str) -> Result<String, DecodeError> {
        self.get(name)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Self::wrong(name, "a string"))
    }

    fn optional_string(&self, name: &str) -> Result<Option<String>, DecodeError> {
        match self.0.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(_) => Err(Self::wrong(name, "a string")),
        }
    }

    fn u64(&self, name: &str) -> Result<u64, DecodeError> {
        self.get(name)?
            .as_u64()
            .ok_or_else(|| Self::wrong(name, "a whole number"))
    }

    fn optional_u64(&self, name: &str) -> Result<Option<u64>, DecodeError> {
        match self.0.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => self.u64(name).map(Some),
        }
    }

    fn u32(&self, name: &str) -> Result<u32, DecodeError> {
        u32::try_from(self.u64(name)?).map_err(|_| Self::wrong(name, "a 32-bit number"))
    }

    fn bool(&self, name: &str) -> Result<bool, DecodeError> {
        self.get(name)?
            .as_bool()
            .ok_or_else(|| Self::wrong(name, "true or false"))
    }

    fn array(&self, name: &str) -> Result<&'a Vec<Value>, DecodeError> {
        self.get(name)?
            .as_array()
            .ok_or_else(|| Self::wrong(name, "a list"))
    }

    fn object(&self, name: &str) -> Result<Map<String, Value>, DecodeError> {
        match self.0.get(name) {
            None | Some(Value::Null) => Ok(Map::new()),
            Some(Value::Object(map)) => Ok(map.clone()),
            Some(_) => Err(Self::wrong(name, "an object")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(frame: &[u8]) -> Result<Option<Frame>, FrameError> {
        read_frame(&mut &frame[..])
    }

    #[test]
    fn a_frame_round_trips() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, FrameKind::Json, b"{\"a\":1}").unwrap();
        assert_eq!(&bytes[..5], &[7, 0, 0, 0, 0]);
        let frame = round_trip(&bytes).unwrap().unwrap();
        assert_eq!(frame.kind, FrameKind::Json);
        assert_eq!(frame.payload, b"{\"a\":1}");

        let mut bytes = Vec::new();
        write_frame(&mut bytes, FrameKind::Binary, &[9, 8, 7]).unwrap();
        let frame = round_trip(&bytes).unwrap().unwrap();
        assert_eq!(frame.kind, FrameKind::Binary);
        assert_eq!(frame.payload, [9, 8, 7]);
    }

    #[test]
    fn several_frames_are_read_in_order_then_a_clean_end() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, FrameKind::Json, b"1").unwrap();
        write_frame(&mut bytes, FrameKind::Json, b"22").unwrap();
        let mut reader = &bytes[..];
        assert_eq!(read_frame(&mut reader).unwrap().unwrap().payload, b"1");
        assert_eq!(read_frame(&mut reader).unwrap().unwrap().payload, b"22");
        assert!(read_frame(&mut reader).unwrap().is_none());
    }

    #[test]
    fn an_empty_payload_is_a_frame() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, FrameKind::Binary, &[]).unwrap();
        assert!(round_trip(&bytes).unwrap().unwrap().payload.is_empty());
    }

    #[test]
    fn a_stream_cut_inside_a_frame_is_truncated() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, FrameKind::Json, b"hello").unwrap();
        for cut in [1, 4, 5, 7] {
            assert!(
                matches!(round_trip(&bytes[..cut]), Err(FrameError::Truncated)),
                "cut at {cut}"
            );
        }
    }

    #[test]
    fn a_frame_over_the_limit_is_refused_before_reading_it() {
        let mut header = ((MAX_FRAME as u32) + 1).to_le_bytes().to_vec();
        header.push(0);
        assert!(matches!(
            round_trip(&header),
            Err(FrameError::TooLarge(length)) if length == MAX_FRAME as u64 + 1
        ));
        let mut huge = u32::MAX.to_le_bytes().to_vec();
        huge.push(0);
        assert!(matches!(round_trip(&huge), Err(FrameError::TooLarge(_))));
    }

    #[test]
    fn the_largest_allowed_frame_is_accepted_by_the_length_check() {
        let mut header = (MAX_FRAME as u32).to_le_bytes().to_vec();
        header.push(1);
        assert!(matches!(round_trip(&header), Err(FrameError::Truncated)));
    }

    #[test]
    fn an_unknown_kind_is_refused() {
        let bytes = [0, 0, 0, 0, 9];
        assert!(matches!(
            round_trip(&bytes),
            Err(FrameError::UnknownKind(9))
        ));
    }

    #[test]
    fn requests_round_trip_through_json() {
        let requests = [
            Request::Hello { protocol: 1 },
            Request::LoadModel {
                path: "/m/ggml-tiny.en.bin".into(),
                gpu: true,
                threads: 4,
            },
            Request::Transcribe {
                job: 7,
                wav_path: "/a.wav".into(),
                language: Some("en".into()),
                prompt: None,
            },
            Request::Cancel { job: 7 },
            Request::Ping,
            Request::Shutdown,
        ];
        for request in requests {
            assert_eq!(Request::from_json(&request.to_json()), Ok(request));
        }
    }

    #[test]
    fn events_round_trip_through_json() {
        let segment = Segment {
            start_ms: 10,
            end_ms: 900,
            text: " hi".into(),
        };
        let events = [
            Event::Ready {
                version: "2.0.0".into(),
                protocol: 1,
                gpu: false,
            },
            Event::Loading {
                model: "tiny.en".into(),
            },
            Event::Loaded {
                model: "tiny.en".into(),
                ms: 120,
                gpu: true,
            },
            Event::Segment {
                job: 3,
                segment: segment.clone(),
            },
            Event::Result {
                job: 3,
                text: "hi".into(),
                language: "en".into(),
                segments: vec![segment],
                audio_ms: 1000,
                ms: 300,
            },
            Event::Cancelled { job: 3 },
            Event::error(Some(3), "ENGINE_BAD_LANGUAGE", "xx"),
            Event::error(None, "ENGINE_LOAD_FAILED", "bad file"),
            Event::Pong,
        ];
        for event in events {
            assert_eq!(Event::from_json(&event.to_json()), Ok(event));
        }
    }

    #[test]
    fn an_error_can_carry_a_reason_that_survives_the_wire() {
        let sent = Event::error_with_reason(None, "ENGINE_LOAD_FAILED", "no AVX2", REASON_CPU);
        let frame = Frame {
            kind: FrameKind::Json,
            payload: serde_json::to_vec(&sent.to_json()).unwrap(),
        };
        let Event::Error { params, .. } = Event::from_frame(&frame).unwrap() else {
            panic!("an error event");
        };
        assert_eq!(
            error_texts(&params),
            ("no AVX2".to_owned(), Some("cpu".to_owned()))
        );
        let plain = Event::error(None, "ENGINE_LOAD_FAILED", "bad file");
        let Event::Error { params, .. } = plain else {
            panic!("an error event");
        };
        assert_eq!(error_texts(&params), ("bad file".to_owned(), None));
    }

    #[test]
    fn an_unknown_type_is_told_apart_from_a_malformed_message() {
        assert_eq!(
            Request::from_json(&json!({"type": "dance"})),
            Err(DecodeError::UnknownType("dance".into()))
        );
        assert!(matches!(
            Request::from_json(&json!({"type": "cancel"})),
            Err(DecodeError::Invalid(_))
        ));
        assert!(matches!(
            Request::from_json(&json!({"type": "cancel", "job": "x"})),
            Err(DecodeError::Invalid(_))
        ));
        assert!(matches!(
            Request::from_json(&json!([1])),
            Err(DecodeError::Invalid(_))
        ));
        assert!(matches!(
            Request::from_json(&json!({"job": 1})),
            Err(DecodeError::Invalid(_))
        ));
    }

    #[test]
    fn a_transcribe_request_may_leave_language_and_prompt_out() {
        let request =
            Request::from_json(&json!({"type": "transcribe", "job": 1, "wav_path": "/a"}));
        assert_eq!(
            request,
            Ok(Request::Transcribe {
                job: 1,
                wav_path: "/a".into(),
                language: None,
                prompt: None,
            })
        );
    }

    #[test]
    fn a_frame_that_is_not_json_is_invalid() {
        let frame = Frame {
            kind: FrameKind::Json,
            payload: b"{nope".to_vec(),
        };
        assert!(matches!(
            Request::from_frame(&frame),
            Err(DecodeError::Invalid(_))
        ));
        let binary = Frame {
            kind: FrameKind::Binary,
            payload: Vec::new(),
        };
        assert!(matches!(
            Event::from_frame(&binary),
            Err(DecodeError::Invalid(_))
        ));
    }
}
