//! The C API of sherpa-onnx, opened with `dlopen` at run time.
//!
//! The worker does not link the library: the workspace builds and tests without it, and the
//! package ships one pinned, checksummed copy (`cargo xtask tts-runtime`). The structs below
//! copy `c-api.h` of sherpa-onnx 2.0.0 field for field. [`Api::open`] refuses any other version,
//! because a changed layout would corrupt memory without an error.

use libloading::Library;
use std::ffi::{CStr, CString, c_char, c_float, c_void};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;

/// The only sherpa-onnx version whose struct layout this module knows.
pub const VERSION: &str = "2.0.0";

type Text = *const c_char;

// Model families other than Kokoro stay zeroed; they are here for the layout only.
#[allow(dead_code)]
#[repr(C)]
struct VitsConfig {
    model: Text,
    lexicon: Text,
    tokens: Text,
    data_dir: Text,
    noise_scale: c_float,
    noise_scale_w: c_float,
    length_scale: c_float,
    dict_dir: Text,
}

#[allow(dead_code)]
#[repr(C)]
struct MatchaConfig {
    acoustic_model: Text,
    vocoder: Text,
    lexicon: Text,
    tokens: Text,
    data_dir: Text,
    noise_scale: c_float,
    length_scale: c_float,
    dict_dir: Text,
}

#[allow(dead_code)]
#[repr(C)]
struct KokoroConfig {
    model: Text,
    voices: Text,
    tokens: Text,
    data_dir: Text,
    length_scale: c_float,
    dict_dir: Text,
    lexicon: Text,
    lang: Text,
}

#[allow(dead_code)]
#[repr(C)]
struct KittenConfig {
    model: Text,
    voices: Text,
    tokens: Text,
    data_dir: Text,
    length_scale: c_float,
    lexicon: Text,
}

#[allow(dead_code)]
#[repr(C)]
struct ZipvoiceConfig {
    tokens: Text,
    encoder: Text,
    decoder: Text,
    vocoder: Text,
    data_dir: Text,
    lexicon: Text,
    feat_scale: c_float,
    t_shift: c_float,
    target_rms: c_float,
    guidance_scale: c_float,
}

#[allow(dead_code)]
#[repr(C)]
struct PocketConfig {
    lm_flow: Text,
    lm_main: Text,
    encoder: Text,
    decoder: Text,
    text_conditioner: Text,
    vocab_json: Text,
    token_scores_json: Text,
    voice_embedding_cache_capacity: i32,
}

#[allow(dead_code)]
#[repr(C)]
struct SupertonicConfig {
    duration_predictor: Text,
    text_encoder: Text,
    vector_estimator: Text,
    vocoder: Text,
    tts_json: Text,
    unicode_indexer: Text,
    voice_style: Text,
}

#[allow(dead_code)]
#[repr(C)]
struct ModelConfig {
    vits: VitsConfig,
    num_threads: i32,
    debug: i32,
    provider: Text,
    matcha: MatchaConfig,
    kokoro: KokoroConfig,
    kitten: KittenConfig,
    zipvoice: ZipvoiceConfig,
    pocket: PocketConfig,
    supertonic: SupertonicConfig,
}

#[allow(dead_code)]
#[repr(C)]
struct TtsConfig {
    model: ModelConfig,
    rule_fsts: Text,
    max_num_sentences: i32,
    rule_fars: Text,
    silence_scale: c_float,
}

#[repr(C)]
struct GeneratedAudio {
    samples: *const f32,
    n: i32,
    sample_rate: i32,
}

#[allow(dead_code)]
#[repr(C)]
struct GenerationConfig {
    silence_scale: c_float,
    speed: c_float,
    sid: i32,
    reference_audio: *const f32,
    reference_audio_len: i32,
    reference_sample_rate: i32,
    reference_text: Text,
    num_steps: i32,
    extra: Text,
    phoneme_codepoints: *const i32,
    phoneme_codepoints_lens: *const i32,
    phoneme_codepoints_num_sentences: i32,
    tokens: *const Text,
    tokens_lens: *const i32,
    tokens_num_sentences: i32,
}

type ProgressFn = unsafe extern "C" fn(*const f32, i32, c_float, *mut c_void) -> i32;
type VersionFn = unsafe extern "C" fn() -> Text;
type CreateFn = unsafe extern "C" fn(*const TtsConfig) -> *const c_void;
type DestroyFn = unsafe extern "C" fn(*const c_void);
type CountFn = unsafe extern "C" fn(*const c_void) -> i32;
type GenerateFn = unsafe extern "C" fn(
    *const c_void,
    Text,
    *const GenerationConfig,
    Option<ProgressFn>,
    *mut c_void,
) -> *const GeneratedAudio;
type DestroyAudioFn = unsafe extern "C" fn(*const GeneratedAudio);

