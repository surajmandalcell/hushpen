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

/// Why a shortcut was not accepted: the code the UI and the hook show, and a sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

impl Refusal {
    /// Another app holds the shortcut. Only the platform layer can tell, after it tried.
    pub fn in_use() -> Self {
        Self {
            code: crate::error::SHORTCUT_IN_USE,
            message: "Another app is using this shortcut. Choose a different one.".to_owned(),
        }
    }

    pub fn invalid(problem: Problem, platform: Platform) -> Self {
        Self {
            code: SHORTCUT_INVALID,
            message: problem.message(platform),
        }
    }
}

fn slot_name(slot: Slot) -> &'static str {
    match slot {
        Slot::Hold => "the hold key",
        Slot::HandsFree => "hands-free",
        Slot::PasteLast => "paste last",
        Slot::Command => "Command Mode",
    }
}

/// Checks a recorded shortcut for `slot` against the system's shortcuts and the other slots.
/// `others` is what the other slots hold now.
pub fn check(
    slot: Slot,
    combo: &Combo,
    others: &BTreeMap<Slot, Setting>,
    platform: Platform,
) -> Result<(), Refusal> {
    if combo.reserved(platform) {
        return Err(Refusal {
            code: SHORTCUT_RESERVED,
            message: "The system uses this shortcut. Choose a different one.".to_owned(),
        });
    }
    let taken = others
        .iter()
        .filter(|(other, _)| **other != slot)
        .find(|(_, setting)| setting.combo() == Some(combo));
    if let Some((other, _)) = taken {
        return Err(Refusal {
            code: SHORTCUT_RESERVED,
            message: format!(
                "Hushpen already uses this shortcut for {}. Choose a different one.",
                slot_name(*other)
            ),
        });
    }
    Ok(())
}

/// The live shortcuts for what the slots hold. A slot in `unavailable` (another app holds it)
/// stays saved but does not run.
pub fn bindings(values: &BTreeMap<Slot, Setting>, unavailable: &BTreeSet<Slot>) -> Bindings {
    let live = |slot: Slot| values.get(&slot).filter(|_| !unavailable.contains(&slot));
    let combo = |slot: Slot| live(slot).and_then(Setting::combo).cloned();
    Bindings {
        hold: combo(Slot::Hold),
        hands_free: live(Slot::HandsFree).map(|setting| match setting {
            Setting::HandsFree(hands_free) => hands_free.clone(),
            Setting::Keys(combo) => HandsFree::Custom(combo.clone()),
        }),
        paste_last: combo(Slot::PasteLast),
        command: combo(Slot::Command),
    }
}
