//! The anonymous usage number that the first update check of a day sends (D-56).
//!
//! One integer: bits 0-9 hold the uses of the reported day, capped at 1023; bits 10-25 hold the
//! models used that day, one bit for each slot of [`MODEL_SLOTS`]; bits 26-27 hold the format
//! version, 1. A use is one completed dictation, file transcription, or read aloud. The update
//! API keeps the same slot list, so a slot never changes its model: a removed model keeps its
//! slot, and a new model takes the next free one.

/// The model id that History stores, and its name.
pub const MODEL_SLOTS: [(&str, &str); 11] = [
    ("nemotron-labs-unified-asr-0.6b-q8_0", "Nemotron"),
    ("parakeet-v3-q4_k", "Parakeet v3"),
    ("large-v3-turbo-q5_0", "Whisper Large v3 Turbo"),
    ("small-q5_1", "Whisper Small"),
    ("base-q5_1", "Whisper Base"),
    ("s1-mini", "S1-mini"),
    ("qwen3.5-2b", "Qwen3.5 2B"),
    ("system", "System voice"),
    ("kokoro", "Kokoro"),
    ("chatterbox-turbo", "Chatterbox Turbo"),
    ("chatterbox-nano", "Chatterbox Nano"),
];

pub const MAX_USES: u32 = 1023;
const VERSION: u32 = 1;

/// The uses of one day and the slots of the models used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub uses: u32,
    pub slots: u16,
}

impl Usage {
    /// The usage of a day from its count and the model ids History stored. Models that have no
    /// slot are left out.
    pub fn new(uses: i64, models: &[String]) -> Self {
        let slots = MODEL_SLOTS
            .iter()
            .enumerate()
            .filter(|(_, (id, _))| models.iter().any(|model| model == id))
            .fold(0, |slots, (slot, _)| slots | 1 << slot);
        Self {
            uses: u32::try_from(uses.clamp(0, i64::from(MAX_USES))).unwrap_or(0),
            slots,
        }
    }

    /// The number the update check sends as `u`.
    pub fn number(&self) -> u32 {
        self.uses.min(MAX_USES) | u32::from(self.slots) << 10 | VERSION << 26
    }

    /// Reads a number back, as the update API does. `None` for another format version.
    pub fn from_number(number: u32) -> Option<Self> {
        (number >> 26 == VERSION).then_some(Self {
            uses: number & MAX_USES,
            slots: (number >> 10) as u16,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_round_trips_caps_and_stays_out_while_off() {
        let models = ["nemotron-labs-unified-asr-0.6b-q8_0", "kokoro", "base"].map(String::from);
        let usage = Usage::new(12, &models);
        assert_eq!(usage.number(), 12 | (1 | 1 << 8) << 10 | 1 << 26);
        assert_eq!(Usage::from_number(usage.number()), Some(usage));

        let busy = Usage::new(5000, &[]);
        assert_eq!(Usage::from_number(busy.number()).unwrap().uses, MAX_USES);
        assert_eq!(Usage::from_number(0), None);

        let version = crate::update::Version::parse("2.5.0").unwrap();
        let mut count = crate::update::Count::new(0, "", "");
        let off = count.query(&version, crate::os::Os::Mac);
        assert!(!off.contains("u="));
        count.usage = Some(usage);
        let on = count.query(&version, crate::os::Os::Mac);
        assert_eq!(on, format!("{off}&u={}", usage.number()));
    }
}
