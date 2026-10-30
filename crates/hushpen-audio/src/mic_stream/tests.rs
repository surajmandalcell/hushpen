use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Clone, Default)]
struct World {
    opens: Arc<AtomicUsize>,
    open_ms: Arc<AtomicU64>,
    fail: Arc<Mutex<Option<CaptureError>>>,
    device: Arc<Mutex<String>>,
    log: Arc<Mutex<Vec<&'static str>>>,
    route: Arc<Mutex<Option<Arc<Route>>>>,
}

struct FakeStream(Arc<Mutex<Vec<&'static str>>>);

impl Playable for FakeStream {
    fn play(&self) -> Result<(), CaptureError> {
        lock(&self.0).push("play");
        Ok(())
    }

    fn pause(&self) -> Result<(), CaptureError> {
        lock(&self.0).push("pause");
        Ok(())
    }
}

struct FakeBackend(World);

impl Backend for FakeBackend {
    fn open(
        &mut self,
        _selection: &str,
        route: &Arc<Route>,
        _last_data: &Arc<AtomicU64>,
        _epoch: Instant,
    ) -> Result<Opened, CaptureError> {
        thread::sleep(Duration::from_millis(self.0.open_ms.load(Ordering::SeqCst)));
        self.0.opens.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = lock(&self.0.fail).clone() {
            return Err(error);
        }
        *lock(&self.0.route) = Some(Arc::clone(route));
        Ok(Opened {
            stream: Box::new(FakeStream(Arc::clone(&self.0.log))),
            device: Some(lock(&self.0.device).clone()),
        })
    }

    fn resolve(&mut self, _selection: &str) -> Option<String> {
        Some(lock(&self.0.device).clone())
    }

    fn present(&mut self, _device: &str) -> bool {
        true
    }
}

fn hub(world: &World) -> MicHub {
    *lock(&world.device) = "mic-1".into();
    let world = world.clone();
    MicHub::with_factory(Arc::new(move || Box::new(FakeBackend(world.clone()))))
}

struct Session {
    tx: Sender<Msg>,
    frames: Receiver<Msg>,
    sink: EventSink,
    events: Receiver<CaptureEvent>,
}

fn session() -> Session {
    let (tx, frames) = mpsc::channel();
    let (event_tx, events) = mpsc::channel();
    let event_tx = Mutex::new(event_tx);
    let sink: EventSink = Arc::new(move |event| {
        let _ = lock(&event_tx).send(event);
    });
    Session {
        tx,
        frames,
        sink,
        events,
    }
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let give_up = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < give_up, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(5));
    }
}

fn next_event(session: &Session) -> CaptureEvent {
    session
        .events
        .recv_timeout(Duration::from_secs(5))
        .expect("a capture event")
}

fn push(world: &World) {
    let route = lock(&world.route).clone().expect("an open stream");
    route.frames(16_000, 1, vec![0.25; 160]);
}

#[test]
fn a_start_returns_at_once_while_the_stream_is_still_opening() {
    let world = World::default();
    world.open_ms.store(600, Ordering::SeqCst);
    let hub = hub(&world);
    let session = session();

    let asked = Instant::now();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert!(
        asked.elapsed() < Duration::from_millis(50),
        "attach took {:?}",
        asked.elapsed()
    );
    assert_eq!(world.opens.load(Ordering::SeqCst), 0, "still opening");

    assert_eq!(next_event(&session), CaptureEvent::Started);
    push(&world);
    assert!(matches!(
        session.frames.recv_timeout(Duration::from_secs(1)),
        Ok(Msg::Frames { .. })
    ));
    handle.shut_down();
}

#[test]
fn a_stream_opened_ahead_of_time_starts_at_once_and_is_reused() {
    let world = World::default();
    let hub = hub(&world);
    hub.warm("default");
    wait_for("the warm open", || world.opens.load(Ordering::SeqCst) == 1);
    wait_for("the stream to be ready", || {
        lock(&hub.slot)
            .as_ref()
            .is_some_and(|warm| *lock(&warm.phase) == Phase::Ready)
    });
    assert!(lock(&world.log).is_empty(), "a warm stream stays paused");

    let session = session();
    let asked = Instant::now();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);
    assert!(
        asked.elapsed() < Duration::from_millis(100),
        "resuming took {:?}",
        asked.elapsed()
    );
    handle.shut_down();

    let again = self::session();
    let handle = hub.attach("default", &again.tx, &again.sink).unwrap();
    assert_eq!(next_event(&again), CaptureEvent::Started);
    handle.shut_down();
    assert_eq!(
        world.opens.load(Ordering::SeqCst),
        1,
        "one open for two sessions"
    );
    assert_eq!(*lock(&world.log), ["play", "pause", "play", "pause"]);
}

