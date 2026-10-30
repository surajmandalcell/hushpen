//! The model catalog: `assets/models.json`, embedded at build time.
//!
//! The file pins every downloadable model by size and sha256. The app trusts a model file only
//! when it matches its entry here. Schema 2 adds each model's kind and the details the Models view
//! shows; a schema 1 file, without them, still parses.

use crate::shortcut::Platform;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::OnceLock;

const EMBEDDED: &str = include_str!("../../../assets/models.json");

/// The first schema that requires `kind` and the details on every entry.
const DETAILS_SCHEMA: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Languages {
    Multilingual,
    English,
}

/// What a model does. The order is the order of the Models view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModelKind {
    /// A Whisper model (`models/whisper/<file>`).
    Speech,
    /// An AI cleanup model (`models/llm/<id>.gguf`).
    Cleanup,
    /// A voice model for Read aloud (`models/tts/<id>/`).
    Voice,
}

impl ModelKind {
    pub const ALL: [ModelKind; 3] = [ModelKind::Speech, ModelKind::Cleanup, ModelKind::Voice];

    /// The value of the entry's `kind` field.
    pub fn key(self) -> &'static str {
        match self {
            ModelKind::Speech => "speech",
            ModelKind::Cleanup => "cleanup",
            ModelKind::Voice => "voice",
        }
    }

    /// The catalog list that holds this kind.
    fn list(self) -> &'static str {
        match self {
            ModelKind::Speech => "whisper",
            ModelKind::Cleanup => "llm",
            ModelKind::Voice => "voice",
        }
    }
}

/// One pinned file of a model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFile {
    pub file: String,
    pub bytes: u64,
    /// Lower-case hex.
    pub sha256: String,
    pub url: String,
}

/// What the Models view tells about a model beyond its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    /// One line: what the model is.
    pub description: String,
    /// From 1, the slowest, to 5, the fastest, against the other models of its kind.
    pub speed: u8,
    /// From 1 to 5, the most accurate, against the other models of its kind.
    pub accuracy: u8,
    /// How most weights are stored, by the name whisper.cpp, llama.cpp, or ONNX gives it: `F16`,
    /// `Q5_1`, `Q4_K_M`, `INT8`.
    pub quantization: String,
    /// Memory of the worker process that holds the model, right after the load.
    ///
    /// Both figures are measured, not estimated: the release workers on an M4 Pro Mac with 24 GB,
    /// driven through their pipes the way the app drives them (GPU on for speech and cleanup, 8
    /// threads), as the larger of the resident size and the physical footprint. Linux and other
    /// Macs differ somewhat.
    pub ram_idle_bytes: u64,
    /// The most memory of that process during a long job: 5 minutes of audio, a cleanup of 500
    /// words, or a paragraph read aloud.
    pub ram_peak_bytes: u64,
    /// Who or what the model suits, as one short line.
    pub recommended_for: String,
    /// The model's page on Hugging Face.
    pub source_page: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    /// Short id: `tiny.en`, `qwen3.5-2b`, `kokoro-82m-int8`. A speech model's id is also the
    /// value of `dictation.modelId`.
    pub id: String,
    pub kind: ModelKind,
    pub name: String,
    /// One file for a speech or a cleanup model, every file of the folder for a voice model.
    /// Never empty. File names are unique across the catalog, so partial downloads never clash.
    pub files: Vec<ModelFile>,
    pub license: String,
    pub languages: Languages,
    /// The model the app proposes first among its kind.
    pub default: bool,
    /// `None` in a schema 1 catalog.
    pub info: Option<ModelInfo>,
}

