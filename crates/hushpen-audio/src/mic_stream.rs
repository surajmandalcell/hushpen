//! The microphone stream, kept ready so a recording starts at once.
//!
//! Opening a stream takes about a second on PulseAudio, and the audio of that second can never
//! be recovered. A [`MicHub`] therefore opens the stream ahead of time and leaves it paused
//! (a paused stream sends nothing and shows no microphone in use). Starting a recording only
//! resumes it. A start that finds no ready stream queues behind the open: the caller never
//! waits, and a failed open is reported through the session's event sink.
//!
//! One thread owns each stream, because streams are not `Send` on every host.

use crate::capture::{CaptureEvent, EventSink, Msg, Source};
use crate::devices::DEFAULT_ID;
use crate::error::CaptureError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, DeviceId, FromSample, SampleFormat, SizedSample, StreamConfig, SupportedBufferSize,
};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// How long a recording may get no callbacks at all before the capture counts as lost. A
/// PulseAudio virtual source sends nothing while no one plays into it, so this is long; a
/// removed device is caught by the device poll.
const STALL_LIMIT: Duration = Duration::from_secs(10);
/// How often the thread checks that the device it opened still exists.
const DEVICE_POLL: Duration = Duration::from_secs(1);
const MIC_STOP_TIMEOUT: Duration = Duration::from_millis(1500);
/// How often the thread looks for a command.
const COMMAND_WAIT: Duration = Duration::from_millis(250);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Where the stream's audio and errors go: the running session, or nowhere.
#[derive(Default)]
pub(crate) struct Route {
    session: Mutex<Option<(Sender<Msg>, EventSink)>>,
    failed: AtomicBool,
}

impl Route {
    fn attach(&self, attachment: &Attachment) {
        *lock(&self.session) = Some((attachment.tx.clone(), Arc::clone(&attachment.sink)));
        self.failed.store(false, Ordering::SeqCst);
    }

    fn detach(&self) {
        *lock(&self.session) = None;
    }

    pub(crate) fn frames(&self, rate: u32, channels: u16, data: Vec<f32>) {
        if let Some((tx, _)) = &*lock(&self.session) {
            let _ = tx.send(Msg::Frames {
                source: Source::Mic,
                rate,
                channels,
                data,
                at: Instant::now(),
            });
        }
    }

    /// Reports one failure per session.
    pub(crate) fn report(&self, error: CaptureError) {
        let sink = lock(&self.session)
            .as_ref()
            .map(|(_, sink)| Arc::clone(sink));
        if let Some(sink) = sink
            && !self.failed.swap(true, Ordering::SeqCst)
        {
            log::warn!("{error}");
            sink(CaptureEvent::Error(error));
        }
    }
}

/// A stream that can be resumed and paused.
pub(crate) trait Playable {
    fn play(&self) -> Result<(), CaptureError>;
    fn pause(&self) -> Result<(), CaptureError>;
}

pub(crate) struct Opened {
    pub stream: Box<dyn Playable>,
    /// The device the stream reads, to notice when it goes away.
    pub device: Option<String>,
}

/// What the mic thread needs from the audio system. The hub builds one on the thread that
/// uses it.
pub(crate) trait Backend {
    /// Opens a paused stream for `selection` that sends its audio to `route`.
    fn open(
        &mut self,
        selection: &str,
        route: &Arc<Route>,
        last_data: &Arc<AtomicU64>,
        epoch: Instant,
    ) -> Result<Opened, CaptureError>;
    /// The device `selection` names right now, if there is one.
    fn resolve(&mut self, selection: &str) -> Option<String>;
    fn present(&mut self, device: &str) -> bool;
}

type Factory = Arc<dyn Fn() -> Box<dyn Backend> + Send + Sync>;

pub(crate) struct Attachment {
    tx: Sender<Msg>,
    sink: EventSink,
    done: Sender<()>,
}