#[test]
fn audio_reaches_the_session_only_while_it_is_attached() {
    let world = World::default();
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);
    push(&world);
    assert!(session.frames.recv_timeout(Duration::from_secs(1)).is_ok());

    handle.shut_down();
    push(&world);
    assert!(
        session
            .frames
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "a paused stream sends nothing to the old session"
    );
}

#[test]
fn a_start_before_the_last_stop_takes_the_stream_and_both_sessions_keep_their_audio() {
    let world = World::default();
    let hub = hub(&world);
    let old = session();
    let old_handle = hub.attach("default", &old.tx, &old.sink).unwrap();
    assert_eq!(next_event(&old), CaptureEvent::Started);

    // The new session starts while the old one still waits for its tail.
    let new = session();
    let new_handle = hub.attach("default", &new.tx, &new.sink).unwrap();
    assert_eq!(next_event(&new), CaptureEvent::Started);
    push(&world);
    assert!(old.frames.recv_timeout(Duration::from_secs(1)).is_ok());
    assert!(new.frames.recv_timeout(Duration::from_secs(1)).is_ok());

    let before = Instant::now();
    old_handle.shut_down();
    assert!(before.elapsed() < Duration::from_millis(500));
    push(&world);
    assert!(new.frames.recv_timeout(Duration::from_secs(1)).is_ok());
    assert!(
        old.frames.recv_timeout(Duration::from_millis(100)).is_err(),
        "the old session hears nothing once it let go"
    );
    assert_eq!(*lock(&world.log), ["play"], "the new session is not paused");

    new_handle.shut_down();
    assert_eq!(*lock(&world.log), ["play", "pause"]);
}

#[test]
fn a_failed_open_reaches_the_session_and_the_next_start_opens_again() {
    let world = World::default();
    *lock(&world.fail) = Some(CaptureError::unavailable("no microphone"));
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(
        next_event(&session),
        CaptureEvent::StartFailed(CaptureError::unavailable("no microphone"))
    );
    handle.shut_down();
    // A fast host can fail the first open before `attach` looks, and `attach` then opens a
    // second time on its own, so count only the open that the next start causes.
    let failed_opens = world.opens.load(Ordering::SeqCst);

    *lock(&world.fail) = None;
    let again = self::session();
    let handle = hub.attach("default", &again.tx, &again.sink).unwrap();
    assert_eq!(next_event(&again), CaptureEvent::Started);
    assert_eq!(world.opens.load(Ordering::SeqCst), failed_opens + 1);
    handle.shut_down();
}

#[test]
fn a_stream_that_reports_a_failure_is_retired_and_the_next_start_opens_again() {
    let world = World::default();
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);

    // The stream fails while recording, as a wedged audio unit does through its error
    // callback.
    let route = lock(&world.route).clone().expect("an open stream");
    route.report(CaptureError::failed("audio unit died"));
    assert_eq!(
        next_event(&session),
        CaptureEvent::Error(CaptureError::failed("audio unit died"))
    );
    handle.shut_down();

    wait_for("the broken stream to retire", || {
        lock(&hub.slot)
            .as_ref()
            .is_some_and(|warm| *lock(&warm.phase) == Phase::Dead)
    });
    assert!(
        lock(&world.log).contains(&"pause"),
        "the broken stream is stopped"
    );

    let again = self::session();
    let handle = hub.attach("default", &again.tx, &again.sink).unwrap();
    assert_eq!(next_event(&again), CaptureEvent::Started);
    assert_eq!(
        world.opens.load(Ordering::SeqCst),
        2,
        "a fresh stream opens"
    );
    handle.shut_down();
}

#[test]
fn a_silent_run_is_not_failed_and_its_audio_reaches_the_session() {
    let world = World::default();
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);

    // A muted microphone or a silent virtual source delivers buffers of exact zeros (2 s per
    // buffer here). The run must end as "no speech", never as a capture failure.
    let route = lock(&world.route).clone().expect("an open stream");
    for _ in 0..3 {
        route.frames(16_000, 1, vec![0.0; 32_000]);
    }
    assert!(
        session
            .events
            .recv_timeout(Duration::from_millis(300))
            .is_err(),
        "a silent run raises no capture event"
    );
    let delivered = session
        .frames
        .try_iter()
        .filter(|message| matches!(message, Msg::Frames { data, .. } if data.len() == 32_000))
        .count();
    assert_eq!(delivered, 3, "every silent buffer reaches the session");
    handle.shut_down();
}

