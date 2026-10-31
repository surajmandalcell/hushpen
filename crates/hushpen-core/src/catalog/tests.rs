use super::*;
use serde_json::json;

const IDS: [&str; 4] = [
    SPEECH_DEFAULT,
    "large-v3-turbo-q5_0",
    "small-q5_1",
    "base-q5_1",
];

#[test]
fn the_embedded_catalog_lists_the_four_speech_models_in_order() {
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
        if entry.id == SPEECH_DEFAULT {
            assert_eq!(file.file, "ggml-parakeet-tdt-0.6b-v3-q4_k.bin");
            assert_eq!(entry.languages, Languages::European);
            assert_eq!(entry.license, "CC-BY-4.0");
        } else {
            assert_eq!(file.file, format!("ggml-{}.bin", entry.id));
            assert_eq!(entry.languages, Languages::Multilingual);
        }
        let prefix = format!("https://{}/", "huggingface.co");
        assert!(file.url.starts_with(&prefix), "{}", entry.id);
        assert!(file.url.ends_with(&file.file), "{}", entry.id);
        let revision = file
            .url
            .split("/resolve/")
            .nth(1)
            .unwrap()
            .split('/')
            .next()
            .unwrap();
        assert_eq!(revision.len(), 40, "{}", entry.id);
        assert!(
            revision.bytes().all(|b| b.is_ascii_hexdigit()),
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
            assert!(info.ram_idle_bytes > 100_000_000, "{}", entry.id);
            assert!(info.ram_peak_bytes >= info.ram_idle_bytes, "{}", entry.id);
            assert!(
                ["F16", "Q5_0", "Q5_1", "Q4_K", "Q4_K_M", "INT8", "Q4"]
                    .contains(&info.quantization.as_str()),
                "{} has an unknown quantization",
                entry.id
            );
            // A quantized file names its format; a plain whisper.cpp file is F16 and names none.
            let first = entry.files[0].url.to_ascii_lowercase();
            let named = first.contains(&info.quantization.to_ascii_lowercase());
            assert!(
                named || (info.quantization == "F16" && !first.contains("-q")),
                "{} is not {}",
                entry.id,
                info.quantization
            );
            assert!(
                info.source_page.starts_with("https://huggingface.co/"),
                "{}",
                entry.id
            );
            for file in &entry.files {
                let repo = info
                    .source_page
                    .trim_start_matches("https://huggingface.co");
                // The Turbo repository ships no voice clip, so its default voice comes from
                // another Chatterbox export.
                let from = if file.file == crate::chatterbox::DEFAULT_VOICE_FILE {
                    "https://huggingface.co/"
                } else {
                    repo
                };
                assert!(
                    file.url.contains(from) && file.url.contains("/resolve/"),
                    "{} is not from its source page",
                    file.file
                );
            }
        }
    }
}

#[test]
fn parakeet_matches_the_verified_download() {
    let expect = [(
        SPEECH_DEFAULT,
        415_611_879,
        "8b205b8b39c6535e153de6fb11c51db46125d45c4f16ba496fe41a0fe71b885e",
    )];
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
            "large-v3-turbo-q5_0",
            574_041_195,
            "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
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

fn offered() -> Vec<&'static str> {
    embedded()
        .offered_llm()
        .iter()
        .map(|entry| entry.id.as_str())
        .collect()
}

