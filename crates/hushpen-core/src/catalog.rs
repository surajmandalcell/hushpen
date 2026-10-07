//! The model catalog: `assets/models.json`, embedded at build time.
//!
//! The file pins every downloadable model by size and sha256. The app trusts a model file only
//! when it matches its entry here.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub whisper: Vec<ModelEntry>,
}

impl Catalog {
    pub fn parse(json: &str) -> Result<Self, String> {
        let root: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
        let list = root
            .get("whisper")
            .and_then(Value::as_array)
            .ok_or("the catalog has no `whisper` list")?;
        let mut whisper = Vec::with_capacity(list.len());
        for item in list {
            let entry = parse_entry(item)?;
            if whisper.iter().any(|seen: &ModelEntry| seen.id == entry.id) {
                return Err(format!("duplicate model id `{}`", entry.id));
            }
            whisper.push(entry);
        }
        if whisper.iter().filter(|entry| entry.default).count() > 1 {
            return Err("more than one default model".into());
        }
        Ok(Self { whisper })
    }

    pub fn whisper(&self, id: &str) -> Option<&ModelEntry> {
        self.whisper.iter().find(|entry| entry.id == id)
    }

    pub fn default_whisper(&self) -> Option<&ModelEntry> {
        self.whisper.iter().find(|entry| entry.default)
    }
}

/// The catalog that ships in the app. A broken embedded file would be a build defect that the
/// unit tests catch; at run time it yields an empty list rather than a crash.
pub fn embedded() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        Catalog::parse(EMBEDDED).unwrap_or(Catalog {
            whisper: Vec::new(),
        })
    })
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
    let languages = match text(item, "languages")?.as_str() {
        "multilingual" => Languages::Multilingual,
        "english" => Languages::English,
        other => return Err(format!("model `{id}` has unknown languages `{other}`")),
    };
    let file = text(item, "file")?;
    if file.contains(['/', '\\']) || file.starts_with('.') {
        return Err(format!("model `{id}` has an unsafe file name"));
    }
    Ok(ModelEntry {
        name: text(item, "name")?,
        file,
        bytes,
        sha256,
        url: text(item, "url")?,
        license: text(item, "license")?,
        languages,
        default: item
            .get("default")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        id,
    })
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
}
