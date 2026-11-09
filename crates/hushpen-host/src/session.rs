use crate::{Workers, idle, lock};
use hushpen_core::catalog::{self, ModelKind};
use hushpen_core::error;
use hushpen_core::llm_protocol::{LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest};
use hushpen_core::protocol::{Event, FrameKind, PROTOCOL_VERSION, Request, read_frame, write_json};
use hushpen_core::remote::UnloadRule;
use hushpen_engine::{
    ImportSpec, JobOutcome, LlmOutcome, LlmSpec, LlmWork, LoadSpec, TranscribeSpec,
};
use hushpen_remote::{RemoteStream, RemoteWriter, WorkerKind, read_worker_options};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

type Output = Arc<Mutex<RemoteWriter>>;

/// Serves one authenticated connection. Model files stay under the host's data root.
pub fn serve_session(
    mut stream: RemoteStream,
    workers: Arc<Workers>,
    data: &Path,
    rule: UnloadRule,
) -> io::Result<()> {
    let (kind, requested_rule) = read_worker_options(&mut stream)?;
    let rule = requested_rule.unwrap_or(rule);
    if !rule.allowed_on(workers.os) {
        return Err(invalid("unload rule is unavailable on this host"));
    }
    let gate = match kind {
        WorkerKind::Speech => &workers.speech_session,
        WorkerKind::Cleanup => &workers.cleanup_session,
    };
    let _guard = gate.try_lock().map_err(|_| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "this worker already serves a client",
        )
    })?;
    *lock(match kind {
        WorkerKind::Speech => &workers.speech_rule,
        WorkerKind::Cleanup => &workers.cleanup_rule,
    }) = rule;
    let (mut input, output) = stream.split()?;
    let output = Arc::new(Mutex::new(output));
    let jobs = Arc::new(Mutex::new(HashMap::<u64, u64>::new()));
    let cache = data.join("cache/remote");
    std::fs::create_dir_all(&cache)?;
    let mut audio = None;
    let result = (|| {
        while let Some(frame) = read_frame(&mut input).map_err(io::Error::other)? {
            if frame.kind != FrameKind::Json {
                return Err(invalid("expected a worker control frame"));
            }
            if kind == WorkerKind::Speech {
                let value: Value =
                    serde_json::from_slice(&frame.payload).map_err(io::Error::other)?;
                if value.get("type").and_then(Value::as_str) == Some("remote_audio") {
                    if audio.is_some() || !lock(&jobs).is_empty() {
                        return Err(invalid("audio is already pending"));
                    }
                    audio = Some(receive_audio(&mut input, &cache, &value)?);
                    continue;
                }
                let request = Request::from_frame(&frame).map_err(io::Error::other)?;
                if matches!(request, Request::Shutdown) {
                    break;
                }
                speech(request, &workers, &output, &jobs, data, rule, &mut audio)?;
            } else {
                let request = LlmRequest::from_frame(&frame).map_err(io::Error::other)?;
                if matches!(request, LlmRequest::Shutdown) {
                    break;
                }
                cleanup(request, &workers, &output, &jobs, data, rule)?;
            }
        }
        Ok(())
    })();
    for job in lock(&jobs).values() {
        match kind {
            WorkerKind::Speech => workers.speech.cancel(*job),
            WorkerKind::Cleanup => workers.cleanup.cancel(*job),
        }
    }
    result
}

fn send(output: &Output, value: &Value) -> io::Result<()> {
    write_json(&mut *lock(output), value).map_err(io::Error::other)
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn receive_audio(input: &mut impl Read, cache: &Path, value: &Value) -> io::Result<NamedTempFile> {
    receive_audio_until(
        input,
        cache,
        value,
        Instant::now() + Duration::from_secs(60),
    )
}

fn receive_audio_until(
    input: &mut impl Read,
    cache: &Path,
    value: &Value,
    deadline: Instant,
) -> io::Result<NamedTempFile> {
    let mut left = value
        .get("bytes")
        .and_then(Value::as_u64)
        .filter(|bytes| *bytes > 0 && *bytes <= 256 * 1024 * 1024)
        .ok_or_else(|| invalid("invalid remote audio length"))?;
    let mut file = tempfile::Builder::new().suffix(".wav").tempfile_in(cache)?;
    while left > 0 {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "audio transfer timed out",
            ));
        }
        let frame = read_frame(input)
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("incomplete remote audio"))?;
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "audio transfer timed out",
            ));
        }
        if frame.kind != FrameKind::Binary
            || frame.payload.is_empty()
            || frame.payload.len() > 64 * 1024
            || frame.payload.len() as u64 > left
        {
            return Err(invalid("invalid remote audio frame"));
        }
        file.write_all(&frame.payload)?;
        left -= frame.payload.len() as u64;
    }
    file.flush()?;
    Ok(file)
}

