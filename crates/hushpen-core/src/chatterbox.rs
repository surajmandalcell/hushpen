//! The Chatterbox voice models, Turbo and Nano: the files of their catalog entries, the clip a
//! voice is cloned from, and the English check of the text they read.
//!
//! Both models clone the voice of a short clip and read English only. A model folder ships one
//! clip, the voice used when the user has cloned none.

use std::path::{Path, PathBuf};

pub const TURBO_ID: &str = "chatterbox-turbo";
pub const NANO_ID: &str = "chatterbox-nano";
/// The catalog ids, also their folders in `models/tts/`.
pub const MODEL_IDS: [&str; 2] = [TURBO_ID, NANO_ID];

pub const TOKENIZER_FILE: &str = "tokenizer.json";
pub const SPEECH_ENCODER_FILE: &str = "speech_encoder_q4.onnx";
pub const EMBED_TOKENS_FILE: &str = "embed_tokens_q4.onnx";
pub const LANGUAGE_MODEL_FILE: &str = "language_model_q4.onnx";
pub const DECODER_FILE: &str = "conditional_decoder_q4.onnx";
/// The clip of the voice a model reads with when no other voice is chosen.
pub const DEFAULT_VOICE_FILE: &str = "default_voice.wav";
/// The voice id of that clip, beside the ids of the voices the user cloned.
pub const BUILT_IN_VOICE: &str = "built-in";
/// The four graphs. ONNX Runtime reads each one's weights from `<graph>_data` beside it.
pub const GRAPHS: [&str; 4] = [
    SPEECH_ENCODER_FILE,
    EMBED_TOKENS_FILE,
    LANGUAGE_MODEL_FILE,
    DECODER_FILE,
];
/// Every file the worker opens, in the order the catalog lists them: the largest graph first,
/// since the catalog reads the quantization from the first file.
pub const FILES: [&str; 10] = [
    LANGUAGE_MODEL_FILE,
    "language_model_q4.onnx_data",
    EMBED_TOKENS_FILE,
    "embed_tokens_q4.onnx_data",
    SPEECH_ENCODER_FILE,
    "speech_encoder_q4.onnx_data",
    DECODER_FILE,
    "conditional_decoder_q4.onnx_data",
    TOKENIZER_FILE,
    DEFAULT_VOICE_FILE,
];

/// The rate the models speak at, and the rate of a saved voice clip.
pub const SAMPLE_RATE: u32 = 24_000;
/// The shortest and longest clip a voice is cloned from. Under 5 seconds the voice comes out
/// unlike the speaker; the models hear at most 10 seconds of a clip.
pub const MIN_CLIP_SECS: f32 = 5.0;
pub const MAX_CLIP_SECS: f32 = 20.0;
/// The part of a clip the models hear.
pub const HEARD_CLIP_SECS: f32 = 10.0;

pub fn is_model(id: &str) -> bool {
    MODEL_IDS.contains(&id)
}

/// Where the worker keeps what `model` computed from `clip`, so it runs the speech encoder once
/// for each voice: `voices/a1/reference.wav` gives `voices/a1/reference.chatterbox-turbo.cond`.
pub fn conditioning_path(clip: &Path, model: &str) -> PathBuf {
    let stem = clip
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    clip.with_file_name(format!("{stem}.{model}.cond"))
}

/// Words that are common in English text and rare in the other languages of Latin script.
const ENGLISH_WORDS: [&str; 32] = [
    "the", "and", "of", "to", "is", "that", "it", "you", "was", "for", "on", "are", "with", "as",
    "his", "they", "be", "at", "this", "have", "from", "or", "by", "not", "what", "we", "can",
    "will", "there", "which", "would", "their",
];
/// Words that are common in German, French, Spanish, Italian, Portuguese, or Dutch text and rare
/// in English text.
const OTHER_WORDS: [&str; 60] = [
    "der", "die", "das", "und", "ist", "nicht", "ein", "eine", "ich", "mit", "auf", "den", "dem",
    "zu", "sie", "le", "la", "les", "et", "est", "une", "des", "du", "que", "qui", "pas", "pour",
    "dans", "je", "vous", "avec", "el", "los", "las", "y", "una", "del", "por", "con", "para",
    "es", "se", "lo", "il", "che", "di", "non", "sono", "della", "per", "um", "uma", "com", "não",
    "het", "een", "van", "ik", "niet", "ook",
];

/// Whether `text` reads as English: its letters are Latin, and its common words are English
/// words more than the words of another Latin-script language. Text without letters, such as
/// a number, counts as English.
pub fn reads_as_english(text: &str) -> bool {
    let (mut latin, mut other) = (0_usize, 0_usize);
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        if is_latin(c) {
            latin += 1;
        } else {
            other += 1;
        }
    }
    if latin + other == 0 {
        return true;
    }
    if other * 5 > latin {
        return false;
    }
    let (mut english, mut foreign) = (0_usize, 0_usize);
    for word in text
        .split(|c: char| !c.is_alphabetic())
        .filter(|word| !word.is_empty())
    {
        let word = word.to_lowercase();
        if ENGLISH_WORDS.contains(&word.as_str()) {
            english += 1;
        } else if OTHER_WORDS.contains(&word.as_str()) {
            foreign += 1;
        }
    }
    foreign < 2 || foreign <= english * 2
}

fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic()
        || ('\u{00c0}'..='\u{024f}').contains(&c)
        || ('\u{1e00}'..='\u{1eff}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_graph_has_its_weights_in_the_file_list() {
        for graph in GRAPHS {
            assert!(FILES.contains(&graph), "{graph}");
            let weights = format!("{graph}_data");
            assert!(FILES.contains(&weights.as_str()), "{weights}");
        }
        assert!(FILES.contains(&TOKENIZER_FILE) && FILES.contains(&DEFAULT_VOICE_FILE));
        assert!(is_model(TURBO_ID) && is_model(NANO_ID) && !is_model("kokoro-82m-int8"));
    }

    #[test]
    fn the_conditioning_sits_beside_its_clip_for_each_model() {
        let clip = Path::new("/d/voices/a1/reference.wav");
        assert_eq!(
            conditioning_path(clip, TURBO_ID),
            Path::new("/d/voices/a1/reference.chatterbox-turbo.cond")
        );
        assert_eq!(
            conditioning_path(Path::new("/m/default_voice.wav"), NANO_ID),
            Path::new("/m/default_voice.chatterbox-nano.cond")
        );
    }

    #[test]
    fn english_text_reads_as_english_and_other_languages_do_not() {
        for text in [
            "The meeting starts at nine. Please bring the report.",
            "Hello!",
            "It costs $45.99, and the café opens at 3:30.",
            "1, 2, 3.",
            "Naïve résumé",
            "Paris is the capital of France, and Berlin is the capital of Germany.",
        ] {
            assert!(reads_as_english(text), "{text}");
        }
        for text in [
            "東京は日本の首都です。",
            "Привет, как дела?",
            "Das ist nicht der Weg, und ich weiß es.",
            "Je ne sais pas ce que vous voulez dans la vie.",
            "El perro y el gato están en la casa con los niños.",
            "Hello 世界のみなさん、こんにちは",
        ] {
            assert!(!reads_as_english(text), "{text}");
        }
    }
}
