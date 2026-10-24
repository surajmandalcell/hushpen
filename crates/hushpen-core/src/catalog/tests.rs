use super::*;
use serde_json::json;

const IDS: [&str; 13] = [
    "tiny",
    "tiny.en",
    "base",
    "base.en",
    "base-q5_1",
    "small",
    "small.en",
    "small-q5_1",
    "medium",
    "medium.en",
    "large-v3-turbo",
    "large-v3-turbo-q5_0",
    "large-v3",
];

#[test]
fn the_embedded_catalog_lists_the_whisper_models_in_order() {
    let ids: Vec<_> = embedded().whisper.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, IDS);
}

#[test]
fn every_entry_has_a_hash_a_size_a_language_field_and_a_huggingface_url() {
    for entry in &embedded().whisper {
        let [file] = entry.files.as_slice() else {
            panic!("{} has more than one file", entry.id);
        };
        assert_eq!(file.sha256.len(), 64, "{}", entry.id);
        assert!(file.bytes > 50_000_000, "{}", entry.id);
        assert_eq!(file.file, format!("ggml-{}.bin", entry.id));
        let prefix = format!("https://{}/", "huggingface.co");
        assert!(file.url.starts_with(&prefix), "{}", entry.id);
        assert!(file.url.ends_with(&file.file), "{}", entry.id);
        let english = entry.id.contains(".en");
        assert_eq!(
            entry.languages == Languages::English,
            english,
            "{}",
            entry.id
        );
    }
}

