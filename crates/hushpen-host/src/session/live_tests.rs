use crate::{Workers, serve_session};
use hushpen_core::{idle::Idle, os::Os, remote::UnloadRule};
use hushpen_engine::{
    AsrEngine, CancelFlag, EngineClient, EngineError, EngineLog, InProcess, JobOutcome, Link,
    LlmClient, LoadSpec, PcmSource, RemoteSpec, Timing, TranscribeOptions, TranscribeSpec,
    Transcript,
    server::{ServerConfig, serve},
};
use hushpen_remote::{Identity, RemoteStream, Resolver, WorkerKind};
use std::net::TcpListener;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

struct SampleCount;

impl AsrEngine for SampleCount {
    fn gpu_in_use(&self) -> bool {
        false
    }

    fn transcribe_live(
        &mut self,
        source: &mut dyn PcmSource,
        _: &TranscribeOptions,
        _: &CancelFlag,
        partial: &mut dyn FnMut(&str),
    ) -> Result<Transcript, EngineError> {
        let mut samples = Vec::new();
        while source.read(&mut samples)? {
            partial(&samples.len().to_string());
        }
        assert_eq!(samples.first(), Some(&0.25));
        assert_eq!(samples.last(), Some(&-0.5));
        Ok(Transcript {
            text: samples.len().to_string(),
            language: "en".into(),
            segments: vec![],
        })
    }

    fn transcribe_stream(
        &mut self,
        _: &mut dyn PcmSource,
        _: &TranscribeOptions,
        _: &CancelFlag,
        _: &mut dyn FnMut(f32),
    ) -> Result<Transcript, EngineError> {
        panic!("paired live recording used the batch path")
    }
}

#[test]
fn paired_live_forwards_new_pcm_before_close_and_discards_host_audio_on_disconnect() {
    let root = tempfile::tempdir().unwrap();
    let speech = EngineClient::start_link(
        Link::InProcess(InProcess::new(|input, output| {
            serve(
                input,
                output,
                ServerConfig {
                    version: "test".into(),
                    gpu_available: false,
                    loader: Arc::new(|_, _| Ok(Box::new(SampleCount))),
                },
            );
        })),
        Timing::default(),
        EngineLog::discard(),
    );
    let wait = Duration::from_secs(5);
    speech
        .load_model(LoadSpec {
            path: root.path().join("fixture.bin"),
            gpu: false,
            threads: 1,
            idle: Idle::KEEP,
        })
        .wait_timeout(wait)
        .unwrap()
        .unwrap();
    let idle = InProcess::new(|mut input, _| {
        let _ = std::io::copy(&mut input, &mut std::io::sink());
    });
    let cleanup = LlmClient::start_link(Link::InProcess(idle), Timing::llm(), EngineLog::discard());
    let workers = Workers::from_clients(speech, cleanup, Os::Linux, false);
    for cancel in [false, true] {
        let server = Identity::generate().unwrap();
        let client = Identity::generate().unwrap();
        let server_key = server.public();
        let client_key = client.public();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let host_root = root.path().join("host");
        let host_cache = host_root.join("cache/remote");
        let host_workers = Arc::clone(&workers);
        let host = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let peer = RemoteStream::accept(socket, &server, &[client_key], None, wait).unwrap();
            let _ = serve_session(
                peer.stream,
                host_workers,
                &host_root,
                UnloadRule::KeepLoaded,
            );
        });
        let peer =
            RemoteStream::connect(addr, &client, server_key, None, wait, &Resolver::default())
                .unwrap();
        let engine = EngineClient::start_link(
            Link::Remote(RemoteSpec::new(peer.stream, WorkerKind::Speech)),
            Timing::default(),
            EngineLog::discard(),
        );
        assert!(engine.wait_for(wait, |status| status.remote_live));
        let path = root.path().join("local.wav");
        let mut source = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        source.flush().unwrap();
        let (partial, words) = mpsc::channel();
        let job = engine.transcribe_live(
            TranscribeSpec {
                wav_path: path,
                language: Some("en".into()),
                prompt: None,
            },
            Arc::new(move |text| {
                let _ = partial.send(text);
            }),
        );
        for _ in 0..1_600 {
            source.write_sample(8192_i16).unwrap();
        }
        source.flush().unwrap();
        assert_eq!(words.recv_timeout(wait).unwrap(), "1600");
        assert!(job.try_outcome().is_none());
        assert_eq!(std::fs::read_dir(&host_cache).unwrap().count(), 1);
        if cancel {
            job.cancel();
        } else {
            source.write_sample(-16384_i16).unwrap();
            source.finalize().unwrap();
            job.finish_live();
            let Some(JobOutcome::Done(done)) = job.wait_timeout(wait) else {
                panic!("missing final result")
            };
            assert_eq!(done.text, "1601");
        }
        engine.shutdown();
        host.join().unwrap();
        let until = Instant::now() + wait;
        while workers
            .speech_busy
            .load(std::sync::atomic::Ordering::Acquire)
            && Instant::now() < until
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(std::fs::read_dir(&host_cache).unwrap().count(), 0);
    }
    workers.shutdown();
}
