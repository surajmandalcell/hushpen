//! Turns raw key presses and releases into pipeline events, and records new shortcuts.
//!
//! The platform layer translates its native events into [`Phys`] keys and calls [`Engine::press`]
//! and [`Engine::release`]. Both answer with what to send on: pipeline events for the live
//! shortcuts, or recorder progress while a new shortcut is being recorded. While the recorder is
//! open no live shortcut fires.
//!
//! The engine also says which key events should stop at Hushpen ([`Engine::swallows`]): every key
//! but a modifier while the recorder is open, and the key that completes a live shortcut, so
//! the front app does not also act on Cmd+Tab or F5.

use super::{Combo, HandsFree, Key, Modifier, Named};
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
    Named(Named),
    /// A key with no name, by the system's number (see [`Key::Code`]).
    Code(u32),
    Esc,
}

impl Phys {
    fn recorded(self) -> Key {
        match self {
            Phys::Modifier(modifier, Side::Left) => Key::Any(modifier),
            Phys::Modifier(modifier, Side::Right) => Key::Right(modifier),
            Phys::Fn => Key::Fn,
            Phys::Char(c) => Key::Char(c),
            Phys::Named(named) => Key::Named(named),
            Phys::Code(code) => Key::Code(code),
            Phys::Esc => Key::Named(Named::Escape),
        }
    }