#[test]
fn every_embedded_model_has_its_kind_and_its_details() {
    for kind in ModelKind::ALL {
        let list = match kind {
            ModelKind::Speech => &embedded().whisper,
            ModelKind::Cleanup => &embedded().llm,
            ModelKind::Voice => &embedded().voice,
        };
        assert!(!list.is_empty(), "{kind:?}");
        for entry in list {
            assert_eq!(entry.kind, kind, "{}", entry.id);
            let info = entry.info.as_ref().unwrap();
            assert!(!info.description.is_empty(), "{}", entry.id);
            assert!(!info.recommended_for.is_empty(), "{}", entry.id);
            assert!((1..=5).contains(&info.speed), "{}", entry.id);
            assert!((1..=5).contains(&info.accuracy), "{}", entry.id);
            assert!(info.ram_bytes > 100_000_000, "{}", entry.id);
            assert!(
                info.source_page.starts_with("https://huggingface.co/"),
                "{}",
                entry.id
            );
            for file in &entry.files {
                let repo = info
                    .source_page
                    .trim_start_matches("https://huggingface.co");
                assert!(
                    file.url.contains(&format!("{repo}/resolve/")),
                    "{} is not from its source page",
                    file.file
                );
            }
        }
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
    assert_pinned(&expect);
}

/// Sizes and hashes from the Hugging Face API (`lfs.size`, `lfs.oid`) of ggerganov/whisper.cpp.
#[test]
fn the_added_whisper_variants_match_the_hugging_face_files() {
    let expect = [
        (
            "base-q5_1",
            59_707_625,
            "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
        ),
        (
            "small-q5_1",
            190_085_487,
            "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        ),
        (
            "medium.en",
            1_533_774_781,
            "cc37e93478338ec7700281a7ac30a10128929eb8f427dda2e865faa8f6da4356",
        ),
        (
            "large-v3-turbo-q5_0",
            574_041_195,
            "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        ),
        (
            "large-v3",
            3_095_033_483,
            "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
        ),
    ];
    assert_pinned(&expect);
}

fn assert_pinned(expect: &[(&str, u64, &str)]) {
    for (id, bytes, sha) in expect {
        let entry = embedded().whisper(id).unwrap();
        let file = &entry.files[0];
        assert_eq!((file.bytes, file.sha256.as_str()), (*bytes, *sha), "{id}");
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
        let file = &entry.files[0];
        assert_eq!((file.bytes, file.sha256.as_str()), (bytes, sha), "{id}");
        assert_eq!(file.file, format!("{id}.gguf"));
        assert!(file.url.starts_with("https://huggingface.co/"), "{id}");
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
        assert!(entry.bytes() > 1_000_000_000, "{}", entry.id);
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

/// A schema 1 speech entry: the field set before kinds and details.
fn old_entry(id: &str, sha: &str, file: &str) -> Value {
    json!({"id": id, "name": "N", "file": file, "bytes": 5, "sha256": sha, "url": "u",
        "license": "MIT", "languages": "english"})
}

/// A schema 2 speech entry with every detail.
fn new_entry(id: &str, file: &str) -> Value {
    let mut entry = old_entry(id, &"a".repeat(64), file);
    let details = json!({"kind": "speech", "description": "D", "speed": 3, "accuracy": 4,
        "ram_bytes": 1000, "recommended_for": "R",
        "source_page": "https://huggingface.co/ggerganov/whisper.cpp"});
    for (key, value) in details.as_object().unwrap() {
        entry[key] = value.clone();
    }
    entry
}

fn parse(value: Value) -> Result<Catalog, String> {
    Catalog::parse(&value.to_string())
}

#[test]
fn a_bad_hash_a_duplicate_or_a_path_in_the_file_name_is_refused() {
    let good = "a".repeat(64);
    let one = |entry: Value| parse(json!({"whisper": [entry]}));
    assert!(one(old_entry("x", &good, "ggml-x.bin")).is_ok());
    assert!(one(old_entry("x", "abc", "ggml-x.bin")).is_err());
    assert!(one(old_entry("x", &"A".repeat(64), "ggml-x.bin")).is_err());
    assert!(one(old_entry("x", &good, "../ggml-x.bin")).is_err());
    let twice = old_entry("x", &good, "f");
    assert!(parse(json!({"whisper": [twice.clone(), twice]})).is_err());
    let same_file = [old_entry("x", &good, "f"), old_entry("y", &good, "f")];
    assert!(
        parse(json!({"whisper": same_file})).is_err(),
        "two models never share a partial download name"
    );
}

#[test]
fn a_schema_1_catalog_parses_without_kinds_or_details() {
    let good = "a".repeat(64);
    let catalog = parse(json!({"whisper": [old_entry("x", &good, "f")]})).unwrap();
    let entry = &catalog.whisper[0];
    assert_eq!(entry.kind, ModelKind::Speech, "the list gives the kind");
    assert_eq!(entry.info, None);
    assert_eq!(entry.bytes(), 5);
}

#[test]
fn schema_2_reads_the_details_and_requires_them() {
    let catalog = parse(json!({"schema": 2, "whisper": [new_entry("x", "f")]})).unwrap();
    let info = catalog.whisper[0].info.clone().unwrap();
    assert_eq!((info.speed, info.accuracy, info.ram_bytes), (3, 4, 1000));
    assert_eq!(info.recommended_for, "R");

    let without = |key: &str| {
        let mut entry = new_entry("x", "f");
        entry.as_object_mut().unwrap().remove(key);
        parse(json!({"schema": 2, "whisper": [entry]}))
    };
    for key in [
        "kind",
        "description",
        "speed",
        "accuracy",
        "ram_bytes",
        "recommended_for",
        "source_page",
    ] {
        assert!(without(key).is_err(), "{key} is required in schema 2");
    }
    let with = |key: &str, value: Value| {
        let mut entry = new_entry("x", "f");
        entry[key] = value;
        parse(json!({"schema": 2, "whisper": [entry]}))
    };
    assert!(with("speed", json!(0)).is_err());
    assert!(with("accuracy", json!(6)).is_err());
    assert!(
        with("kind", json!("cleanup")).is_err(),
        "kind matches its list"
    );
    assert!(with("source_page", json!("https://example.com/x")).is_err());
}

#[test]
fn the_kokoro_voice_is_pinned_file_by_file_with_every_file_the_worker_opens() {
    let entry = embedded().voice(crate::kokoro::MODEL_ID).unwrap();
    assert_eq!(entry.kind, ModelKind::Voice);
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
    let voice = |id: &str, kind: &str, files: Vec<Value>| {
        parse(
            json!({"whisper": [], "voice": [{"id": id, "kind": kind, "name": "N",
            "license": "MIT", "languages": "english", "files": files}]}),
        )
    };
    let file = |name: &str| json!({"file": name, "bytes": 5, "sha256": good, "url": "u"});
    let parsed = voice("v", "voice", vec![file("a.onnx")]).unwrap();
    assert_eq!(parsed.voice[0].bytes(), 5);
    assert!(voice("v", "speech", vec![file("a.onnx")]).is_err());
    assert!(voice("../v", "voice", vec![file("a.onnx")]).is_err());
    assert!(voice("v", "voice", vec![]).is_err());
    assert!(voice("v", "voice", vec![file("../a.onnx")]).is_err());
    assert!(voice("v", "voice", vec![file("a.onnx"), file("a.onnx")]).is_err());
    let old = parse(json!({"whisper": []})).unwrap();
    assert!(
        old.voice.is_empty(),
        "a catalog from before voice models parses"
    );
}