fn worker_threads(threads: u32) -> io::Result<usize> {
    if !(1..=64).contains(&threads) {
        return Err(invalid("worker threads must be 1 to 64"));
    }
    Ok(threads as usize)
}

fn model_path(data: &Path, incoming: &str, kind: ModelKind) -> io::Result<PathBuf> {
    let filename = incoming
        .rsplit(['/', '\\'])
        .next()
        .ok_or_else(|| invalid("invalid model file"))?;
    let catalog = catalog::embedded();
    let entries = match kind {
        ModelKind::Speech => &catalog.whisper,
        ModelKind::Cleanup => &catalog.llm,
        ModelKind::Voice => return Err(invalid("voice is not served")),
    };
    let pin = entries
        .iter()
        .flat_map(|entry| &entry.files)
        .find(|file| file.file == filename)
        .ok_or_else(|| invalid("model is not in the host catalog"))?;
    let root = data
        .join(if kind == ModelKind::Speech {
            "models/whisper"
        } else {
            "models/llm"
        })
        .canonicalize()?;
    let path = root.join(filename).canonicalize()?;
    if !path.starts_with(&root) || !path.is_file() {
        return Err(invalid("model is outside the host model folder"));
    }
    if !hushpen_store::model_files::is_verified(&path)
        || hushpen_store::model_files::check(&path, pin.bytes, &pin.sha256)
            != hushpen_store::model_files::Check::Ready
    {
        return Err(invalid("model has not passed verification"));
    }
    Ok(path)
}

fn speech(
    request: Request,
    workers: &Arc<Workers>,
    output: &Output,
    jobs: &Arc<Mutex<HashMap<u64, u64>>>,
    data: &Path,
    rule: UnloadRule,
    audio: &mut Option<NamedTempFile>,
) -> io::Result<()> {
    match request {
        Request::Hello { protocol } => {
            if protocol != PROTOCOL_VERSION {
                return Err(invalid("speech protocol does not match"));
            }
            send(
                output,
                &Event::Ready {
                    version: env!("CARGO_PKG_VERSION").into(),
                    protocol: PROTOCOL_VERSION,
                    gpu: false,
                }
                .to_json(),
            )
        }
        Request::Ping => send(output, &Event::Pong.to_json()),
        Request::Unload => {
            workers.speech.unload();
            Ok(())
        }
        Request::LoadModel {
            path, gpu, threads, ..
        } => {
            let threads = match worker_threads(threads) {
                Ok(threads) => threads,
                Err(problem) => {
                    return send(
                        output,
                        &Event::error(None, error::ENGINE_LOAD_FAILED, problem.to_string())
                            .to_json(),
                    );
                }
            };
            let path = match model_path(data, &path, ModelKind::Speech) {
                Ok(path) => path,
                Err(problem) => {
                    return send(
                        output,
                        &Event::error(None, error::ENGINE_LOAD_FAILED, problem.to_string())
                            .to_json(),
                    );
                }
            };
            if workers.speech_busy.swap(true, Ordering::AcqRel) {
                return Err(invalid("speech worker is busy"));
            }
            let load = workers.speech.load_model(LoadSpec {
                path,
                gpu,
                threads,
                idle: idle(rule),
            });
            let output = Arc::clone(output);
            let workers = Arc::clone(workers);
            std::thread::spawn(move || {
                let event = match load.wait_timeout(Duration::from_secs(120)) {
                    Some(Ok(loaded)) => Event::Loaded {
                        model: loaded.model,
                        ms: loaded.ms,
                        gpu: loaded.gpu,
                    },
                    Some(Err(failure)) => {
                        Event::error(None, error::ENGINE_LOAD_FAILED, failure.detail)
                    }
                    None => Event::error(
                        None,
                        error::ENGINE_LOAD_FAILED,
                        "the host model load timed out",
                    ),
                };
                workers.speech_busy.store(false, Ordering::Release);
                let _ = send(&output, &event.to_json());
            });
            Ok(())
        }
        Request::Cancel { job } => {
            if let Some(job) = lock(jobs).get(&job) {
                workers.speech.cancel(*job);
            }
            Ok(())
        }
        Request::Transcribe {
            job,
            language,
            prompt,
            ..
        } => {
            let audio = audio
                .take()
                .ok_or_else(|| invalid("speech requires transferred audio"))?;
            if workers.speech_busy.swap(true, Ordering::AcqRel) {
                return Err(invalid("speech worker is busy"));
            }
            let handle = workers.speech.transcribe(TranscribeSpec {
                wav_path: audio.path().into(),
                language,
                prompt,
            });
            lock(jobs).insert(job, handle.id());
            let workers = Arc::clone(workers);
            let output = Arc::clone(output);
            let jobs = Arc::clone(jobs);
            std::thread::spawn(move || {
                finish_speech(
                    handle
                        .wait_timeout(Duration::from_secs(300))
                        .unwrap_or_else(|| {
                            handle.cancel();
                            JobOutcome::Cancelled
                        }),
                    job,
                    &output,
                );
                drop(audio);
                lock(&jobs).remove(&job);
                workers.speech_busy.store(false, Ordering::Release);
            });
            Ok(())
        }
        Request::ImportFile {
            job,
            language,
            prompt,
            translate,
            ..
        } => {
            let audio = audio
                .take()
                .ok_or_else(|| invalid("import requires transferred audio"))?;
            if workers.speech_busy.swap(true, Ordering::AcqRel) {
                return Err(invalid("speech worker is busy"));
            }
            let handle = workers.speech.import(
                ImportSpec {
                    path: audio.path().into(),
                    language,
                    prompt,
                    translate,
                },
                None,
            );
            lock(jobs).insert(job, handle.id());
            let workers = Arc::clone(workers);
            let output = Arc::clone(output);
            let jobs = Arc::clone(jobs);
            std::thread::spawn(move || {
                finish_speech(
                    handle
                        .wait_timeout(Duration::from_secs(300))
                        .unwrap_or_else(|| {
                            handle.cancel();
                            JobOutcome::Cancelled
                        }),
                    job,
                    &output,
                );
                drop(audio);
                lock(&jobs).remove(&job);
                workers.speech_busy.store(false, Ordering::Release);
            });
            Ok(())
        }
        Request::Shutdown => Ok(()),
    }
}

