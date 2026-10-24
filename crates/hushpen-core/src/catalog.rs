//! The model catalog: `assets/models.json`, embedded at build time.
//!
//! The file pins every downloadable model by size and sha256. The app trusts a model file only
//! when it matches its entry here.

use crate::shortcut::Platform;
use serde_json::Value;
use std::sync::OnceLock;

const EMBEDDED: &str = include_str!("../../../assets/models.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Languages {
    Multilingual,
    English,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    /// Short id, also the value of `dictation.modelId`: `tiny.en`, `base`.
    pub id: String,
    pub name: String,
    /// File name in `models/whisper/`.
    pub file: String,
    pub bytes: u64,
    /// Lower-case hex.
    pub sha256: String,
    pub url: String,
    pub license: String,
    pub languages: Languages,
    /// The model the app proposes first.
    pub default: bool,
}

/// One file of a model that is a folder of files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFile {
    pub file: String,
    pub bytes: u64,
    /// Lower-case hex.
    pub sha256: String,
    pub url: String,
}

/// A voice model for Read aloud: the folder `models/tts/<id>/` with every file of `files`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceEntry {
    pub id: String,
    pub name: String,
    pub license: String,
    pub languages: Languages,
    pub default: bool,
    pub files: Vec<ModelFile>,
}

impl VoiceEntry {
    /// The size of the whole folder.
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub whisper: Vec<ModelEntry>,
    /// The AI cleanup models. The file name is `<id>.gguf`, which is where the worker looks.
    pub llm: Vec<ModelEntry>,
    pub voice: Vec<VoiceEntry>,
}

/// The cleanup model for Macs with enough memory to hold it.
pub const LLM_LARGE: &str = "gemma-4-e2b";
/// The cleanup model for every other machine.
pub const LLM_SMALL: &str = "qwen3.5-2b";
/// Memory a Mac needs before it is offered [`LLM_LARGE`].
pub const LLM_LARGE_MIN_RAM_BYTES: u64 = 16 << 30;

impl Catalog {
    pub fn parse(json: &str) -> Result<Self, String> {
        let root: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
        let whisper = parse_list(&root, "whisper", true)?;
        if whisper.iter().filter(|entry| entry.default).count() > 1 {
            return Err("more than one default model".into());
        }
        let llm = parse_list(&root, "llm", false)?;
        if let Some(entry) = llm
            .iter()
            .find(|entry| whisper.iter().any(|w| w.id == entry.id))
        {
            return Err(format!("duplicate model id `{}`", entry.id));
        }
        if let Some(entry) = llm
            .iter()
            .find(|entry| entry.file != format!("{}.gguf", entry.id))
        {
            return Err(format!(
                "model `{}` must be stored as `<id>.gguf`",
                entry.id
            ));
        }
        let voice = parse_voices(&root)?;
        if let Some(entry) = voice
            .iter()
            .find(|entry| whisper.iter().chain(&llm).any(|other| other.id == entry.id))
        {
            return Err(format!("duplicate model id `{}`", entry.id));
        }
        Ok(Self {
            whisper,
            llm,
            voice,
        })
    }

    pub fn whisper(&self, id: &str) -> Option<&ModelEntry> {
        self.whisper.iter().find(|entry| entry.id == id)
    }

    pub fn default_whisper(&self) -> Option<&ModelEntry> {
        self.whisper.iter().find(|entry| entry.default)
    }

    pub fn llm(&self, id: &str) -> Option<&ModelEntry> {
        self.llm.iter().find(|entry| entry.id == id)
    }

    pub fn voice(&self, id: &str) -> Option<&VoiceEntry> {
        self.voice.iter().find(|entry| entry.id == id)
    }

    /// The AI cleanup models this machine can run, best first. A Mac with 16 GB or more gets
    /// [`LLM_LARGE`] ahead of [`LLM_SMALL`]; every other machine gets [`LLM_SMALL`] only. No
    /// model under 1B parameters is ever listed.
    pub fn offered_llm(&self, platform: Platform, ram_bytes: u64) -> Vec<&ModelEntry> {
        let large = platform == Platform::MacOs && ram_bytes >= LLM_LARGE_MIN_RAM_BYTES;
        [(large, LLM_LARGE), (true, LLM_SMALL)]
            .into_iter()
            .filter(|(offered, _)| *offered)
            .filter_map(|(_, id)| self.llm(id))
            .collect()
    }