/// The functions the worker calls. The library stays open as long as any copy of them.
pub struct Api {
    create: CreateFn,
    destroy: DestroyFn,
    sample_rate: CountFn,
    num_speakers: CountFn,
    generate: GenerateFn,
    destroy_audio: DestroyAudioFn,
    version: String,
    _library: Library,
}

impl Api {
    pub fn open(path: &Path) -> Result<Self, String> {
        // SAFETY: opening the library runs its initializers. The package ships this library
        // from a pinned archive whose checksum it verified.
        let library = unsafe { Library::new(path) }
            .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
        let symbol_error = |name: &str, error: libloading::Error| {
            format!("{} has no {name}: {error}", path.display())
        };
        // SAFETY: every symbol is read with the signature that c-api.h of sherpa-onnx 2.0.0
        // gives it, and the version check below runs before any other call. The pointers stay
        // valid because `Api` keeps the library open.
        unsafe {
            let version = *library
                .get::<VersionFn>(b"SherpaOnnxGetVersionStr\0")
                .map_err(|error| symbol_error("SherpaOnnxGetVersionStr", error))?;
            let version = CStr::from_ptr(version()).to_string_lossy().into_owned();
            if version != VERSION {
                return Err(format!(
                    "{} is sherpa-onnx {version}, this worker needs {VERSION}",
                    path.display()
                ));
            }
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(|error| symbol_error($name, error))?
                };
            }
            Ok(Self {
                create: symbol!("SherpaOnnxCreateOfflineTts"),
                destroy: symbol!("SherpaOnnxDestroyOfflineTts"),
                sample_rate: symbol!("SherpaOnnxOfflineTtsSampleRate"),
                num_speakers: symbol!("SherpaOnnxOfflineTtsNumSpeakers"),
                generate: symbol!("SherpaOnnxOfflineTtsGenerateWithConfig"),
                destroy_audio: symbol!("SherpaOnnxDestroyOfflineTtsGeneratedAudio"),
                version,
                _library: library,
            })
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

/// The files sherpa-onnx reads of one Kokoro model. It gets no lexicon: the worker turns text
/// into phonemes itself (see `crate::phonemes`).
pub struct KokoroFiles<'a> {
    pub model: &'a Path,
    pub voices: &'a Path,
    pub tokens: &'a Path,
}

/// A loaded Kokoro model.
pub struct Tts {
    api: Arc<Api>,
    raw: NonNull<c_void>,
}

// SAFETY: one thread at a time uses a `Tts` (the worker thread owns it), and sherpa-onnx keeps
// no state of it in thread-local storage.
unsafe impl Send for Tts {}

fn c_text(text: &str) -> Result<CString, String> {
    CString::new(text).map_err(|_| "a text holds a NUL byte".to_owned())
}

fn c_path(path: &Path) -> Result<CString, String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("{} is not a UTF-8 path", path.display()))?;
    c_text(text)
}

impl Tts {
    /// Loads the model to run on the CPU.
    pub fn kokoro(api: Arc<Api>, files: &KokoroFiles<'_>, threads: i32) -> Result<Self, String> {
        let model = c_path(files.model)?;
        let voices = c_path(files.voices)?;
        let tokens = c_path(files.tokens)?;
        // SAFETY: every field of the config is a pointer, an integer, or a float, so all zero
        // bytes are a valid value: null paths and the library's defaults.
        let mut config: TtsConfig = unsafe { std::mem::zeroed() };
        config.model.kokoro.model = model.as_ptr();
        config.model.kokoro.voices = voices.as_ptr();
        config.model.kokoro.tokens = tokens.as_ptr();
        config.model.kokoro.length_scale = 1.0;
        config.model.num_threads = threads.max(1);
        // Core ML stops the process on this int8 model: ONNX Runtime throws a C++ exception
        // (a dynamic shape it does not support), which no Rust code can catch.
        config.model.provider = c"cpu".as_ptr();
        // One sentence per callback, so the first audio comes out after the first sentence.
        config.max_num_sentences = 1;
        config.silence_scale = 0.2;
        // SAFETY: the config and the strings it points to live until the call returns; the
        // library copies what it keeps.
        let raw = unsafe { (api.create)(&config) };
        NonNull::new(raw.cast_mut())
            .map(|raw| Self { api, raw })
            .ok_or_else(|| "sherpa-onnx could not load the model".to_owned())
    }