#[test]
fn a_stream_that_ends_a_session_on_exact_zeros_is_retired_and_the_next_start_opens_again() {
    let world = World::default();
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);

    // A wedged audio unit keeps delivering buffers, all zeros. Nothing is raised during the
    // run, but the stream is not trusted for the next one.
    let route = lock(&world.route).clone().expect("an open stream");
    for _ in 0..3 {
        route.frames(16_000, 1, vec![0.0; 32_000]);
    }
    handle.shut_down();
    assert_eq!(
        lock(&hub.slot).as_ref().map(|warm| *lock(&warm.phase)),
        Some(Phase::Dead),
        "the stream is dead before the end of the session is announced"
    );
    assert!(
        lock(&world.log).contains(&"pause"),
        "the dead stream is stopped"
    );

    let again = self::session();
    let handle = hub.attach("default", &again.tx, &again.sink).unwrap();
    assert_eq!(next_event(&again), CaptureEvent::Started);
    assert_eq!(
        world.opens.load(Ordering::SeqCst),
        2,
        "a fresh stream opens"
    );
    handle.shut_down();
}

#[test]
fn quiet_audio_above_exact_zeros_is_not_dead_input() {
    let world = World::default();
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);

    // 6 s of the faintest nonzero noise (a real microphone's noise floor), plus 3 s of zeros
    // that reset before the limit.
    let route = lock(&world.route).clone().expect("an open stream");
    route.frames(16_000, 1, vec![0.0; 32_000]);
    for _ in 0..3 {
        route.frames(16_000, 1, vec![f32::MIN_POSITIVE; 32_000]);
    }
    route.frames(16_000, 1, vec![0.0; 32_000]);
    assert!(
        session
            .events
            .recv_timeout(Duration::from_millis(300))
            .is_err(),
        "quiet audio is not a dead input"
    );
    handle.shut_down();

    let again = self::session();
    let handle = hub.attach("default", &again.tx, &again.sink).unwrap();
    assert_eq!(next_event(&again), CaptureEvent::Started);
    assert_eq!(
        world.opens.load(Ordering::SeqCst),
        1,
        "the working stream is reused"
    );
    handle.shut_down();
}

#[test]
fn a_microphone_that_was_muted_and_then_spoke_is_kept() {
    let world = World::default();
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);

    // 6 s of zeros pass the limit; real audio after them clears it.
    let route = lock(&world.route).clone().expect("an open stream");
    for _ in 0..3 {
        route.frames(16_000, 1, vec![0.0; 32_000]);
    }
    push(&world);
    handle.shut_down();

    let again = self::session();
    let handle = hub.attach("default", &again.tx, &again.sink).unwrap();
    assert_eq!(next_event(&again), CaptureEvent::Started);
    assert_eq!(
        world.opens.load(Ordering::SeqCst),
        1,
        "the stream that carried real audio is reused"
    );
    handle.shut_down();
}

#[test]
fn another_microphone_opens_another_stream() {
    let world = World::default();
    let hub = hub(&world);
    hub.warm("default");
    hub.warm("mic-2");
    wait_for("both opens", || world.opens.load(Ordering::SeqCst) == 2);
    hub.warm("mic-2");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        world.opens.load(Ordering::SeqCst),
        2,
        "the same one is not opened again"
    );
}

#[test]
fn a_device_that_changed_while_idle_is_opened_again_at_the_next_start() {
    let world = World::default();
    let hub = hub(&world);
    hub.warm("default");
    wait_for("the warm open", || world.opens.load(Ordering::SeqCst) == 1);

    *lock(&world.device) = "mic-2".into();
    wait_for("the stream to retire", || {
        lock(&hub.slot)
            .as_ref()
            .is_some_and(|warm| *lock(&warm.phase) == Phase::Dead)
    });
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);
    assert_eq!(world.opens.load(Ordering::SeqCst), 2);
    handle.shut_down();
}

#[test]
fn a_stream_that_is_replaced_during_a_session_does_not_hold_up_its_stop() {
    let world = World::default();
    let hub = hub(&world);
    let session = session();
    let handle = hub.attach("default", &session.tx, &session.sink).unwrap();
    assert_eq!(next_event(&session), CaptureEvent::Started);

    hub.warm("mic-2");
    wait_for("the second open", || {
        world.opens.load(Ordering::SeqCst) == 2
    });
    push(&world);
    // The newest route is the second stream's, so the session heard nothing from it.
    assert!(
        session
            .frames
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    let before = Instant::now();
    handle.shut_down();
    assert!(before.elapsed() < Duration::from_millis(500));
}