    /// The model Settings proposes first.
    pub fn suggested_llm(&self, platform: Platform, ram_bytes: u64) -> Option<&ModelEntry> {
        self.offered_llm(platform, ram_bytes).into_iter().next()
    }
}

/// The catalog that ships in the app. A broken embedded file would be a build defect that the
/// unit tests catch; at run time it yields an empty list rather than a crash.
pub fn embedded() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        Catalog::parse(EMBEDDED).unwrap_or(Catalog {
            whisper: Vec::new(),
            llm: Vec::new(),
            voice: Vec::new(),
        })
    })
}

fn parse_list(root: &Value, key: &str, required: bool) -> Result<Vec<ModelEntry>, String> {
    let Some(list) = root.get(key).and_then(Value::as_array) else {
        return if required {
            Err(format!("the catalog has no `{key}` list"))
        } else {
            Ok(Vec::new())
        };
    };
    let mut entries: Vec<ModelEntry> = Vec::with_capacity(list.len());
    for item in list {
        let entry = parse_entry(item)?;
        if entries.iter().any(|seen| seen.id == entry.id) {
            return Err(format!("duplicate model id `{}`", entry.id));
        }
        entries.push(entry);
    }
    Ok(entries)
}

fn text(item: &Value, key: &str) -> Result<String, String> {
    item.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("model entry needs a text `{key}`"))
}

fn parse_entry(item: &Value) -> Result<ModelEntry, String> {
    let id = text(item, "id")?;
    let ModelFile {
        file,
        bytes,
        sha256,
        url,
    } = parse_file(item, &id)?;
    Ok(ModelEntry {
        name: text(item, "name")?,
        file,
        bytes,
        sha256,
        url,
        license: text(item, "license")?,
        languages: parse_languages(item, &id)?,
        default: is_default(item),
        id,
    })
}

/// The `voice` list. It is optional, so a catalog from before voice models still parses.
fn parse_voices(root: &Value) -> Result<Vec<VoiceEntry>, String> {
    let Some(list) = root.get("voice").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut entries: Vec<VoiceEntry> = Vec::with_capacity(list.len());
    for item in list {
        let id = text(item, "id")?;
        if id.contains(['/', '\\']) || id.starts_with('.') {
            return Err(format!("voice model `{id}` has an unsafe folder name"));
        }
        if text(item, "kind")? != "voice" {
            return Err(format!("voice model `{id}` must be of kind `voice`"));
        }
        let mut files: Vec<ModelFile> = Vec::new();
        for file in item
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let file = parse_file(file, &id)?;
            if files.iter().any(|seen| seen.file == file.file) {
                return Err(format!("voice model `{id}` lists `{}` twice", file.file));
            }
            files.push(file);
        }
        if files.is_empty() {
            return Err(format!("voice model `{id}` needs a list of files"));
        }
        if entries.iter().any(|seen| seen.id == id) {
            return Err(format!("duplicate model id `{id}`"));
        }
        entries.push(VoiceEntry {
            name: text(item, "name")?,
            license: text(item, "license")?,
            languages: parse_languages(item, &id)?,
            default: is_default(item),
            files,
            id,
        });
    }
    if entries.iter().filter(|entry| entry.default).count() > 1 {
        return Err("more than one default voice model".into());
    }
    Ok(entries)
}

/// The pinned file fields of model `id`: a safe name, a size, a sha256, and a URL.
fn parse_file(item: &Value, id: &str) -> Result<ModelFile, String> {
    let sha256 = text(item, "sha256")?;
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(format!("model `{id}` has no 64-digit lower-case sha256"));
    }
    let bytes = item
        .get("bytes")
        .and_then(Value::as_u64)
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| format!("model `{id}` needs a byte size"))?;
    let file = text(item, "file")?;
    if file.contains(['/', '\\']) || file.starts_with('.') {
        return Err(format!("model `{id}` has an unsafe file name"));
    }
    Ok(ModelFile {
        file,
        bytes,
        sha256,
        url: text(item, "url")?,
    })
}

fn parse_languages(item: &Value, id: &str) -> Result<Languages, String> {
    match text(item, "languages")?.as_str() {
        "multilingual" => Ok(Languages::Multilingual),
        "english" => Ok(Languages::English),
        other => Err(format!("model `{id}` has unknown languages `{other}`")),
    }
}

