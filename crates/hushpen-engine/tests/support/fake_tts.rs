//! A scripted stand-in for `hushpen-tts`, used only by the supervisor tests. It speaks the real
//! frame protocol and answers `Speak` with two short audio frames, then `Done`. Odd input steers
//! it: a text with `hang` gets no answer until its `Cancel`, a text with `crash` ends the
//! process, and a model folder with `missing` fails to load.

use hushpen_core::protocol::read_frame;
use hushpen_core::tts_protocol::{
    SpeakStats, TTS_PROTOCOL_VERSION, TtsEvent, TtsRequest, write_event,
};
use std::io::{self, Write};
use std::path::Path;

fn send(out: &mut impl Write, event: &TtsEvent) {
    write_event(out, event).expect("the parent closed the pipe");
    out.flush().ok();
}

fn main() {
    let mut input = io::stdin().lock();
    let mut out = io::stdout().lock();
    let mut hung: Vec<u64> = Vec::new();
    while let Ok(Some(frame)) = read_frame(&mut input) {
        let Ok(request) = TtsRequest::from_frame(&frame) else {
            std::process::exit(3);
        };
        match request {
            TtsRequest::Hello { .. } => send(
                &mut out,
                &TtsEvent::Ready {
                    version: "fake".into(),
                    protocol: TTS_PROTOCOL_VERSION,
                },
            ),
            TtsRequest::LoadModel { dir, .. } if dir.contains("missing") => send(
                &mut out,
                &TtsEvent::error(None, "TTS_LOAD_FAILED", "no such folder"),
            ),
            TtsRequest::LoadModel { dir, .. } => {
                let model = Path::new(&dir)
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
                send(&mut out, &TtsEvent::Loaded { model, ms: 1 });
            }
            TtsRequest::Speak { job, text, .. } => {
                if text.contains("crash") {
                    std::process::exit(9);
                }
                if text.contains("hang") {
                    hung.push(job);
                    continue;
                }
                for samples in [vec![0.25; 240], vec![-0.25; 120]] {
                    send(
                        &mut out,
                        &TtsEvent::Audio {
                            job,
                            sample_rate: 24_000,
                            samples,
                        },
                    );
                }
                let stats = SpeakStats {
                    audio_ms: 15,
                    ms: 2,
                    load_ms: 0,
                    first_audio_ms: 1,
                };
                send(&mut out, &TtsEvent::Done { job, stats });
            }
            TtsRequest::Cancel { job } => {
                if let Some(position) = hung.iter().position(|id| *id == job) {
                    hung.remove(position);
                    send(&mut out, &TtsEvent::Cancelled { job });
                }
            }
            TtsRequest::ListVoices | TtsRequest::Unload => {}
            TtsRequest::Ping => send(&mut out, &TtsEvent::Pong),
            TtsRequest::Shutdown => return,
        }
    }
}
