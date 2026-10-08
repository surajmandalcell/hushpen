//! Settings > Shortcuts: the hold key, hands-free, paste last, and the Command Mode key.
//!
//! This owns what each slot holds, the recorder, and the rules a new shortcut passes: it must be
//! a shortcut at all, not reserved by the system or used by another slot, and not held by
//! another app. A refused shortcut keeps the old value. The key listener is reached through
//! [`KeyControl`], so the rules run the same with or without a keyboard.

pub mod panel;

use crate::hook;
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{Context, FocusHandle};
use hushpen_core::error::SETTINGS_CORRUPT;
use hushpen_core::shortcut::{
    Bindings, Combo, Key, Platform, Recording, Refusal, Setting, Slot, bindings, check,
};
use hushpen_platform::keys::{GlobalKeys, RecordSink};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// A recorder that sees no key for this long closes, so a forgotten one cannot hold back the
/// hold key for good.
const RECORDER_IDLE: Duration = Duration::from_secs(10);

/// What the shortcut rules need from the key listener.
pub trait KeyControl {
    fn set_bindings(&self, bindings: Bindings);
    fn start_recording(&self);
    fn stop_recording(&self);
    fn in_use(&self, combo: &Combo) -> bool;
}

impl KeyControl for GlobalKeys {
    fn set_bindings(&self, bindings: Bindings) {
        GlobalKeys::set_bindings(self, bindings);
    }

    fn start_recording(&self) {
        GlobalKeys::start_recording(self);
    }

    fn stop_recording(&self) {
        GlobalKeys::stop_recording(self);
    }

    fn in_use(&self, combo: &Combo) -> bool {
        GlobalKeys::in_use(self, combo)
    }
}

struct Recorder {
    slot: Slot,
    progress: Vec<Key>,
    /// Tells a timeout that belongs to an older recorder from the current one.
    id: u64,
}

pub struct Shortcuts {
    storage: Rc<Storage>,
    platform: Platform,
    keys: Option<Rc<dyn KeyControl>>,
    /// Why no shortcut can be recorded, when the key listener is off.
    why_off: Option<String>,
    values: BTreeMap<Slot, Setting>,
    failures: BTreeMap<Slot, Refusal>,
    /// The slots whose shortcut another app holds: saved, but not live.
    taken: BTreeSet<Slot>,
    recorder: Option<Recorder>,
    recorders_opened: u64,
    records: UnboundedSender<Recording>,
    pub(crate) field_focus: Vec<FocusHandle>,
    pub(crate) reset_focus: FocusHandle,
}