    pub fn sample_rate(&self) -> u32 {
        // SAFETY: `raw` is a live handle from SherpaOnnxCreateOfflineTts.
        let rate = unsafe { (self.api.sample_rate)(self.raw.as_ptr()) };
        u32::try_from(rate).unwrap_or(0)
    }

    pub fn speakers(&self) -> i32 {
        // SAFETY: `raw` is a live handle from SherpaOnnxCreateOfflineTts.
        unsafe { (self.api.num_speakers)(self.raw.as_ptr()) }
    }

    /// Speaks `sentences` of phonemes, one model run each. `sink` gets the audio of each
    /// sentence and returns `false` to stop before the next one.
    pub fn generate(
        &mut self,
        sentences: &[String],
        speaker: i32,
        speed: f32,
        sink: &mut dyn FnMut(&[f32]) -> bool,
    ) -> Result<(), String> {
        let mut codepoints: Vec<i32> = Vec::new();
        let mut lens: Vec<i32> = Vec::new();
        for sentence in sentences {
            let before = codepoints.len();
            // A char is at most U+10FFFF, so it fits an i32.
            codepoints.extend(sentence.chars().map(|c| u32::from(c) as i32));
            let len = codepoints.len() - before;
            // The library refuses a sentence of no phonemes.
            if len > 0 {
                lens.push(i32::try_from(len).map_err(|_| "a sentence is too long".to_owned())?);
            }
        }
        if lens.is_empty() {
            return Ok(());
        }
        let count = i32::try_from(lens.len()).map_err(|_| "the text is too long".to_owned())?;
        // SAFETY: as in `kokoro`, all zero bytes are a valid config.
        let mut config: GenerationConfig = unsafe { std::mem::zeroed() };
        config.silence_scale = 0.2;
        config.speed = speed;
        config.sid = speaker;
        config.extra = c"{}".as_ptr();
        config.phoneme_codepoints = codepoints.as_ptr();
        config.phoneme_codepoints_lens = lens.as_ptr();
        config.phoneme_codepoints_num_sentences = count;
        let mut sink: &mut dyn FnMut(&[f32]) -> bool = sink;
        let arg: *mut &mut dyn FnMut(&[f32]) -> bool = &mut sink;
        // SAFETY: `raw` is a live handle, the config and the phonemes it points to outlive the
        // call, and `arg` points to `sink`, which outlives the call; `on_progress` reads it
        // back as that type.
        let audio = unsafe {
            (self.api.generate)(
                self.raw.as_ptr(),
                c"".as_ptr(),
                &config,
                Some(on_progress),
                arg.cast(),
            )
        };
        if audio.is_null() {
            return Err("sherpa-onnx wrote no audio".into());
        }
        // SAFETY: `audio` came from SherpaOnnxOfflineTtsGenerateWithConfig. Its samples were
        // already given to `sink`, so it is freed unread.
        unsafe { (self.api.destroy_audio)(audio) };
        Ok(())
    }
}

impl Drop for Tts {
    fn drop(&mut self) {
        // SAFETY: `raw` is a live handle and is not used again.
        unsafe { (self.api.destroy)(self.raw.as_ptr()) };
    }
}

/// Hands one new piece of audio to the sink of `Tts::generate`. Returns 1 to go on, 0 to stop.
unsafe extern "C" fn on_progress(
    samples: *const f32,
    count: i32,
    _progress: c_float,
    arg: *mut c_void,
) -> i32 {
    // SAFETY: `arg` is the sink pointer that `Tts::generate` passed, alive for the whole call.
    let sink = unsafe { &mut *arg.cast::<&mut dyn FnMut(&[f32]) -> bool>() };
    let samples = match usize::try_from(count) {
        // SAFETY: the library passes `count` valid samples that live during the callback.
        Ok(count) if count > 0 && !samples.is_null() => unsafe {
            std::slice::from_raw_parts(samples, count)
        },
        _ => &[],
    };
    i32::from(sink(samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_library_is_an_error_not_a_crash() {
        let problem = Api::open(Path::new("/nonexistent/libsherpa-onnx-c-api.so"))
            .err()
            .unwrap();
        assert!(problem.contains("cannot open"), "{problem}");
    }

    #[test]
    fn a_nul_byte_in_a_text_is_refused() {
        assert!(c_text("a\0b").is_err());
        assert!(c_path(Path::new("/m/model.onnx")).is_ok());
    }
}
