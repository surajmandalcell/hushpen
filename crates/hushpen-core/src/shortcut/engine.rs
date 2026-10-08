//! Turns raw key presses and releases into pipeline events, and records new shortcuts.
//!
//! The platform layer translates its native events into [`Phys`] keys and calls [`Engine::press`]
//! and [`Engine::release`]. Both answer with what to send on: pipeline events for the live
//! shortcuts, or recorder progress while a new shortcut is being recorded. While the recorder is
//! open no live shortcut fires.

use super::{Combo, HandsFree, Key, Modifier};
use crate::dictation::AppEvent;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Left,
    Right,
}

/// A physical key as the platform reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phys {
    Modifier(Modifier, Side),
    Fn,
    /// A lowercase character key. Space is `' '`.
    Char(char),
    Esc,
}

impl Phys {
    fn recorded(self) -> Option<Key> {
        match self {
            Phys::Modifier(modifier, Side::Left) => Some(Key::Any(modifier)),
            Phys::Modifier(modifier, Side::Right) => Some(Key::Right(modifier)),
            Phys::Fn => Some(Key::Fn),
            Phys::Char(c) if c.is_ascii_alphanumeric() => Some(Key::Char(c)),
            Phys::Char(_) | Phys::Esc => None,
        }
    }
}

/// The shortcuts that are live. A slot is `None` when it has no shortcut that can run, for
/// example because another app holds it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    pub hold: Option<Combo>,
    pub hands_free: Option<HandsFree>,
    pub paste_last: Option<Combo>,
    pub command: Option<Combo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recording {
    /// The keys pressed so far, still held or just let go.
    Progress(Vec<Key>),
    /// Every key is up: the shortcut is the most keys that were down together.
    Captured(Vec<Key>),
    /// Esc. The recorder is closed.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    App(AppEvent),
    Record(Recording),
}

fn accepts(key: Key, phys: Phys, claimed: &BTreeSet<Phys>) -> bool {
    match (key, phys) {
        (Key::Fn, Phys::Fn) => true,
        (Key::Char(a), Phys::Char(b)) => a == b,
        (Key::Right(wanted), Phys::Modifier(modifier, Side::Right)) => wanted == modifier,
        // A right-hand key that the hold or Command Mode shortcut uses does not count as the
        // plain modifier of another shortcut, or one chord would start two things.
        (Key::Any(wanted), Phys::Modifier(modifier, side)) => {
            wanted == modifier && (side == Side::Left || !claimed.contains(&phys))
        }
        _ => false,
    }
}

fn satisfied(combo: &Combo, down: &BTreeSet<Phys>, claimed: &BTreeSet<Phys>) -> bool {
    combo
        .keys()
        .iter()
        .all(|key| down.iter().any(|phys| accepts(*key, *phys, claimed)))
}

fn uses(combo: &Combo, phys: Phys, claimed: &BTreeSet<Phys>) -> bool {
    combo.keys().iter().any(|key| accepts(*key, phys, claimed))
}

/// A shortcut that fires when it is let go. It arms when the last key goes down with no other
/// key held, and disarms when any other key joins, so `Ctrl+V` never fires `Ctrl+Alt+V`.
fn press_toggle(
    combo: Option<&Combo>,
    armed: &mut bool,
    down: &BTreeSet<Phys>,
    claimed: &BTreeSet<Phys>,
) {
    let Some(combo) = combo else {
        return;
    };
    if !down.iter().all(|phys| uses(combo, *phys, claimed)) {
        *armed = false;
    } else if satisfied(combo, down, claimed) {
        *armed = true;
    }
}

fn release_toggle(
    combo: Option<&Combo>,
    armed: &mut bool,
    down: &BTreeSet<Phys>,
    claimed: &BTreeSet<Phys>,
) -> bool {
    let Some(combo) = combo else {
        return false;
    };
    if !*armed || down.iter().any(|phys| uses(combo, *phys, claimed)) {
        return false;
    }
    *armed = false;
    true
}

#[derive(Debug, Default)]
struct Recorder {
    held: BTreeSet<Phys>,
    peak: BTreeSet<Key>,
}

#[derive(Debug)]
pub struct Engine {
    bindings: Bindings,
    claimed: BTreeSet<Phys>,
    down: BTreeSet<Phys>,
    hold_on: bool,
    hands_free_armed: bool,
    paste_last_armed: bool,
    /// Esc belongs to the pipeline: it is sent on as [`AppEvent::Esc`].
    escape: bool,
    recorder: Option<Recorder>,
}

impl Engine {
    pub fn new(bindings: Bindings) -> Self {
        let mut engine = Self {
            bindings: Bindings::default(),
            claimed: BTreeSet::new(),
            down: BTreeSet::new(),
            hold_on: false,
            hands_free_armed: false,
            paste_last_armed: false,
            escape: false,
            recorder: None,
        };
        engine.set_bindings(bindings);
        engine
    }

    pub fn bindings(&self) -> &Bindings {
        &self.bindings
    }

