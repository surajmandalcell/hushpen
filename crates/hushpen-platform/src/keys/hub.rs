//! The part of the key listener that no system owns: the engine behind one lock, and the two
//! sinks it answers to. The platform thread calls [`Hub::press`] and [`Hub::release`]; the app
//! thread rebinds and records through the same hub.

use super::{RecordSink, Sink};
use hushpen_core::shortcut::{Bindings, Engine, Output, Phys};
use std::sync::{Mutex, MutexGuard};

pub(super) struct Hub {
    engine: Mutex<Engine>,
    sink: Sink,
    record: RecordSink,
}

impl Hub {
    pub(super) fn new(bindings: Bindings, sink: Sink, record: RecordSink) -> Self {
        Self {
            engine: Mutex::new(Engine::new(bindings)),
            sink,
            record,
        }
    }

    fn engine(&self) -> MutexGuard<'_, Engine> {
        self.engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Sends on after the lock is gone, so a sink can call back into the hub.
    fn send(&self, outputs: Vec<Output>) {
        for output in outputs {
            match output {
                Output::App(event) => (self.sink)(event),
                Output::Record(progress) => (self.record)(progress),
            }
        }
    }

    pub(super) fn press(&self, key: Phys) {
        let outputs = self.engine().press(key);
        self.send(outputs);
    }

    pub(super) fn release(&self, key: Phys) {
        let outputs = self.engine().release(key);
        self.send(outputs);
    }

    #[cfg(target_os = "macos")]
    pub(super) fn reset(&self) {
        let outputs = self.engine().reset();
        self.send(outputs);
    }

    pub(super) fn set_bindings(&self, bindings: Bindings) {
        let outputs = self.engine().set_bindings(bindings);
        self.send(outputs);
    }

    pub(super) fn set_escape(&self, escape: bool) {
        self.engine().set_escape(escape);
    }

    pub(super) fn start_recording(&self) {
        self.engine().start_recording();
    }

    pub(super) fn stop_recording(&self) {
        self.engine().stop_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::dictation::AppEvent;
    use hushpen_core::shortcut::{Combo, Key, Modifier, Platform, Recording, Side};
    use std::sync::Arc;

    const R_ALT: Phys = Phys::Modifier(Modifier::Alt, Side::Right);

    type Log<T> = Arc<Mutex<Vec<T>>>;

    fn hub(hold: Option<Combo>) -> (Hub, Log<AppEvent>, Log<Recording>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let records = Arc::new(Mutex::new(Vec::new()));
        let sink_events = Arc::clone(&events);
        let sink_records = Arc::clone(&records);
        let hub = Hub::new(
            Bindings {
                hold,
                ..Bindings::default()
            },
            Arc::new(move |event| sink_events.lock().unwrap().push(event)),
            Arc::new(move |record| sink_records.lock().unwrap().push(record)),
        );
        (hub, events, records)
    }

    fn right_alt() -> Combo {
        Combo::new([Key::Right(Modifier::Alt)], Platform::Linux).unwrap()
    }

    #[test]
    fn the_hold_key_reaches_the_app_sink() {
        let (hub, events, _) = hub(Some(right_alt()));
        hub.press(R_ALT);
        hub.release(R_ALT);
        assert_eq!(
            *events.lock().unwrap(),
            vec![AppEvent::HoldDown, AppEvent::HoldUp]
        );
    }

    #[test]
    fn rebinding_while_held_lets_the_old_hold_go() {
        let (hub, events, _) = hub(Some(right_alt()));
        hub.press(R_ALT);
        hub.set_bindings(Bindings::default());
        assert_eq!(
            *events.lock().unwrap(),
            vec![AppEvent::HoldDown, AppEvent::HoldUp]
        );
    }

    #[test]
    fn the_recorder_answers_to_its_own_sink_and_the_hold_key_stays_quiet() {
        let (hub, events, records) = hub(Some(right_alt()));
        hub.start_recording();
        hub.press(R_ALT);
        hub.release(R_ALT);
        assert!(events.lock().unwrap().is_empty());
        assert_eq!(
            records.lock().unwrap().last(),
            Some(&Recording::Captured(vec![Key::Right(Modifier::Alt)]))
        );
    }
}
