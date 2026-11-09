//! A remote upload must stop even while the supervisor waits for socket space.

use hushpen_core::protocol::{Event, PROTOCOL_VERSION, read_frame, write_json};
use hushpen_engine::{
    EngineClient, EngineLog, EngineState, Link, RemoteSpec, Timing, TranscribeSpec,
};
use hushpen_remote::{Identity, RemoteStream, Resolver, WorkerKind, read_worker};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn cancellation_interrupts_a_backpressured_audio_upload() {
    let server = Identity::generate().unwrap();
    let client_identity = Identity::generate().unwrap();
    let server_key = server.public();
    let client_key = client_identity.public();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (upload_started, started) = mpsc::channel();
    let (stop, stopped) = mpsc::channel();
    let server_thread = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        let mut stream =
            RemoteStream::accept(socket, &server, &[client_key], None, Duration::from_secs(3))
                .unwrap()
                .stream;
        assert_eq!(read_worker(&mut stream).unwrap(), WorkerKind::Speech);
        let _hello = read_frame(&mut stream).unwrap().unwrap();
        write_json(
            &mut stream,
            &Event::Ready {
                version: "test".into(),
                protocol: PROTOCOL_VERSION,
                gpu: false,
            }
            .to_json(),
        )
        .unwrap();
        let header = read_frame(&mut stream).unwrap().unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header.payload).unwrap();
        assert_eq!(header["type"], "remote_audio");
        upload_started.send(()).unwrap();
        let _ = stopped.recv_timeout(Duration::from_secs(5));
    });
    let session = RemoteStream::connect(
        address,
        &client_identity,
        server_key,
        None,
        Duration::from_secs(3),
        &Resolver::default(),
    )
    .unwrap();
    let client = EngineClient::start_link(
        Link::Remote(RemoteSpec::new(session.stream, WorkerKind::Speech)),
        Timing::default(),
        EngineLog::discard(),
    );
    assert!(
        client.wait_for(Duration::from_secs(3), |status| status.state
            == EngineState::Ready)
    );
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("upload.wav");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(32 * 1024 * 1024)
        .unwrap();
    let job = client.transcribe(TranscribeSpec {
        wav_path: path,
        language: None,
        prompt: None,
    });
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(job.wait_timeout(Duration::from_millis(100)).is_none());
    let began = Instant::now();
    job.cancel();
    assert!(job.wait_timeout(Duration::from_secs(1)).is_some());
    client.shutdown();
    assert!(began.elapsed() < Duration::from_secs(1));
    let _ = stop.send(());
    server_thread.join().unwrap();
}