    /// A modifier or Fn: its events always reach the front app.
    fn is_modifier(self) -> bool {
        matches!(self, Phys::Modifier(..) | Phys::Fn)
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
    pub read_aloud: Option<Combo>,
}

impl Bindings {
    /// Every live shortcut that has keys of its own. The built-in hands-free has none.
    pub fn chords(&self) -> impl Iterator<Item = &Combo> {
        [
            self.hold.as_ref(),
            self.command.as_ref(),
            self.paste_last.as_ref(),
            self.read_aloud.as_ref(),
            self.hands_free.as_ref().and_then(HandsFree::combo),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recording {
    /// The keys pressed so far, still held or just let go.
    Progress(Vec<Key>),
    /// Every key is up: the shortcut is the most keys that were down together.
    Captured(Vec<Key>),
    /// Esc with no other key down. The recorder is closed.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    App(AppEvent),
    Record(Recording),
}

fn accepts(combo: &Combo, key: Key, phys: Phys, claimed: &BTreeSet<Phys>) -> bool {
    match (key, phys) {
        (Key::Fn, Phys::Fn) => true,
        (Key::Char(a), Phys::Char(b)) => a == b,
        (Key::Named(a), Phys::Named(b)) => a == b,
        (Key::Named(Named::Escape), Phys::Esc) => true,
        (Key::Code(a), Phys::Code(b)) => a == b,
        (Key::Right(wanted), Phys::Modifier(modifier, Side::Right)) => wanted == modifier,
        // A right-hand key that the hold or Command Mode shortcut uses does not count as the
        // plain modifier of another shortcut, or one chord would start two things. With the
        // right key in the same chord, the plain one is the left key.
        (Key::Any(wanted), Phys::Modifier(modifier, side)) => {
            wanted == modifier
                && (side == Side::Left || (!claimed.contains(&phys) && !combo.left_only(wanted)))
        }
        _ => false,
    }
}

fn satisfied(combo: &Combo, down: &BTreeSet<Phys>, claimed: &BTreeSet<Phys>) -> bool {
    combo
        .keys()
        .iter()
        .all(|key| down.iter().any(|phys| accepts(combo, *key, *phys, claimed)))
}

fn uses(combo: &Combo, phys: Phys, claimed: &BTreeSet<Phys>) -> bool {
    combo
        .keys()
        .iter()
        .any(|key| accepts(combo, *key, phys, claimed))
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
    command_on: bool,
    hands_free_armed: bool,
    paste_last_armed: bool,
    read_aloud_armed: bool,
    /// Esc belongs to the pipeline: it is sent on as [`AppEvent::Esc`].
    escape: bool,
    recorder: Option<Recorder>,
    /// The keys whose press stopped at Hushpen, so their repeats and release stop too.
    swallowed: BTreeSet<Phys>,
}

impl Engine {
    pub fn new(bindings: Bindings) -> Self {
        let mut engine = Self {
            bindings: Bindings::default(),
            claimed: BTreeSet::new(),
            down: BTreeSet::new(),
            hold_on: false,
            command_on: false,
            hands_free_armed: false,
            paste_last_armed: false,
            read_aloud_armed: false,
            escape: false,
            recorder: None,
            swallowed: BTreeSet::new(),
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
        let (command_was_on, hold_was_on) = (
            std::mem::take(&mut self.command_on),
            std::mem::take(&mut self.hold_on),
        );
        if self.recorder.is_none() {
            if command_was_on {
                out.push(Output::App(AppEvent::CommandUp));
            }
            if hold_was_on {
                out.push(Output::App(AppEvent::HoldUp));
            }
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
        self.read_aloud_armed = false;
        self.hold_on = self.hold_satisfied();
        self.command_on = self.command_satisfied();
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
        self.read_aloud_armed = false;
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

    fn command_satisfied(&self) -> bool {
        self.bindings
            .command
            .as_ref()
            .is_some_and(|combo| satisfied(combo, &self.down, &self.claimed))
    }

    /// Whether the keys that are down so far could still grow into the command chord.
    fn chord_possible(&self) -> bool {
        self.bindings.command.as_ref().is_some_and(|combo| {
            self.down
                .iter()
                .all(|phys| uses(combo, *phys, &self.claimed))
        })
    }

    /// The hold and command events for what the last key change switched on or off. The
    /// command keys contain the hold key, so a release sends the command first: the command
    /// session is ended by its own event. A hold key that the command chord may still join is
    /// only armed, and a chord that goes down in one step starts no hold at all.
    fn follow_hold(&mut self) -> Vec<AppEvent> {
        let mut events = Vec::new();
        let (hold, command) = (self.hold_satisfied(), self.command_satisfied());
        if !command && self.command_on {
            events.push(AppEvent::CommandUp);
        }
        if !hold && self.hold_on {
            events.push(AppEvent::HoldUp);
        }
        if hold && !self.hold_on && !command {
            events.push(if self.chord_possible() {
                AppEvent::HoldArm
            } else {
                AppEvent::HoldDown
            });
        }
        if command && !self.command_on {
            events.push(AppEvent::CommandDown);
        }
        self.hold_on = hold;
        self.command_on = command;
        events
    }

    /// Whether the last press of `key` stopped at Hushpen. Ask after [`Engine::press`] for the
    /// press and its repeats, and before [`Engine::release`] for the release.
    pub fn swallows(&self, key: Phys) -> bool {
        self.swallowed.contains(&key)
    }

    /// Whether the keys down now are exactly a live shortcut that has `key` as its main key, or
    /// Space with the hold key for the built-in hands-free.
    fn completes_shortcut(&self, key: Phys) -> bool {
        let exact = |combo: &Combo| {
            uses(combo, key, &self.claimed)
                && satisfied(combo, &self.down, &self.claimed)
                && self
                    .down
                    .iter()
                    .all(|phys| uses(combo, *phys, &self.claimed))
        };
        self.bindings.chords().any(exact)
            || (key == Phys::Char(' ')
                && self.hold_on
                && self.bindings.hands_free == Some(HandsFree::Default))
    }

    pub fn press(&mut self, key: Phys) -> Vec<Output> {
        let recording = self.recorder.is_some();
        let out = self.press_key(key);
        if !key.is_modifier() && (recording || self.completes_shortcut(key)) {
            self.swallowed.insert(key);
        }
        out
    }

    fn press_key(&mut self, key: Phys) -> Vec<Output> {
        if !self.down.insert(key) {
            return Vec::new();
        }
        if let Some(recorder) = &mut self.recorder {
            // Esc alone closes the recorder; with other keys down it is part of the shortcut.
            if key == Phys::Esc && recorder.held.is_empty() {
                self.recorder = None;
                self.follow_hold();
                return vec![Output::Record(Recording::Cancelled)];
            }
            recorder.held.insert(key);
            recorder.peak.insert(key.recorded());
            let progress =
                Output::Record(Recording::Progress(recorder.peak.iter().copied().collect()));
            self.follow_hold();
            return vec![progress];
        }
        let mut out = Vec::new();
        if key == Phys::Esc && self.escape {
            out.push(Output::App(AppEvent::Esc));
        }
        out.extend(self.follow_hold().into_iter().map(Output::App));
        if key == Phys::Char(' ')
            && self.hold_on
            && !self.command_on
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
        press_toggle(
            self.bindings.read_aloud.as_ref(),
            &mut self.read_aloud_armed,
            &self.down,
            &self.claimed,
        );
        out
    }

    pub fn release(&mut self, key: Phys) -> Vec<Output> {
        self.swallowed.remove(&key);
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
        let mut out: Vec<Output> = self.follow_hold().into_iter().map(Output::App).collect();
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
        if release_toggle(
            self.bindings.read_aloud.as_ref(),
            &mut self.read_aloud_armed,
            &self.down,
            &self.claimed,
        ) {
            out.push(Output::App(AppEvent::ReadAloud));
        }
        out
    }

    /// The platform lost its key stream (a macOS tap was replaced), so nothing is down any
    /// more. A hold that was on is let go.
    pub fn reset(&mut self) -> Vec<Output> {
        self.down.clear();
        self.swallowed.clear();
        self.hands_free_armed = false;
        self.paste_last_armed = false;
        self.read_aloud_armed = false;
        if let Some(recorder) = &mut self.recorder {
            recorder.held.clear();
        }
        let (command_was_on, hold_was_on) = (
            std::mem::take(&mut self.command_on),
            std::mem::take(&mut self.hold_on),
        );
        let mut out = Vec::new();
        if self.recorder.is_none() {
            if command_was_on {
                out.push(Output::App(AppEvent::CommandUp));
            }
            if hold_was_on {
                out.push(Output::App(AppEvent::HoldUp));
            }
        }
        out
    }
}