impl Shortcuts {
    pub fn new(storage: Rc<Storage>, platform: Platform, cx: &mut Context<Self>) -> Self {
        let (records, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some(recording) = queue.next().await {
                if this
                    .update(cx, |me, cx| me.on_recording(recording, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let values = Slot::ALL
            .into_iter()
            .map(|slot| (slot, saved(&storage, slot, platform)))
            .collect();
        Self {
            storage,
            platform,
            keys: None,
            why_off: Some("Global keys are not running.".to_owned()),
            values,
            failures: BTreeMap::new(),
            taken: BTreeSet::new(),
            recorder: None,
            recorders_opened: 0,
            records,
            field_focus: Slot::ALL
                .iter()
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            reset_focus: cx.focus_handle().tab_stop(true),
        }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// The shortcuts to start the key listener with.
    pub fn bindings(&self) -> Bindings {
        bindings(&self.values, &self.taken)
    }

    /// Where the key listener reports the recorder. Thread safe.
    pub fn record_sink(&self) -> RecordSink {
        let records = self.records.clone();
        Arc::new(move |recording| {
            let _ = records.unbounded_send(recording);
        })
    }

    /// Hands over the running key listener, or says why there is none. Each shortcut is tried
    /// against the other apps now, so a chord that another app holds shows its error at start
    /// and does not run.
    pub fn attach_keys(
        &mut self,
        keys: Result<Rc<dyn KeyControl>, String>,
        cx: &mut Context<Self>,
    ) {
        match keys {
            Ok(keys) => {
                self.keys = Some(keys);
                self.why_off = None;
                let slots: Vec<Slot> = self.values.keys().copied().collect();
                for slot in slots {
                    if self.held_by_other_app(slot) {
                        self.mark_taken(slot);
                    }
                }
                self.apply_live();
            }
            Err(why) => {
                self.keys = None;
                self.why_off = Some(why);
            }
        }
        cx.notify();
    }

    fn held_by_other_app(&self, slot: Slot) -> bool {
        let combo = self.values.get(&slot).and_then(Setting::combo);
        match (&self.keys, combo) {
            (Some(keys), Some(combo)) => keys.in_use(combo),
            _ => false,
        }
    }

    fn mark_taken(&mut self, slot: Slot) {
        self.taken.insert(slot);
        self.failures.insert(slot, Refusal::in_use());
        hook::record_event("shortcut", &format!("{} in-use", slot.key()));
    }

    fn apply_live(&self) {
        if let Some(keys) = &self.keys {
            keys.set_bindings(self.bindings());
        }
    }

    pub fn value(&self, slot: Slot) -> &Setting {
        &self.values[&slot]
    }

    pub fn setting_text(&self, slot: Slot) -> String {
        self.value(slot).to_text(self.platform)
    }

    /// The words for the field of `slot`, in this system's key names.
    pub fn display(&self, slot: Slot) -> String {
        let hold = match self.value(Slot::Hold) {
            Setting::Keys(combo) => combo.clone(),
            Setting::HandsFree(_) => unreachable!("the hold slot holds keys"),
        };
        self.value(slot).display(&hold, self.platform)
    }

    pub fn failure(&self, slot: Slot) -> Option<&Refusal> {
        self.failures.get(&slot)
    }

    /// Whether the shortcut of `slot` runs: another app does not hold it.
    pub fn is_live(&self, slot: Slot) -> bool {
        !self.taken.contains(&slot)
    }

    pub fn recording(&self) -> Option<Slot> {
        self.recorder.as_ref().map(|recorder| recorder.slot)
    }

    /// The keys pressed so far in the open recorder, in this system's names.
    pub fn progress(&self) -> Option<String> {
        let recorder = self.recorder.as_ref()?;
        let text = Combo::display_keys(&recorder.progress, self.platform);
        (!text.is_empty()).then_some(text)
    }

    pub fn why_off(&self) -> Option<&str> {
        self.why_off.as_deref()
    }

    /// Opens the recorder for `slot`. No live shortcut runs while it is open.
    pub fn start_recording(&mut self, slot: Slot, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(why) = &self.why_off {
            return Err(why.clone());
        }
        let Some(keys) = self.keys.clone() else {
            return Err("Global keys are not running.".to_owned());
        };
        if self.recorder.is_some() {
            keys.stop_recording();
        }
        self.failures.remove(&slot);
        self.recorders_opened += 1;
        let id = self.recorders_opened;
        self.recorder = Some(Recorder {
            slot,
            progress: Vec::new(),
            id,
        });
        keys.start_recording();
        hook::record_event("shortcut", &format!("{} recording", slot.key()));
        cx.spawn(async move |this, cx| {
            let mut idle = RECORDER_IDLE;
            loop {
                cx.background_executor().timer(idle).await;
                let Ok(again) = this.update(cx, |me, cx| me.idle_check(id, cx)) else {
                    return;
                };
                match again {
                    Some(wait) => idle = wait,
                    None => return,
                }
            }
        })
        .detach();
        cx.notify();
        Ok(())
    }

    /// Closes a recorder that saw no key for [`RECORDER_IDLE`]. Returns how long to wait again
    /// while it is still being used.
    fn idle_check(&mut self, id: u64, cx: &mut Context<Self>) -> Option<Duration> {
        let recorder = self
            .recorder
            .as_ref()
            .filter(|recorder| recorder.id == id)?;
        if recorder.progress.is_empty() {
            self.cancel_recording(cx);
            None
        } else {
            Some(RECORDER_IDLE)
        }
    }

    pub fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        if self.recorder.take().is_some() {
            if let Some(keys) = &self.keys {
                keys.stop_recording();
            }
            hook::record_event("shortcut", "recording cancelled");
            cx.notify();
        }
    }

    fn on_recording(&mut self, recording: Recording, cx: &mut Context<Self>) {
        let Some(slot) = self.recording() else {
            return;
        };
        match recording {
            Recording::Progress(keys) => {
                if let Some(recorder) = &mut self.recorder {
                    recorder.progress = keys;
                }
            }
            Recording::Cancelled => {
                self.recorder = None;
                hook::record_event("shortcut", "recording cancelled");
            }
            Recording::Captured(keys) => {
                self.recorder = None;
                self.apply(slot, keys);
            }
        }
        cx.notify();
    }

    /// Tries a recorded shortcut for `slot`. A refusal is shown on the slot and nothing changes.
    fn apply(&mut self, slot: Slot, keys: Vec<Key>) {
        match self.try_apply(slot, keys) {
            Ok(()) => {
                self.failures.remove(&slot);
                self.taken.remove(&slot);
                self.apply_live();
                hook::record_event(
                    "shortcut",
                    &format!("{} {}", slot.key(), self.setting_text(slot)),
                );
            }
            Err(refusal) => {
                hook::record_event("shortcut", &format!("{} {}", slot.key(), refusal.code));
                self.failures.insert(slot, refusal);
            }
        }
    }

    fn try_apply(&mut self, slot: Slot, keys: Vec<Key>) -> Result<(), Refusal> {
        let combo = Combo::new(keys, self.platform)
            .map_err(|problem| Refusal::invalid(problem, self.platform))?;
        check(slot, &combo, &self.values, self.platform)?;
        if self.keys.as_ref().is_some_and(|keys| keys.in_use(&combo)) {
            return Err(Refusal::in_use());
        }
        let setting = match slot {
            Slot::HandsFree => Setting::HandsFree(hushpen_core::shortcut::HandsFree::Custom(combo)),
            _ => Setting::Keys(combo),
        };
        self.save(slot, setting)
    }

    fn save(&mut self, slot: Slot, setting: Setting) -> Result<(), Refusal> {
        self.storage
            .settings
            .set(slot.setting_key(), json!(setting.to_text(self.platform)))
            .map_err(|error| Refusal {
                code: SETTINGS_CORRUPT,
                message: format!("The shortcut could not be saved: {error}"),
            })?;
        self.values.insert(slot, setting);
        Ok(())
    }

    /// Puts all four shortcuts back to the factory defaults of this system. A default that
    /// another app holds is saved too and shows the in-use error.
    pub fn reset_all(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.recorder.take().is_some()
            && let Some(keys) = &self.keys
        {
            keys.stop_recording();
        }
        self.failures.clear();
        self.taken.clear();
        for slot in Slot::ALL {
            self.save(slot, Setting::default_for(slot, self.platform))
                .map_err(|refusal| refusal.message)?;
        }
        for slot in Slot::ALL {
            if self.held_by_other_app(slot) {
                self.mark_taken(slot);
            }
        }
        self.apply_live();
        hook::record_event("shortcut", "reset");
        cx.notify();
        Ok(())
    }

    /// `hookctl state` section `shortcuts`.
    pub fn state_json(&self) -> Value {
        let slots: serde_json::Map<String, Value> = Slot::ALL
            .into_iter()
            .map(|slot| {
                (
                    slot.key().to_owned(),
                    json!({
                        "display": self.display(slot),
                        "setting": self.setting_text(slot),
                        "live": self.is_live(slot),
                        "error": self.failure(slot).map(|refusal| json!({
                            "code": refusal.code,
                            "message": refusal.message,
                        })),
                    }),
                )
            })
            .collect();
        json!({
            "recording": self.recording().map(Slot::key),
            "progress": self.progress(),
            "recorder_available": self.why_off.is_none(),
            "slots": slots,
        })
    }
}

/// The saved text of a slot, or the default when the text is missing or is not a shortcut.
fn saved(storage: &Storage, slot: Slot, platform: Platform) -> Setting {
    let text = storage.settings.get(slot.setting_key());
    match text.as_ref().and_then(Value::as_str) {
        Some(text) => Setting::parse(slot, text, platform).unwrap_or_else(|problem| {
            log::warn!(
                "{} is not a shortcut ({problem:?}); the default is used",
                slot.setting_key()
            );
            Setting::default_for(slot, platform)
        }),
        None => Setting::default_for(slot, platform),
    }
}

#[cfg(test)]
mod tests;