    /// Whether Esc is sent on as [`AppEvent::Esc`].
    pub fn set_escape(&mut self, escape: bool) {
        self.escape = escape;
    }

    /// Swaps the live shortcuts. A hold that is on is let go first, and a key that is already
    /// down starts nothing.
    pub fn set_bindings(&mut self, bindings: Bindings) -> Vec<Output> {
        let mut out = Vec::new();
        if std::mem::take(&mut self.hold_on) && self.recorder.is_none() {
            out.push(Output::App(AppEvent::HoldUp));
        }
        self.claimed = [&bindings.hold, &bindings.command]
            .into_iter()
            .flatten()
            .flat_map(|combo| combo.keys().iter())
            .filter_map(|key| match key {
                Key::Right(modifier) => Some(Phys::Modifier(*modifier, Side::Right)),
                _ => None,
            })
            .collect();
        self.bindings = bindings;
        self.hands_free_armed = false;
        self.paste_last_armed = false;
        self.hold_on = self.hold_satisfied();
        out
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Opens the recorder. Keys that are down now are ignored until they come up.
    pub fn start_recording(&mut self) {
        self.recorder = Some(Recorder::default());
        self.hands_free_armed = false;
        self.paste_last_armed = false;
    }

    pub fn stop_recording(&mut self) {
        self.recorder = None;
    }

    fn hold_satisfied(&self) -> bool {
        self.bindings
            .hold
            .as_ref()
            .is_some_and(|combo| satisfied(combo, &self.down, &self.claimed))
    }

    /// The hold event when the hold shortcut just went on or off.
    fn follow_hold(&mut self) -> Option<AppEvent> {
        let on = self.hold_satisfied();
        if on == self.hold_on {
            return None;
        }
        self.hold_on = on;
        Some(if on {
            AppEvent::HoldDown
        } else {
            AppEvent::HoldUp
        })
    }

    pub fn press(&mut self, key: Phys) -> Vec<Output> {
        if !self.down.insert(key) {
            return Vec::new();
        }
        if let Some(recorder) = &mut self.recorder {
            let progress = if key == Phys::Esc {
                self.recorder = None;
                self.follow_hold();
                return vec![Output::Record(Recording::Cancelled)];
            } else {
                recorder.held.insert(key);
                key.recorded().map(|recorded| {
                    recorder.peak.insert(recorded);
                    Output::Record(Recording::Progress(recorder.peak.iter().copied().collect()))
                })
            };
            self.follow_hold();
            return progress.into_iter().collect();
        }
        let mut out = Vec::new();
        if key == Phys::Esc && self.escape {
            out.push(Output::App(AppEvent::Esc));
        }
        out.extend(self.follow_hold().map(Output::App));
        if key == Phys::Char(' ')
            && self.hold_on
            && self.bindings.hands_free == Some(HandsFree::Default)
        {
            out.push(Output::App(AppEvent::HandsFreeToggle));
        }
        let custom = self.bindings.hands_free.as_ref().and_then(HandsFree::combo);
        press_toggle(
            custom,
            &mut self.hands_free_armed,
            &self.down,
            &self.claimed,
        );
        press_toggle(
            self.bindings.paste_last.as_ref(),
            &mut self.paste_last_armed,
            &self.down,
            &self.claimed,
        );
        out
    }

    pub fn release(&mut self, key: Phys) -> Vec<Output> {
        if !self.down.remove(&key) {
            return Vec::new();
        }
        if let Some(recorder) = &mut self.recorder {
            let finished =
                recorder.held.remove(&key) && recorder.held.is_empty() && !recorder.peak.is_empty();
            let keys: Vec<Key> = recorder.peak.iter().copied().collect();
            self.follow_hold();
            if !finished {
                return Vec::new();
            }
            self.recorder = None;
            return vec![Output::Record(Recording::Captured(keys))];
        }
        let mut out: Vec<Output> = self.follow_hold().map(Output::App).into_iter().collect();
        let custom = self.bindings.hands_free.as_ref().and_then(HandsFree::combo);
        if release_toggle(
            custom,
            &mut self.hands_free_armed,
            &self.down,
            &self.claimed,
        ) {
            out.push(Output::App(AppEvent::HandsFreeToggle));
        }
        if release_toggle(
            self.bindings.paste_last.as_ref(),
            &mut self.paste_last_armed,
            &self.down,
            &self.claimed,
        ) {
            out.push(Output::App(AppEvent::PasteLast));
        }
        out
    }

    /// The platform lost its key stream (a macOS tap was replaced), so nothing is down any
    /// more. A hold that was on is let go.
    pub fn reset(&mut self) -> Vec<Output> {
        self.down.clear();
        self.hands_free_armed = false;
        self.paste_last_armed = false;
        if let Some(recorder) = &mut self.recorder {
            recorder.held.clear();
        }
        match std::mem::take(&mut self.hold_on) {
            true if self.recorder.is_none() => vec![Output::App(AppEvent::HoldUp)],
            _ => Vec::new(),
        }
    }
}
