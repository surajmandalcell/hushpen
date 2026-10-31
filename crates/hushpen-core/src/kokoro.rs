//! The Kokoro voice model: the files of its catalog entry and the voices it speaks.
//!
//! sherpa-onnx 2.0 has no espeak-ng, so the worker turns text into phonemes itself with the US
//! English lexicon the entry ships, and only the English voices are offered. The British voices
//! read with the same lexicon.
//!
//! The v1.0 weights include the 2026-10-26 int8 fix: the two STFT convolutions stay in
//! float32, so their phase calculation cannot divide quantized zero by zero.

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

/// The speakers in `voices.bin`, as the model's `n_speakers` says. The library refuses a voices
/// file of any other size.
pub const SPEAKERS: usize = 54;
/// The style vectors of one speaker: one row for each phoneme count up to 510, each of 256
/// floats (the model's `style_dim` is `510,1,256`). A file holds the speakers one after the
/// other, in little-endian `f32`.
pub const STYLE_ROWS: usize = 510;
pub const STYLE_WIDTH: usize = 256;
/// The bytes of one speaker in `voices.bin`.
pub const SPEAKER_BYTES: usize = STYLE_ROWS * STYLE_WIDTH * 4;
/// The bytes of `voices.bin`.
pub const VOICES_BYTES: usize = SPEAKERS * SPEAKER_BYTES;
/// The speaker a mix file holds the blend in: `em_santa`, a Spanish voice the lexicon cannot
/// read, so every offered voice stays in the file as it is.
pub const MIX_SPEAKER: i32 = 53;
/// The folder in `models/tts/` that keeps the voices files of mixes.
pub const MIX_DIR: &str = "mixes";
/// The share of the second voice a mix may take, in percent.
pub const MIX_PERCENTS: std::ops::RangeInclusive<u8> = 1..=99;
pub const DEFAULT_MIX_PERCENT: u8 = 50;

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
/// (speakers 28 to 53 speak languages the lexicon does not cover).
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

/// Two voices blended into one: the style vectors of `base` and `with`, weighted by
/// `100 - percent` and `percent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceMix {
    pub base: &'static KokoroVoice,
    pub with: &'static KokoroVoice,
    /// The share of `with`, in [`MIX_PERCENTS`].
    pub percent: u8,
}

impl VoiceMix {
    /// `None` for an unknown voice, a voice mixed with itself, or a share out of range.
    pub fn new(base: &str, with: &str, percent: u8) -> Option<Self> {
        let (base, with) = (find_voice(base)?, find_voice(with)?);
        (base != with && MIX_PERCENTS.contains(&percent)).then_some(Self {
            base,
            with,
            percent,
        })
    }

    /// Reads the voice id of a mix, such as `af_heart:70+am_adam:30`. The shares add up to 100.
    pub fn parse(id: &str) -> Option<Self> {
        let (base, with) = id.split_once('+')?;
        let share = |part: &str| {
            let (voice, percent) = part.split_once(':')?;
            Some((voice.to_owned(), percent.parse::<u8>().ok()?))
        };
        let ((base, base_percent), (with, percent)) = (share(base)?, share(with)?);
        if u16::from(base_percent) + u16::from(percent) != 100 {
            return None;
        }
        Self::new(&base, &with, percent)
    }

    /// The voice id that `Speak` takes for this mix.
    pub fn id(&self) -> String {
        format!(
            "{}:{}+{}:{}",
            self.base.id,
            100 - self.percent,
            self.with.id,
            self.percent
        )
    }

    /// The weights of `base` and `with`. They add up to 1.
    pub fn weights(&self) -> (f32, f32) {
        let with = f32::from(self.percent) / 100.0;
        (1.0 - with, with)
    }

    /// The name of the voices file in [`MIX_DIR`]: one file for each pair and share.
    pub fn file_name(&self) -> String {
        format!("{}+{}-{}.bin", self.base.id, self.with.id, self.percent)
    }
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

    #[test]
    fn the_voices_file_layout_matches_the_shipped_file() {
        // voices.bin of kokoro-int8-multi-lang-v1_0, as the catalog pins it.
        assert_eq!(VOICES_BYTES, 28_200_960);
        assert!(usize::try_from(MIX_SPEAKER).unwrap() < SPEAKERS);
        assert!(VOICES.iter().all(|voice| voice.speaker != MIX_SPEAKER));
    }

    #[test]
    fn a_mix_id_round_trips_and_its_weights_add_up_to_one() {
        let mix = VoiceMix::new("af_heart", "am_adam", 30).unwrap();
        assert_eq!(mix.id(), "af_heart:70+am_adam:30");
        assert_eq!(VoiceMix::parse(&mix.id()), Some(mix));
        for percent in MIX_PERCENTS {
            let (base, with) = VoiceMix::new("af_bella", "bf_emma", percent)
                .unwrap()
                .weights();
            assert!((base + with - 1.0).abs() < 1e-6, "{percent}");
            assert!(base > 0.0 && with > 0.0, "{percent}");
        }
        assert_eq!(mix.weights(), (0.7, 0.3));
    }

    #[test]
    fn a_malformed_mix_id_is_refused() {
        for id in [
            "af_heart",
            "af_heart:50+af_heart:50",
            "af_heart:60+am_adam:50",
            "af_heart:100+am_adam:0",
            "af_heart:0+am_adam:100",
            "af_heart:50+zf_xiaobei:50",
            "af_heart:50+am_adam",
            "af_heart:-5+am_adam:105",
            "af_heart:50+am_adam:50+bf_emma:0",
        ] {
            assert_eq!(VoiceMix::parse(id), None, "{id}");
        }
    }

    #[test]
    fn each_pair_and_share_has_its_own_cache_file() {
        let name = |base, with, percent| VoiceMix::new(base, with, percent).unwrap().file_name();
        assert_eq!(name("af_heart", "am_adam", 50), "af_heart+am_adam-50.bin");
        assert_eq!(
            name("af_heart", "am_adam", 50),
            name("af_heart", "am_adam", 50)
        );
        assert_ne!(
            name("af_heart", "am_adam", 50),
            name("am_adam", "af_heart", 50)
        );
        assert_ne!(
            name("af_heart", "am_adam", 50),
            name("af_heart", "am_adam", 60)
        );
    }
}
