//! What a slot holds, and the rules a new shortcut must pass before the app saves it.

use super::{Bindings, Combo, HandsFree, Platform, Problem, Slot};
use crate::error::{SHORTCUT_INVALID, SHORTCUT_RESERVED};
use std::collections::{BTreeMap, BTreeSet};

/// The value of one slot. Hands-free may also be the built-in gestures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting {
    Keys(Combo),
    HandsFree(HandsFree),
}

impl Setting {
    pub fn parse(slot: Slot, text: &str, platform: Platform) -> Result<Self, Problem> {
        match slot {
            Slot::HandsFree => HandsFree::parse(text, platform).map(Setting::HandsFree),
            _ => Combo::parse(text, platform).map(Setting::Keys),
        }
    }

    /// The factory default, which always parses.
    pub fn default_for(slot: Slot, platform: Platform) -> Self {
        Self::parse(slot, slot.default_text(platform), platform)
            .unwrap_or_else(|_| unreachable!("the default of {slot:?} is a valid shortcut"))
    }

    pub fn to_text(&self, platform: Platform) -> String {
        match self {
            Setting::Keys(combo) => combo.to_setting(platform),
            Setting::HandsFree(hands_free) => hands_free.to_setting(platform),
        }
    }

    /// The keys of the shortcut. The built-in hands-free gestures have none of their own.
    pub fn combo(&self) -> Option<&Combo> {
        match self {
            Setting::Keys(combo) => Some(combo),
            Setting::HandsFree(hands_free) => hands_free.combo(),
        }
    }

    /// The words shown to the user. `hold` names the hold key for the built-in hands-free text.
    pub fn display(&self, hold: &Combo, platform: Platform) -> String {
        match self {
            Setting::Keys(combo) => combo.display(platform),
            Setting::HandsFree(hands_free) => hands_free.display(hold, platform),
        }
    }
}

/// How the user reaches the settings that hold a conflicting shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opener {
    /// A `x-apple.systempreferences:` link.
    Url(String),
    /// A program and its arguments, such as `systemsettings kcm_keys`.
    Command(Vec<String>),
}

/// Something else runs a shortcut: a macOS system shortcut that is on, or another X11 app that
/// grabbed it. Only the platform layer can tell, after it asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Who uses it, such as `macOS uses this shortcut for "Move focus to next window".`
    pub message: String,
    /// How to free it, in one line.
    pub steps: String,
    /// The settings to open, when the system has a way to open them.
    pub open: Option<Opener>,
}

/// Why a shortcut was not accepted, or why a saved one does not run: the code the UI and the
/// hook show, a sentence, and for a conflict the way to free the shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
    pub steps: Option<String>,
    pub open: Option<Opener>,
}

impl Refusal {
    pub fn new(code: &'static str, message: String) -> Self {
        Self {
            code,
            message,
            steps: None,
            open: None,
        }
    }

    pub fn conflict(conflict: Conflict) -> Self {
        Self {
            code: crate::error::SHORTCUT_IN_USE,
            message: conflict.message,
            steps: Some(conflict.steps),
            open: conflict.open,
        }
    }

    pub fn invalid(problem: Problem, platform: Platform) -> Self {
        Self::new(SHORTCUT_INVALID, problem.message(platform))
    }
}

fn slot_name(slot: Slot) -> &'static str {
    match slot {
        Slot::Hold => "the hold key",
        Slot::HandsFree => "hands-free",
        Slot::PasteLast => "paste last",
        Slot::Command => "command mode",
        Slot::ReadAloud => "read aloud",
    }
}

/// Checks a recorded shortcut for `slot` against the other slots, the one thing Hushpen itself
/// refuses. `others` is what the other slots hold now, and a slot in `off` does not count.
/// What the system already runs is asked of the platform layer.
pub fn check(
    slot: Slot,
    combo: &Combo,
    others: &BTreeMap<Slot, Setting>,
    off: &BTreeSet<Slot>,
) -> Result<(), Refusal> {
    let taken = others
        .iter()
        .filter(|(other, _)| **other != slot && !off.contains(*other))
        .find(|(_, setting)| setting.combo() == Some(combo));
    if let Some((other, _)) = taken {
        return Err(Refusal::new(
            SHORTCUT_RESERVED,
            format!(
                "Hushpen already uses this shortcut for {}. Choose a different one.",
                slot_name(*other)
            ),
        ));
    }
    Ok(())
}

/// The live shortcuts for what the slots hold. A slot in `unavailable` (another app holds it)
/// or in `off` stays saved but does not run. With the hold key off, the key stays live for the
/// built-in hands-free gestures; the pipeline then does not record while it is held.
pub fn bindings(
    values: &BTreeMap<Slot, Setting>,
    unavailable: &BTreeSet<Slot>,
    off: &BTreeSet<Slot>,
) -> Bindings {
    let saved = |slot: Slot| values.get(&slot).filter(|_| !unavailable.contains(&slot));
    let live = |slot: Slot| saved(slot).filter(|_| !off.contains(&slot));
    let combo = |slot: Slot| live(slot).and_then(Setting::combo).cloned();
    let hands_free = live(Slot::HandsFree).map(|setting| match setting {
        Setting::HandsFree(hands_free) => hands_free.clone(),
        Setting::Keys(combo) => HandsFree::Custom(combo.clone()),
    });
    let hold = if off.contains(&Slot::Hold) {
        saved(Slot::Hold)
            .filter(|_| hands_free == Some(HandsFree::Default))
            .and_then(Setting::combo)
            .cloned()
    } else {
        combo(Slot::Hold)
    };
    Bindings {
        hold,
        hands_free,
        paste_last: combo(Slot::PasteLast),
        command: combo(Slot::Command),
        read_aloud: combo(Slot::ReadAloud),
    }
}