#[test]
fn the_two_cleanup_models_are_pinned_by_size_and_hash() {
    let expect = [
        (
            LLM_CLEANUP,
            484_219_808,
            "3b41ebe2502cbd03e811d5d16b022f5ab551eda58d62597d152f89535003c634",
        ),
        (
            LLM_GENERAL,
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
fn every_machine_gets_s1_mini_first_and_qwen_second() {
    assert_eq!(offered(), [LLM_CLEANUP, LLM_GENERAL]);
    assert_eq!(embedded().suggested_llm().unwrap().id, LLM_CLEANUP);
    // The license asks for this exact name wherever the model is used.
    assert_eq!(
        embedded().llm(LLM_CLEANUP).unwrap().name,
        "S1-mini by Superwhisper"
    );
}

#[test]
fn a_retired_or_unknown_choice_moves_to_the_suggested_model() {
    let catalog = embedded();
    assert_eq!(catalog.chosen_llm(LLM_GENERAL).unwrap().id, LLM_GENERAL);
    for stored in ["gemma-4-e2b", "", "nope"] {
        assert_eq!(
            catalog.chosen_llm(stored).unwrap().id,
            LLM_CLEANUP,
            "{stored}"
        );
    }
    for retired in RETIRED_LLM {
        assert!(catalog.llm(retired).is_none(), "{retired}");
    }
}

#[test]
fn no_cleanup_model_is_over_1_5_gb() {
    for entry in &embedded().llm {
        assert!(entry.bytes() <= 1_500_000_000, "{}", entry.id);
    }
}

#[test]
fn exactly_one_model_is_the_default() {
    let defaults: Vec<_> = embedded().whisper.iter().filter(|e| e.default).collect();
    assert_eq!(defaults.len(), 1);
    assert_eq!(embedded().default_whisper(), Some(defaults[0]));
    assert_eq!(defaults[0].id, SPEECH_DEFAULT);
}

#[test]
fn retired_speech_choices_fall_back_without_changing_supported_choices() {
    let catalog = embedded();
    for id in RETIRED_SPEECH.iter().copied().chain(["", "unknown"]) {
        assert!(catalog.whisper(id).is_none(), "{id}");
        assert_eq!(
            catalog.chosen_speech(id).unwrap().id,
            SPEECH_DEFAULT,
            "{id}"
        );
    }
    for id in IDS {
        assert_eq!(catalog.chosen_speech(id).unwrap().id, id);
    }
}

#[test]
fn speech_capabilities_match_the_models() {
    let parakeet = embedded().whisper(SPEECH_DEFAULT).unwrap();
    assert_eq!(PARAKEET_LANGUAGES.len(), 25);
    for code in PARAKEET_LANGUAGES.into_iter().chain(["auto"]) {
        assert!(parakeet.supports_language(code), "{code}");
        assert_eq!(parakeet.language_notice(code), None, "{code}");
    }
    for code in ["hi", "zh", "ja", "ta", "ar", "xx"] {
        assert!(!parakeet.supports_language(code), "{code}");
        assert!(
            parakeet
                .language_notice(code)
                .unwrap()
                .contains("Whisper Large v3 Turbo Q5")
        );
    }
    assert!(!parakeet.supports_translation());
    assert!(
        !embedded()
            .whisper(SPEECH_TURBO)
            .unwrap()
            .supports_translation()
    );
    for id in ["small-q5_1", "base-q5_1"] {
        let model = embedded().whisper(id).unwrap();
        assert!(model.supports_translation(), "{id}");
        assert!(model.supports_language("hi"), "{id}");
    }
    for entry in &embedded().whisper {
        assert!(entry.bytes() < 1_500_000_000, "{}", entry.id);
        assert!(entry.info.as_ref().unwrap().ram_peak_bytes < 2_000_000_000);
    }
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
        "quantization": "F16", "ram_idle_bytes": 1000, "ram_peak_bytes": 2000, "recommended_for": "R",
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
    assert_eq!((info.speed, info.accuracy), (3, 4));
    assert_eq!(info.quantization, "F16");
    assert_eq!((info.ram_idle_bytes, info.ram_peak_bytes), (1000, 2000));
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
        "quantization",
        "ram_idle_bytes",
        "ram_peak_bytes",
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
        with("ram_peak_bytes", json!(999)).is_err(),
        "the peak is never under the idle memory"
    );
    assert!(
        with("kind", json!("cleanup")).is_err(),
        "kind matches its list"
    );
    assert!(with("source_page", json!("https://github.com/x")).is_err());
}

#[test]
fn the_kokoro_voice_is_pinned_file_by_file_with_every_file_the_worker_opens() {
    let entry = embedded().voice(crate::kokoro::MODEL_ID).unwrap();
    assert_eq!(entry.kind, ModelKind::Voice);
    let files: Vec<_> = entry.files.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(files, crate::kokoro::FILES);
    let expect = [
        (
            114_202_993,
            "75635ea3b85a32d00094dcdd61a258aba87c160f7c264c304a7b3ae9d734195e",
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
        assert!(
            file.url
                .starts_with(&format!("{repo}57adb37fc0e8d4bd430cfc1b8f8c2294945b9af2/")),
            "{}",
            file.url
        );
        assert!(
            file.url.ends_with(&format!("/{}", file.file)),
            "{}",
            file.url
        );
    }
    assert_eq!(entry.bytes(), 148_361_525);
    assert!(entry.info.as_ref().unwrap().ram_peak_bytes < 1_500_000_000);
    assert_eq!(entry.license, "Apache-2.0");
    assert_eq!(entry.languages, Languages::English);
    assert!(entry.default);
}

#[test]
fn the_chatterbox_voices_are_pinned_with_every_file_the_worker_opens() {
    let repos = [
        (
            crate::chatterbox::TURBO_ID,
            "ResembleAI/chatterbox-turbo-ONNX",
            725_633_828,
        ),
        (
            crate::chatterbox::NANO_ID,
            "Masterx/chatterbox-nano-ONNX",
            496_223_239,
        ),
    ];
    for (id, repo, total) in repos {
        let entry = embedded().voice(id).unwrap();
        let files: Vec<_> = entry.files.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(files, crate::chatterbox::FILES, "{id}");
        for file in &entry.files {
            let pinned = format!("https://huggingface.co/{repo}/resolve/");
            let clip = "https://huggingface.co/onnx-community/chatterbox-ONNX/resolve/";
            assert!(
                file.url.starts_with(&pinned) || file.url.starts_with(clip),
                "{}",
                file.url
            );
            assert!(
                file.url.ends_with(&format!("/{}", file.file)),
                "{}",
                file.url
            );
        }
        assert_eq!(entry.bytes(), total, "{id}");
        assert_eq!(
            (entry.license.as_str(), entry.languages),
            ("MIT", Languages::English)
        );
        assert!(!entry.default, "Kokoro stays the voice the app proposes");
    }
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
    let two = parse(json!({"whisper": [], "voice": [
        {"id": "a", "kind": "voice", "name": "A", "license": "MIT", "languages": "english",
            "files": [file("tokenizer.json")]},
        {"id": "b", "kind": "voice", "name": "B", "license": "MIT", "languages": "english",
            "files": [file("tokenizer.json")]},
    ]}));
    assert!(
        two.is_ok(),
        "two voice folders may hold a file of the same name"
    );
    let old = parse(json!({"whisper": []})).unwrap();
    assert!(
        old.voice.is_empty(),
        "a catalog from before voice models parses"
    );
}