impl Attachment {
    fn fail(self, error: CaptureError) {
        log::warn!("{error}");
        (self.sink)(CaptureEvent::StartFailed(error));
        let _ = self.done.send(());
    }
}

pub(crate) enum Command {
    Attach(Attachment),
    Detach,
    Close,
}

/// Ends a session's use of the stream.
pub(crate) struct MicHandle {
    pub(crate) stop: Sender<Command>,
    pub(crate) done: Receiver<()>,
}

impl MicHandle {
    /// Waits briefly. A host that hangs while pausing a stream must not hang the app; the
    /// thread is left to finish on its own.
    pub(crate) fn shut_down(self) {
        let _ = self.stop.send(Command::Detach);
        let _ = self.done.recv_timeout(MIC_STOP_TIMEOUT);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Opening,
    Ready,
    Dead,
}

struct Warm {
    selection: String,
    commands: Sender<Command>,
    phase: Arc<Mutex<Phase>>,
}

impl Warm {
    fn usable_for(&self, selection: &str) -> bool {
        self.selection == selection && *lock(&self.phase) != Phase::Dead
    }
}

pub struct MicHub {
    slot: Mutex<Option<Warm>>,
    factory: Factory,
}

impl Default for MicHub {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MicHub {
    fn drop(&mut self) {
        if let Some(warm) = lock(&self.slot).take() {
            let _ = warm.commands.send(Command::Close);
        }
    }
}

impl MicHub {
    pub fn new() -> Self {
        Self::with_factory(Arc::new(|| Box::new(CpalBackend::default())))
    }

    pub(crate) fn with_factory(factory: Factory) -> Self {
        Self {
            slot: Mutex::new(None),
            factory,
        }
    }

    /// Opens the stream for `device` (`"default"` or a device id) and leaves it paused. Returns
    /// at once.
    pub fn warm(&self, device: &str) {
        let mut slot = lock(&self.slot);
        if let Err(error) = self.ensure(&mut slot, device) {
            log::warn!("{error}");
        }
    }

    fn ensure<'a>(
        &self,
        slot: &'a mut Option<Warm>,
        selection: &str,
    ) -> Result<&'a Warm, CaptureError> {
        if !slot.as_ref().is_some_and(|warm| warm.usable_for(selection)) {
            if let Some(old) = slot.take() {
                let _ = old.commands.send(Command::Close);
            }
            *slot = Some(self.spawn(selection)?);
        }
        slot.as_ref()
            .ok_or_else(|| CaptureError::failed("no microphone thread"))
    }

    fn spawn(&self, selection: &str) -> Result<Warm, CaptureError> {
        let (commands, queue) = mpsc::channel();
        let phase = Arc::new(Mutex::new(Phase::Opening));
        let factory = Arc::clone(&self.factory);
        let wanted = selection.to_owned();
        let thread_phase = Arc::clone(&phase);
        thread::Builder::new()
            .name("hushpen-mic".into())
            .spawn(move || run(&wanted, &queue, &thread_phase, &factory))?;
        Ok(Warm {
            selection: selection.to_owned(),
            commands,
            phase,
        })
    }

    /// Starts feeding `tx` from the stream. Returns at once, whether or not the stream is open
    /// yet. An open that fails reaches `sink` as [`CaptureEvent::StartFailed`].
    pub(crate) fn attach(
        &self,
        selection: &str,
        tx: &Sender<Msg>,
        sink: &EventSink,
    ) -> Result<MicHandle, CaptureError> {
        let mut slot = lock(&self.slot);
        for _ in 0..2 {
            let warm = self.ensure(&mut slot, selection)?;
            let (done, done_rx) = mpsc::channel();
            let attachment = Attachment {
                tx: tx.clone(),
                sink: Arc::clone(sink),
                done,
            };
            // The thread sets `Dead` under this lock, and answers every command that came
            // before it did, so a start is never left waiting on a thread that is gone.
            let phase = lock(&warm.phase);
            let sent =
                *phase != Phase::Dead && warm.commands.send(Command::Attach(attachment)).is_ok();
            drop(phase);
            if sent {
                return Ok(MicHandle {
                    stop: warm.commands.clone(),
                    done: done_rx,
                });
            }
            *slot = None;
        }
        Err(CaptureError::unavailable("the microphone did not start"))
    }
}