fn finish_speech(outcome: JobOutcome, job: u64, output: &Output) {
    let event = match outcome {
        JobOutcome::Done(done) => {
            for segment in &done.segments {
                let _ = send(
                    output,
                    &Event::Segment {
                        job,
                        segment: segment.clone(),
                    }
                    .to_json(),
                );
            }
            Event::Result {
                job,
                text: done.text,
                language: done.language,
                segments: done.segments,
                audio_ms: done.audio_ms,
                ms: done.decode_ms,
            }
        }
        JobOutcome::Cancelled => Event::Cancelled { job },
        JobOutcome::Failed(failure) => Event::error(Some(job), &failure.code, failure.detail),
    };
    let _ = send(output, &event.to_json());
}

fn cleanup(
    request: LlmRequest,
    workers: &Arc<Workers>,
    output: &Output,
    jobs: &Arc<Mutex<HashMap<u64, u64>>>,
    data: &Path,
    rule: UnloadRule,
) -> io::Result<()> {
    let (job, work) = match request {
        LlmRequest::Hello { protocol } => {
            if protocol != LLM_PROTOCOL_VERSION {
                return Err(invalid("cleanup protocol does not match"));
            }
            return send(
                output,
                &LlmEvent::Ready {
                    version: env!("CARGO_PKG_VERSION").into(),
                    protocol: LLM_PROTOCOL_VERSION,
                    backend: "host".into(),
                }
                .to_json(),
            );
        }
        LlmRequest::Ping => return send(output, &LlmEvent::Pong.to_json()),
        LlmRequest::Unload => {
            workers.cleanup.unload();
            return Ok(());
        }
        LlmRequest::Cancel { job } => {
            if let Some(job) = lock(jobs).get(&job) {
                workers.cleanup.cancel(*job);
            }
            return Ok(());
        }
        LlmRequest::LoadModel {
            path,
            family,
            gpu,
            threads,
            ..
        } => {
            let threads = match worker_threads(threads) {
                Ok(threads) => threads,
                Err(problem) => {
                    return send(
                        output,
                        &LlmEvent::error(None, error::LLM_LOAD_FAILED, problem.to_string())
                            .to_json(),
                    );
                }
            };
            let path = match if workers.system_cleanup {
                Ok(PathBuf::from("system"))
            } else {
                model_path(data, &path, ModelKind::Cleanup)
            } {
                Ok(path) => path,
                Err(problem) => {
                    return send(
                        output,
                        &LlmEvent::error(None, error::LLM_LOAD_FAILED, problem.to_string())
                            .to_json(),
                    );
                }
            };
            if workers.cleanup_busy.swap(true, Ordering::AcqRel) {
                return Err(invalid("cleanup worker is busy"));
            }
            let handle = workers.cleanup.load_model(LlmSpec {
                path,
                family,
                gpu,
                threads,
                idle: idle(rule),
            });
            let output = Arc::clone(output);
            let workers = Arc::clone(workers);
            std::thread::spawn(move || {
                let event = match handle.wait_timeout(Duration::from_secs(120)) {
                    Some(Ok(done)) => LlmEvent::Loaded {
                        model: done.model,
                        ms: done.ms,
                        gpu: done.gpu,
                        backend: done.backend,
                    },
                    Some(Err(failure)) => {
                        LlmEvent::error(None, error::LLM_LOAD_FAILED, failure.detail)
                    }
                    None => LlmEvent::error(
                        None,
                        error::LLM_LOAD_FAILED,
                        "the host model load timed out",
                    ),
                };
                workers.cleanup_busy.store(false, Ordering::Release);
                let _ = send(&output, &event.to_json());
            });
            return Ok(());
        }
        LlmRequest::Cleanup {
            job,
            system,
            user,
            max_tokens,
        } => (
            job,
            LlmWork::Cleanup {
                system,
                user,
                max_tokens,
            },
        ),
        LlmRequest::Command {
            job,
            instruction,
            selection,
            max_tokens,
        } => (
            job,
            LlmWork::Command {
                instruction,
                selection,
                max_tokens,
            },
        ),
        LlmRequest::Shutdown => return Ok(()),
    };
    if workers.cleanup_busy.swap(true, Ordering::AcqRel) {
        return Err(invalid("cleanup worker is busy"));
    }
    let handle = workers.cleanup.run(work);
    lock(jobs).insert(job, handle.id());
    let workers = Arc::clone(workers);
    let output = Arc::clone(output);
    let jobs = Arc::clone(jobs);
    std::thread::spawn(move || {
        let event = match handle
            .wait_timeout(Duration::from_secs(120))
            .unwrap_or_else(|| {
                handle.cancel();
                LlmOutcome::Cancelled
            }) {
            LlmOutcome::Done(done) => LlmEvent::Result {
                job,
                text: done.text,
                stats: done.stats,
            },
            LlmOutcome::Cancelled => LlmEvent::Cancelled { job },
            LlmOutcome::Failed(failure) => {
                LlmEvent::error(Some(job), &failure.code, failure.detail)
            }
        };
        let _ = send(&output, &event.to_json());
        lock(&jobs).remove(&job);
        workers.cleanup_busy.store(false, Ordering::Release);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::protocol::write_frame;
    #[test]
    fn expired_audio_transfers_leave_no_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let problem = receive_audio_until(
            &mut &b""[..],
            directory.path(),
            &serde_json::json!({"bytes":1}),
            Instant::now(),
        )
        .unwrap_err();
        assert_eq!(problem.kind(), io::ErrorKind::TimedOut);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn network_loads_keep_the_existing_worker_thread_limit() {
        for threads in [0, 65, u32::MAX] {
            assert!(worker_threads(threads).is_err());
        }
        for threads in [1, 64] {
            assert_eq!(worker_threads(threads).unwrap(), threads as usize);
        }
    }
    #[test]
    fn audio_frames_are_bounded_and_incomplete_audio_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let mut bytes = Vec::new();
        write_frame(&mut bytes, FrameKind::Binary, b"RIFF").unwrap();
        let file = receive_audio(
            &mut &bytes[..],
            directory.path(),
            &serde_json::json!({"bytes":4}),
        )
        .unwrap();
        assert_eq!(std::fs::read(file.path()).unwrap(), b"RIFF");
        assert!(
            receive_audio(
                &mut &bytes[..],
                directory.path(),
                &serde_json::json!({"bytes":5})
            )
            .is_err()
        );
        assert!(
            receive_audio(
                &mut &bytes[..],
                directory.path(),
                &serde_json::json!({"bytes":u64::MAX})
            )
            .is_err()
        );
    }
    #[test]
    fn unknown_model_paths_are_refused() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["/etc/passwd", "../secrets.json", "C:\\Windows\\system.ini"] {
            assert!(model_path(directory.path(), name, ModelKind::Speech).is_err());
        }
    }

    #[test]
    fn a_catalog_filename_without_a_valid_stamp_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let folder = directory.path().join("models/whisper");
        std::fs::create_dir_all(&folder).unwrap();
        let filename = &catalog::embedded().whisper[0].files[0].file;
        std::fs::write(folder.join(filename), b"damaged").unwrap();
        assert!(model_path(directory.path(), filename, ModelKind::Speech).is_err());
    }
}
