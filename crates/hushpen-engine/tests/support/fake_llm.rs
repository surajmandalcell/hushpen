//! A scripted stand-in for `hushpen-llm`, used only by the supervisor tests. It speaks the real
//! frame protocol and answers `Cleanup` with the upper-cased user text. Odd input steers it:
//! a user text with `hang` gets no answer, and with `stubborn` the job also ignores `Cancel`.
//! The first argument `deaf` makes it say `Ready` and then ignore everything. The argument
//! `slow-ready:<ms>` waits that long before it reads the first frame, like a worker that
//! compiles its GPU backend before it can say `Ready`.

use hushpen_core::llm_protocol::{
    JobStats, LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest, UnloadReason,
};
use hushpen_core::protocol::{read_frame, write_json};
use std::io::{self, Write};
use std::path::Path;

fn send(out: &mut impl Write, event: &LlmEvent) {
    write_json(out, &event.to_json()).expect("the parent closed the pipe");
    out.flush().ok();
}

fn main() {
    let mode = std::env::args().nth(1);
    let deaf = mode.as_deref() == Some("deaf");
    if let Some(ms) = mode
        .as_deref()
        .and_then(|mode| mode.strip_prefix("slow-ready:"))
        .and_then(|ms| ms.parse::<u64>().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    let mut input = io::stdin().lock();
    let mut out = io::stdout().lock();
    let mut model: Option<(String, bool)> = None;
    let mut remembered: Option<(String, bool)> = None;
    let mut hung: Vec<(u64, bool)> = Vec::new();
    while let Ok(Some(frame)) = read_frame(&mut input) {
        let Ok(request) = LlmRequest::from_frame(&frame) else {
            std::process::exit(3);
        };
        if deaf && !matches!(request, LlmRequest::Hello { .. }) {
            continue;
        }
        match request {
            LlmRequest::Hello { .. } => send(
                &mut out,
                &LlmEvent::Ready {
                    version: "fake".into(),
                    protocol: LLM_PROTOCOL_VERSION,
                    backend: "cpu".into(),
                },
            ),
            LlmRequest::LoadModel { path, gpu, .. } => {
                if path.contains("missing") {
                    send(
                        &mut out,
                        &LlmEvent::error(None, "LLM_LOAD_FAILED", "no such file"),
                    );
                    continue;
                }
                let name = Path::new(&path)
                    .file_stem()
                    .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                send(
                    &mut out,
                    &LlmEvent::Loading {
                        model: name.clone(),
                    },
                );
                send(
                    &mut out,
                    &LlmEvent::Loaded {
                        model: name.clone(),
                        ms: 1,
                        gpu,
                        backend: "cpu".into(),
                    },
                );
                remembered = Some((name.clone(), gpu));
                model = Some((name, gpu));
            }
            LlmRequest::Cleanup { job, user, .. } => {
                if model.is_none() {
                    let Some((name, gpu)) = remembered.clone() else {
                        send(
                            &mut out,
                            &LlmEvent::error(Some(job), "LLM_NO_MODEL", "no model"),
                        );
                        continue;
                    };
                    send(
                        &mut out,
                        &LlmEvent::Loaded {
                            model: name.clone(),
                            ms: 1,
                            gpu,
                            backend: "cpu".into(),
                        },
                    );
                    model = Some((name, gpu));
                }
                if user.contains("hang") {
                    hung.push((job, user.contains("stubborn")));
                    continue;
                }
                send(
                    &mut out,
                    &LlmEvent::Result {
                        job,
                        text: user.to_uppercase(),
                        stats: JobStats {
                            prompt_tokens: 10,
                            reused_tokens: 6,
                            new_tokens: 4,
                            gen_tokens: 3,
                            ms: 2,
                            load_ms: 0,
                        },
                    },
                );
            }
            LlmRequest::Command {
                job, instruction, ..
            } => send(
                &mut out,
                &LlmEvent::Result {
                    job,
                    text: instruction,
                    stats: JobStats::default(),
                },
            ),
            LlmRequest::Unload => {
                model = None;
                send(
                    &mut out,
                    &LlmEvent::Unloaded {
                        reason: UnloadReason::Requested,
                    },
                );
            }
            LlmRequest::Cancel { job } => {
                if let Some(position) = hung.iter().position(|(id, _)| *id == job)
                    && !hung[position].1
                {
                    hung.remove(position);
                    send(&mut out, &LlmEvent::Cancelled { job });
                }
            }
            LlmRequest::Ping => send(&mut out, &LlmEvent::Pong),
            LlmRequest::Shutdown => return,
        }
    }
}