fn is_default(item: &Value) -> bool {
    item.get("default")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: [&str; 8] = [
        "tiny",
        "tiny.en",
        "base",
        "base.en",
        "small",
        "small.en",
        "medium",
        "large-v3-turbo",
    ];

    #[test]
    fn the_embedded_catalog_lists_the_eight_whisper_models_in_order() {
        let ids: Vec<_> = embedded().whisper.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, IDS);
    }

    #[test]
    fn every_entry_has_a_hash_a_size_a_language_field_and_a_huggingface_url() {
        for entry in &embedded().whisper {
            assert_eq!(entry.sha256.len(), 64, "{}", entry.id);
            assert!(entry.bytes > 50_000_000, "{}", entry.id);
            assert_eq!(entry.file, format!("ggml-{}.bin", entry.id));
            let prefix = format!("https://{}/", "huggingface.co");
            assert!(entry.url.starts_with(&prefix), "{}", entry.id);
            assert!(entry.url.ends_with(&entry.file), "{}", entry.id);
            let english = entry.id.ends_with(".en");
            assert_eq!(
                entry.languages == Languages::English,
                english,
                "{}",
                entry.id
            );
        }
    }

    #[test]
    fn the_three_local_models_match_the_known_hashes_and_sizes() {
        let expect = [
            (
                "tiny.en",
                77_704_715,
                "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
            ),
            (
                "base.en",
                147_964_211,
                "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
            ),
            (
                "base",
                147_951_465,
                "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
            ),
        ];
        for (id, bytes, sha) in expect {
            let entry = embedded().whisper(id).unwrap();
            assert_eq!((entry.bytes, entry.sha256.as_str()), (bytes, sha), "{id}");
        }
    }

    const GIB: u64 = 1 << 30;

    fn offered(platform: Platform, ram: u64) -> Vec<&'static str> {
        embedded()
            .offered_llm(platform, ram)
            .iter()
            .map(|entry| entry.id.as_str())
            .collect()
    }

    #[test]
    fn the_two_cleanup_models_are_pinned_by_size_and_hash() {
        let expect = [
            (
                LLM_LARGE,
                3_106_738_272,
                "740185b21d22ceb83a11c3aa62ad5842ef32c70f6096d756bbee85a1e4ec34b8",
            ),
            (
                LLM_SMALL,
                1_280_835_840,
                "aaf42c8b7c3cab2bf3d69c355048d4a0ee9973d48f16c731c0520ee914699223",
            ),
        ];
        assert_eq!(embedded().llm.len(), 2);
        for (id, bytes, sha) in expect {
            let entry = embedded().llm(id).unwrap();
            assert_eq!((entry.bytes, entry.sha256.as_str()), (bytes, sha), "{id}");
            assert_eq!(entry.file, format!("{id}.gguf"));
            assert!(entry.url.starts_with("https://huggingface.co/"), "{id}");
            assert_eq!(entry.license, "Apache-2.0");
        }
    }

    #[test]
    fn a_mac_with_16_gb_or_more_gets_gemma_first() {
        assert_eq!(offered(Platform::MacOs, 16 * GIB), [LLM_LARGE, LLM_SMALL]);
        assert_eq!(offered(Platform::MacOs, 64 * GIB), [LLM_LARGE, LLM_SMALL]);
        let suggested = embedded().suggested_llm(Platform::MacOs, 24 * GIB).unwrap();
        assert_eq!(suggested.id, LLM_LARGE);
    }

    #[test]
    fn a_mac_with_8_gb_gets_qwen() {
        assert_eq!(offered(Platform::MacOs, 8 * GIB), [LLM_SMALL]);
        assert_eq!(offered(Platform::MacOs, 16 * GIB - 1), [LLM_SMALL]);
    }

    #[test]
    fn linux_gets_qwen_at_any_memory_and_never_gemma() {
        for ram in [4 * GIB, 16 * GIB, 128 * GIB] {
            assert_eq!(offered(Platform::Linux, ram), [LLM_SMALL], "{ram}");
        }
        let suggested = embedded().suggested_llm(Platform::Linux, 64 * GIB).unwrap();
        assert_eq!(suggested.name, "Qwen3.5 2B");
    }

    #[test]
    fn no_model_under_1b_is_in_the_cleanup_list() {
        for entry in &embedded().llm {
            assert!(entry.bytes > 1_000_000_000, "{}", entry.id);
            assert!(!entry.id.contains("0.8b"), "{}", entry.id);
        }
    }

    #[test]
    fn exactly_one_model_is_the_default() {
        let defaults: Vec<_> = embedded().whisper.iter().filter(|e| e.default).collect();
        assert_eq!(defaults.len(), 1);
        assert_eq!(embedded().default_whisper(), Some(defaults[0]));
        // Chosen from the latency table in library/engine.md; change both together.
        assert_eq!(defaults[0].id, "base");
    }

    #[test]
    fn a_bad_hash_a_duplicate_or_a_path_in_the_file_name_is_refused() {
        let entry = |id: &str, sha: &str, file: &str| {
            format!(
                r#"{{"whisper":[{{"id":"{id}","name":"N","file":"{file}","bytes":5,"sha256":"{sha}","url":"u","license":"MIT","languages":"english"}}]}}"#
            )
        };
        let good = "a".repeat(64);
        assert!(Catalog::parse(&entry("x", &good, "ggml-x.bin")).is_ok());
        assert!(Catalog::parse(&entry("x", "abc", "ggml-x.bin")).is_err());
        assert!(Catalog::parse(&entry("x", &"A".repeat(64), "ggml-x.bin")).is_err());
        assert!(Catalog::parse(&entry("x", &good, "../ggml-x.bin")).is_err());
        let twice = format!(
            r#"{{"whisper":[{0},{0}]}}"#,
            format!(
                r#"{{"id":"x","name":"N","file":"f","bytes":5,"sha256":"{good}","url":"u","license":"MIT","languages":"english"}}"#
            )
        );
        assert!(Catalog::parse(&twice).is_err());
    }

    #[test]
    fn the_kokoro_voice_is_pinned_file_by_file_with_every_file_the_worker_opens() {
        let entry = embedded().voice(crate::kokoro::MODEL_ID).unwrap();
        let files: Vec<_> = entry.files.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(files, crate::kokoro::FILES);
        let expect = [
            (
                114_203_756,
                "4b86207ef680e394d8343bee22dfc4c512e5c707c6d9578e3f35ab09bffd6b36",
            ),
            (
                28_200_960,
                "1c5a5b983d3d50d8586d437a51f3faa2da7919ce76a013c081e65671a3447c29",
            ),
            (
                687,
                "6ebb6bb288f20f3ae8d004d3c2ca27697da27c037d75e81a60e2a6a663f95425",
            ),
            (
                5_956_885,
                "7daaab53a181be9885b853a8582bf1838186317e5dadacbcef9c426d6fa0da14",
            ),
        ];
        let repo = "https://huggingface.co/csukuangfj/kokoro-int8-multi-lang-v1_0/resolve/";
        for (file, (bytes, sha)) in entry.files.iter().zip(expect) {
            assert_eq!(
                (file.bytes, file.sha256.as_str()),
                (bytes, sha),
                "{}",
                file.file
            );
            assert!(file.url.starts_with(repo), "{}", file.url);
            assert!(
                file.url.ends_with(&format!("/{}", file.file)),
                "{}",
                file.url
            );
        }
        assert_eq!(entry.bytes(), 148_362_288);
        assert_eq!(entry.license, "Apache-2.0");
        assert_eq!(entry.languages, Languages::English);
        assert!(entry.default);
    }

    #[test]
    fn a_voice_model_needs_its_kind_its_files_and_a_safe_folder() {
        let good = "a".repeat(64);
        let voice = |id: &str, kind: &str, files: &str| {
            format!(
                r#"{{"whisper":[],"voice":[{{"id":"{id}","kind":"{kind}","name":"N","license":"MIT","languages":"english","files":[{files}]}}]}}"#
            )
        };
        let file =
            |name: &str| format!(r#"{{"file":"{name}","bytes":5,"sha256":"{good}","url":"u"}}"#);
        let parsed = Catalog::parse(&voice("v", "voice", &file("a.onnx"))).unwrap();
        assert_eq!(parsed.voice[0].bytes(), 5);
        assert!(Catalog::parse(&voice("v", "speech", &file("a.onnx"))).is_err());
        assert!(Catalog::parse(&voice("../v", "voice", &file("a.onnx"))).is_err());
        assert!(Catalog::parse(&voice("v", "voice", "")).is_err());
        assert!(Catalog::parse(&voice("v", "voice", &file("../a.onnx"))).is_err());
        let twice = format!("{},{}", file("a.onnx"), file("a.onnx"));
        assert!(Catalog::parse(&voice("v", "voice", &twice)).is_err());
        let old = Catalog::parse(r#"{"whisper":[]}"#).unwrap();
        assert!(
            old.voice.is_empty(),
            "a catalog from before voice models parses"
        );
    }
}