impl ModelEntry {
    /// The size of every file together.
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub whisper: Vec<ModelEntry>,
    /// The AI cleanup models. The file name is `<id>.gguf`, which is where the worker looks.
    pub llm: Vec<ModelEntry>,
    pub voice: Vec<ModelEntry>,
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
        let schema = root.get("schema").and_then(Value::as_u64).unwrap_or(1);
        let catalog = Self {
            whisper: parse_list(&root, ModelKind::Speech, schema)?,
            llm: parse_list(&root, ModelKind::Cleanup, schema)?,
            voice: parse_list(&root, ModelKind::Voice, schema)?,
        };
        if let Some(entry) = catalog.llm.iter().find(|entry| {
            entry
                .files
                .iter()
                .any(|f| f.file != format!("{}.gguf", entry.id))
        }) {
            return Err(format!(
                "model `{}` must be stored as `<id>.gguf`",
                entry.id
            ));
        }
        let (mut ids, mut files) = (HashSet::new(), HashSet::new());
        for entry in catalog.models() {
            if !ids.insert(entry.id.as_str()) {
                return Err(format!("duplicate model id `{}`", entry.id));
            }
            if let Some(file) = entry.files.iter().find(|f| !files.insert(f.file.as_str())) {
                return Err(format!("the file `{}` is listed twice", file.file));
            }
        }
        Ok(catalog)
    }

    /// Every model, speech first, then cleanup, then voice.
    pub fn models(&self) -> impl Iterator<Item = &ModelEntry> {
        self.whisper.iter().chain(&self.llm).chain(&self.voice)
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

    pub fn voice(&self, id: &str) -> Option<&ModelEntry> {
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

/// The list of `kind`. Only the speech list is required, so a catalog from before cleanup or
/// voice models still parses.
fn parse_list(root: &Value, kind: ModelKind, schema: u64) -> Result<Vec<ModelEntry>, String> {
    let Some(list) = root.get(kind.list()).and_then(Value::as_array) else {
        return if kind == ModelKind::Speech {
            Err(format!("the catalog has no `{}` list", kind.list()))
        } else {
            Ok(Vec::new())
        };
    };
    let entries = list
        .iter()
        .map(|item| parse_entry(item, kind, schema))
        .collect::<Result<Vec<_>, _>>()?;
    if entries.iter().filter(|entry| entry.default).count() > 1 {
        return Err(format!("more than one default {} model", kind.key()));
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

fn parse_entry(item: &Value, kind: ModelKind, schema: u64) -> Result<ModelEntry, String> {
    let id = text(item, "id")?;
    if !is_safe_name(&id) {
        return Err(format!("model `{id}` has an unsafe id"));
    }
    match item.get("kind") {
        Some(_) if text(item, "kind")? != kind.key() => {
            return Err(format!("model `{id}` must be of kind `{}`", kind.key()));
        }
        None if schema >= DETAILS_SCHEMA => return Err(format!("model `{id}` needs a `kind`")),
        _ => {}
    }
    let files = if kind == ModelKind::Voice {
        parse_files(item, &id)?
    } else {
        vec![parse_file(item, &id)?]
    };
    Ok(ModelEntry {
        kind,
        name: text(item, "name")?,
        files,
        license: text(item, "license")?,
        languages: parse_languages(item, &id)?,
        default: item
            .get("default")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        info: if schema >= DETAILS_SCHEMA {
            Some(parse_info(item, &id)?)
        } else {
            None
        },
        id,
    })
}

/// The `files` list of a model that is a folder of files.
fn parse_files(item: &Value, id: &str) -> Result<Vec<ModelFile>, String> {
    let mut files: Vec<ModelFile> = Vec::new();
    for file in item
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        files.push(parse_file(file, id)?);
    }
    if files.is_empty() {
        return Err(format!("model `{id}` needs a list of files"));
    }
    Ok(files)
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
    if !is_safe_name(&file) {
        return Err(format!("model `{id}` has an unsafe file name"));
    }
    Ok(ModelFile {
        file,
        bytes,
        sha256,
        url: text(item, "url")?,
    })
}

fn parse_info(item: &Value, id: &str) -> Result<ModelInfo, String> {
    let scale = |key: &str| {
        item.get(key)
            .and_then(Value::as_u64)
            .filter(|value| (1..=5).contains(value))
            .map(|value| value as u8)
            .ok_or_else(|| format!("model `{id}` needs a `{key}` from 1 to 5"))
    };
    let bytes = |key: &str| {
        item.get(key)
            .and_then(Value::as_u64)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| format!("model `{id}` needs `{key}`"))
    };
    let source_page = text(item, "source_page")?;
    if !source_page.starts_with("https://huggingface.co/") {
        return Err(format!("model `{id}` needs a Hugging Face `source_page`"));
    }
    let (ram_idle_bytes, ram_peak_bytes) = (bytes("ram_idle_bytes")?, bytes("ram_peak_bytes")?);
    if ram_peak_bytes < ram_idle_bytes {
        return Err(format!(
            "model `{id}` has a peak memory under its idle memory"
        ));
    }
    Ok(ModelInfo {
        description: text(item, "description")?,
        speed: scale("speed")?,
        accuracy: scale("accuracy")?,
        quantization: text(item, "quantization")?,
        ram_idle_bytes,
        ram_peak_bytes,
        recommended_for: text(item, "recommended_for")?,
        source_page,
    })
}

/// A name that stays inside its folder.
fn is_safe_name(name: &str) -> bool {
    !name.contains(['/', '\\']) && !name.starts_with('.')
}

fn parse_languages(item: &Value, id: &str) -> Result<Languages, String> {
    match text(item, "languages")?.as_str() {
        "multilingual" => Ok(Languages::Multilingual),
        "english" => Ok(Languages::English),
        other => Err(format!("model `{id}` has unknown languages `{other}`")),
    }
}

#[cfg(test)]
mod tests;
