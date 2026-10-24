//! The Kokoro voice model: the files of its catalog entry and the voices it speaks.
//!
//! sherpa-onnx 2.0 has no espeak-ng, so the worker turns text into phonemes itself with the US
//! English lexicon the entry ships, and only the English voices are offered. The British voices
//! read with the same lexicon.

/// The catalog id of the model, also its folder in `models/tts/`.
pub const MODEL_ID: &str = "kokoro-82m-int8";
pub const MODEL_FILE: &str = "model.int8.onnx";
pub const VOICES_FILE: &str = "voices.bin";
pub const TOKENS_FILE: &str = "tokens.txt";
pub const LEXICON_FILE: &str = "lexicon-us-en.txt";
/// Every file the worker opens, in the order the catalog lists them.
pub const FILES: [&str; 4] = [MODEL_FILE, VOICES_FILE, TOKENS_FILE, LEXICON_FILE];
/// The rate Kokoro speaks at.
pub const SAMPLE_RATE: u32 = 24_000;
pub const DEFAULT_VOICE: &str = "af_heart";

/// One voice: its id in the model's speaker table and its speaker number there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KokoroVoice {
    /// `af_heart`: `a` is US and `b` British English, `f` or `m` the voice, then its name.
    pub id: &'static str,
    pub speaker: i32,
}

impl KokoroVoice {
    /// `af_heart` is `Heart`.
    pub fn name(&self) -> String {
        let base = self.id.get(3..).unwrap_or(self.id);
        let mut chars = base.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_uppercase().chain(chars).collect()
        })
    }

    pub fn language(&self) -> &'static str {
        if self.id.starts_with('b') {
            "en-GB"
        } else {
            "en-US"
        }
    }
}

const fn voice(id: &'static str, speaker: i32) -> KokoroVoice {
    KokoroVoice { id, speaker }
}

/// The English voices of `kokoro-int8-multi-lang-v1_0`, numbered as in its speaker table
/// (speakers 28 to 52 speak languages the lexicon does not cover).
pub const VOICES: [KokoroVoice; 28] = [
    voice("af_alloy", 0),
    voice("af_aoede", 1),
    voice("af_bella", 2),
    voice("af_heart", 3),
    voice("af_jessica", 4),
    voice("af_kore", 5),
    voice("af_nicole", 6),
    voice("af_nova", 7),
    voice("af_river", 8),
    voice("af_sarah", 9),
    voice("af_sky", 10),
    voice("am_adam", 11),
    voice("am_echo", 12),
    voice("am_eric", 13),
    voice("am_fenrir", 14),
    voice("am_liam", 15),
    voice("am_michael", 16),
    voice("am_onyx", 17),
    voice("am_puck", 18),
    voice("am_santa", 19),
    voice("bf_alice", 20),
    voice("bf_emma", 21),
    voice("bf_isabella", 22),
    voice("bf_lily", 23),
    voice("bm_daniel", 24),
    voice("bm_fable", 25),
    voice("bm_george", 26),
    voice("bm_lewis", 27),
];

pub fn find_voice(id: &str) -> Option<&'static KokoroVoice> {
    VOICES.iter().find(|voice| voice.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_speaker_numbers_follow_the_table_order() {
        for (index, voice) in VOICES.iter().enumerate() {
            assert_eq!(
                usize::try_from(voice.speaker).ok(),
                Some(index),
                "{}",
                voice.id
            );
        }
    }

    #[test]
    fn names_and_languages_come_from_the_id() {
        let heart = find_voice(DEFAULT_VOICE).unwrap();
        assert_eq!(
            (heart.name(), heart.language()),
            ("Heart".to_owned(), "en-US")
        );
        let george = find_voice("bm_george").unwrap();
        assert_eq!(
            (george.name(), george.language()),
            ("George".to_owned(), "en-GB")
        );
        assert!(find_voice("zf_xiaobei").is_none());
    }
}
