//! Paired Noise connections carry the same frames as local worker pipes.

pub mod discovery;
mod pairing;
mod stream;

pub use pairing::PairOffer;
pub use stream::{
    Identity, RemoteControl, RemoteReader, RemoteStream, RemoteWriter, Resolver, Session,
};

use hushpen_core::protocol::{self, FrameKind};
use hushpen_core::remote::UnloadRule;
use serde_json::json;
use std::io::{self, Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerKind {
    Speech,
    Cleanup,
}

/// Selects a worker before its unchanged Hello and Ready frames.
pub fn select_worker(writer: &mut impl Write, kind: WorkerKind) -> io::Result<()> {
    select_worker_with_rule(writer, kind, None)
}

pub fn select_worker_with_rule(
    writer: &mut impl Write,
    kind: WorkerKind,
    rule: Option<UnloadRule>,
) -> io::Result<()> {
    let mut value = json!({"type": "remote_worker", "kind": match kind {
        WorkerKind::Speech => "speech",
        WorkerKind::Cleanup => "cleanup",
    }});
    if let Some(rule) = rule {
        value["unload"] = match rule {
            UnloadRule::KeepLoaded => json!({"kind":"keepLoaded"}),
            UnloadRule::AfterIdle(minutes) if (1..=1440).contains(&minutes) => {
                json!({"kind":"afterIdle","minutes":minutes})
            }
            UnloadRule::AfterIdle(_) => return Err(invalid("invalid idle minutes")),
            UnloadRule::WhenMemoryLow => json!({"kind":"whenMemoryLow"}),
            UnloadRule::WhileServerModeOpen => json!({"kind":"whileServerModeOpen"}),
        };
    }
    protocol::write_json(writer, &value).map_err(io::Error::other)
}

pub fn read_worker(reader: &mut impl Read) -> io::Result<WorkerKind> {
    read_worker_options(reader).map(|(kind, _)| kind)
}

pub fn read_worker_options(reader: &mut impl Read) -> io::Result<(WorkerKind, Option<UnloadRule>)> {
    let frame = protocol::read_frame(reader)
        .map_err(io::Error::other)?
        .ok_or_else(|| invalid("missing worker choice"))?;
    if frame.kind != FrameKind::Json || frame.payload.len() > 256 {
        return Err(invalid("invalid worker choice"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&frame.payload).map_err(|_| invalid("invalid worker choice"))?;
    if value["type"] != "remote_worker" {
        return Err(invalid("invalid worker choice"));
    }
    let kind = match value["kind"].as_str() {
        Some("speech") => WorkerKind::Speech,
        Some("cleanup") => WorkerKind::Cleanup,
        _ => return Err(invalid("unknown worker kind")),
    };
    let rule = value
        .get("unload")
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| invalid("invalid unload rule"))?;
            let idle = value["kind"] == "afterIdle";
            if object.len() != if idle { 2 } else { 1 } {
                return Err(invalid("invalid unload rule"));
            }
            match value["kind"].as_str() {
                Some("keepLoaded") => Ok(UnloadRule::KeepLoaded),
                Some("afterIdle") => value["minutes"]
                    .as_u64()
                    .filter(|minutes| (1..=1440).contains(minutes))
                    .map(|minutes| UnloadRule::AfterIdle(minutes as u32))
                    .ok_or_else(|| invalid("invalid idle minutes")),
                Some("whenMemoryLow") => Ok(UnloadRule::WhenMemoryLow),
                Some("whileServerModeOpen") => Ok(UnloadRule::WhileServerModeOpen),
                _ => Err(invalid("unknown unload rule")),
            }
        })
        .transpose()?;
    Ok((kind, rule))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "device authentication failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_selection_preserves_optional_unload_rules() {
        for rule in [
            None,
            Some(UnloadRule::KeepLoaded),
            Some(UnloadRule::AfterIdle(30)),
            Some(UnloadRule::WhenMemoryLow),
            Some(UnloadRule::WhileServerModeOpen),
        ] {
            let mut bytes = Vec::new();
            select_worker_with_rule(&mut bytes, WorkerKind::Speech, rule).unwrap();
            assert_eq!(
                read_worker_options(&mut bytes.as_slice()).unwrap(),
                (WorkerKind::Speech, rule)
            );
        }
        for unload in [
            json!(null),
            json!({"kind":"unknown"}),
            json!({"kind":"afterIdle","minutes":0}),
            json!({"kind":"afterIdle","minutes":1441}),
            json!({"kind":"afterIdle","minutes":"30"}),
            json!({"kind":"keepLoaded","minutes":1}),
        ] {
            let mut bytes = Vec::new();
            protocol::write_json(
                &mut bytes,
                &json!({"type":"remote_worker","kind":"speech","unload":unload}),
            )
            .unwrap();
            assert!(read_worker_options(&mut bytes.as_slice()).is_err());
        }
    }
}