fn now_ms(epoch: Instant) -> u64 {
    epoch.elapsed().as_millis() as u64
}

/// The thread of one stream: opens it paused, then resumes and pauses it on command until the
/// device changes or the hub lets it go.
fn run(selection: &str, queue: &Receiver<Command>, phase: &Mutex<Phase>, factory: &Factory) {
    let mut backend = factory();
    let epoch = Instant::now();
    let last_data = Arc::new(AtomicU64::new(0));
    let route = Arc::new(Route::default());
    let Opened { stream, device } = match backend.open(selection, &route, &last_data, epoch) {
        Ok(opened) => opened,
        Err(error) => return retire(phase, queue, &error),
    };
    *lock(phase) = Phase::Ready;
    let mut attached: Option<Attachment> = None;
    let mut next_poll = Instant::now() + DEVICE_POLL;
    loop {
        match queue.recv_timeout(COMMAND_WAIT) {
            Ok(Command::Attach(attachment)) => {
                route.attach(&attachment);
                last_data.store(now_ms(epoch), Ordering::SeqCst);
                match stream.play() {
                    Ok(()) => {
                        (attachment.sink)(CaptureEvent::Started);
                        attached = Some(attachment);
                    }
                    Err(error) => {
                        route.detach();
                        attachment.fail(error.clone());
                        return retire(phase, queue, &error);
                    }
                }
            }
            Ok(Command::Detach) => {
                if let Some(attachment) = attached.take() {
                    if let Err(error) = stream.pause() {
                        log::warn!("{error}");
                        route.detach();
                        let _ = attachment.done.send(());
                        return retire(phase, queue, &error);
                    }
                    route.detach();
                    let _ = attachment.done.send(());
                }
            }
            Ok(Command::Close) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if Instant::now() < next_poll {
            continue;
        }
        next_poll = Instant::now() + DEVICE_POLL;
        if attached.is_some() {
            let silent_for = epoch
                .elapsed()
                .saturating_sub(Duration::from_millis(last_data.load(Ordering::SeqCst)));
            if silent_for > STALL_LIMIT {
                route.report(CaptureError::unavailable(
                    "the microphone stopped sending audio",
                ));
            } else if let Some(device) = &device
                && !backend.present(device)
            {
                // PulseAudio moves a stream to another source when its source goes away, so
                // the stream itself never reports the loss.
                route.report(CaptureError::unavailable("the microphone was removed"));
            }
        } else if backend.resolve(selection) != device {
            // The device went away, or the system default moved: open again at the next use.
            return retire(
                phase,
                queue,
                &CaptureError::unavailable("the microphone changed"),
            );
        }
    }
}

/// Marks the stream dead and answers the starts that were already queued with the reason.
fn retire(phase: &Mutex<Phase>, queue: &Receiver<Command>, error: &CaptureError) {
    *lock(phase) = Phase::Dead;
    while let Ok(command) = queue.try_recv() {
        if let Command::Attach(attachment) = command {
            attachment.fail(error.clone());
        }
    }
}

/// The real microphones, through cpal.
#[derive(Default)]
struct CpalBackend {
    host: Option<cpal::Host>,
}

impl CpalBackend {
    fn host(&mut self) -> &cpal::Host {
        self.host.get_or_insert_with(cpal::default_host)
    }

    fn device(&mut self, selection: &str) -> Result<cpal::Device, CaptureError> {
        find_device(self.host(), selection)
    }
}

impl Backend for CpalBackend {
    fn open(
        &mut self,
        selection: &str,
        route: &Arc<Route>,
        last_data: &Arc<AtomicU64>,
        epoch: Instant,
    ) -> Result<Opened, CaptureError> {
        let device = self.device(selection)?;
        let config = device.default_input_config()?;
        let stream = build_stream(&device, &config, route, last_data, epoch)?;
        Ok(Opened {
            stream: Box::new(CpalStream(stream)),
            device: device.id().ok().map(|id| id.to_string()),
        })
    }

    fn resolve(&mut self, selection: &str) -> Option<String> {
        self.device(selection)
            .ok()
            .and_then(|device| device.id().ok())
            .map(|id| id.to_string())
    }

    fn present(&mut self, device: &str) -> bool {
        let Ok(mut devices) = self.host().input_devices() else {
            return true;
        };
        devices.any(|found| found.id().is_ok_and(|id| id.to_string() == device))
    }
}

struct CpalStream(cpal::Stream);

impl Playable for CpalStream {
    fn play(&self) -> Result<(), CaptureError> {
        Ok(self.0.play()?)
    }

    fn pause(&self) -> Result<(), CaptureError> {
        Ok(self.0.pause()?)
    }
}

/// Finds the cpal device for a saved id. `"default"` follows the system.
fn find_device(host: &cpal::Host, selection: &str) -> Result<cpal::Device, CaptureError> {
    if selection.is_empty() || selection == DEFAULT_ID {
        return host
            .default_input_device()
            .ok_or_else(|| CaptureError::unavailable("no default microphone"));
    }
    let id = DeviceId::from_str(selection)
        .map_err(|error| CaptureError::unavailable(format!("bad device id: {error}")))?;
    host.device_by_id(&id)
        .ok_or_else(|| CaptureError::unavailable(format!("microphone {selection} is not there")))
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    route: &Arc<Route>,
    last_data: &Arc<AtomicU64>,
    epoch: Instant,
) -> Result<cpal::Stream, CaptureError> {
    let build = |stream_config: StreamConfig| match config.sample_format() {
        SampleFormat::F32 => typed::<f32>(device, stream_config, route, last_data, epoch),
        SampleFormat::I16 => typed::<i16>(device, stream_config, route, last_data, epoch),
        SampleFormat::U16 => typed::<u16>(device, stream_config, route, last_data, epoch),
        SampleFormat::I32 => typed::<i32>(device, stream_config, route, last_data, epoch),
        other => Err(CaptureError::failed(format!(
            "unsupported microphone sample format {other:?}"
        ))),
    };
    // PulseAudio hands out about 2 s per callback unless a size is requested, which would
    // delay the level meter and look like a stalled microphone.
    let mut stream_config = config.config();
    if let SupportedBufferSize::Range { min, max } = *config.buffer_size() {
        stream_config.buffer_size = BufferSize::Fixed((config.sample_rate() / 50).clamp(min, max));
        if let Ok(stream) = build(stream_config) {
            return Ok(stream);
        }
        stream_config.buffer_size = BufferSize::Default;
    }
    build(stream_config)
}

fn typed<T>(
    device: &cpal::Device,
    config: StreamConfig,
    route: &Arc<Route>,
    last_data: &Arc<AtomicU64>,
    epoch: Instant,
) -> Result<cpal::Stream, CaptureError>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let (rate, channels) = (config.sample_rate, config.channels);
    let sink = Arc::clone(route);
    let last_data = Arc::clone(last_data);
    let on_error = Arc::clone(route);
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _| {
            last_data.store(now_ms(epoch), Ordering::SeqCst);
            let data = data
                .iter()
                .map(|sample| sample.to_sample::<f32>())
                .collect();
            sink.frames(rate, channels, data);
        },
        move |error| {
            use cpal::ErrorKind::{DeviceChanged, RealtimeDenied, Xrun};
            if matches!(error.kind(), DeviceChanged | Xrun | RealtimeDenied) {
                log::debug!("microphone stream: {error}");
            } else {
                on_error.report(error.into());
            }
        },
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests;
