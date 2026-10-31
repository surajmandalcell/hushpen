//! The settings registry: every key, its default, and its validation rule.

use crate::data_dir::Os;
use hushpen_core::shortcut::{Platform, Slot};
use serde_json::{Map, Value, json};
use std::ops::RangeInclusive;

pub(super) enum Kind {
    Bool,
    /// A non-empty string of at most this many characters.
    Text(usize),
    /// A string that may be empty.
    OptionalText(usize),
    /// `auto` or a language code that whisper knows.
    Language,
    Choice(&'static [&'static str]),
    Int(i64, i64),
    Float(f64, f64),
    /// The string `auto` or an integer in range.
    AutoOrInt(i64, i64),
    /// An array of non-empty strings, cut to this length.
    TextList(usize),
    /// An object of string values; other entries are dropped.
    TextMap,
    /// An object of booleans; other entries are dropped.
    BoolMap,
    /// `null` or an object with numeric `x` and `y`.
    Position,
}

pub(super) struct Spec {
    pub key: &'static str,
    pub kind: Kind,
    pub internal: bool,
    default: fn(Os) -> Value,
}

impl Spec {
    pub fn default_value(&self, os: Os) -> Value {
        (self.default)(os)
    }

    /// The smallest and the largest number a numeric key takes.
    pub fn range(&self) -> Option<RangeInclusive<f64>> {
        match self.kind {
            Kind::Int(min, max) | Kind::AutoOrInt(min, max) => Some(min as f64..=max as f64),
            Kind::Float(min, max) => Some(min..=max),
            _ => None,
        }
    }

    /// The value in its accepted form, or `None` when it is invalid.
    pub fn sanitize(&self, value: &Value) -> Option<Value> {
        match &self.kind {
            Kind::Bool => value.is_boolean().then(|| value.clone()),
            Kind::Text(max) => value
                .as_str()
                .filter(|s| !s.is_empty() && s.chars().count() <= *max)
                .map(|_| value.clone()),
            Kind::OptionalText(max) => value
                .as_str()
                .filter(|s| s.chars().count() <= *max)
                .map(|_| value.clone()),
            Kind::Language => value
                .as_str()
                .filter(|s| hushpen_core::language::is_setting_value(s))
                .map(|_| value.clone()),
            Kind::Choice(options) => value
                .as_str()
                .filter(|s| options.contains(s))
                .map(|_| value.clone()),
            Kind::Int(min, max) => value
                .as_i64()
                .filter(|n| (min..=max).contains(&n))
                .map(|_| value.clone()),
            Kind::Float(min, max) => value
                .as_f64()
                .filter(|n| n.is_finite() && (*min..=*max).contains(n))
                .map(|_| value.clone()),
            Kind::AutoOrInt(min, max) => (value.as_str() == Some("auto")
                || value.as_i64().is_some_and(|n| (min..=max).contains(&&n)))
            .then(|| value.clone()),
            Kind::TextList(max) => value.as_array().map(|items| {
                let kept: Vec<_> = items
                    .iter()
                    .filter(|item| item.as_str().is_some_and(|s| !s.is_empty()))
                    .take(*max)
                    .cloned()
                    .collect();
                Value::Array(kept)
            }),
            Kind::TextMap => filtered_map(value, Value::is_string),
            Kind::BoolMap => filtered_map(value, Value::is_boolean),
            Kind::Position => match value {
                Value::Null => Some(Value::Null),
                Value::Object(map) => {
                    let coordinate = |name: &str| map.get(name).and_then(Value::as_f64);
                    let (x, y) = (coordinate("x")?, coordinate("y")?);
                    (x.is_finite() && y.is_finite()).then(|| json!({"x": x, "y": y}))
                }
                _ => None,
            },
        }
    }
}

fn filtered_map(value: &Value, keep: fn(&Value) -> bool) -> Option<Value> {
    let map = value.as_object()?;
    let kept: Map<String, Value> = map
        .iter()
        .filter(|(_, entry)| keep(entry))
        .map(|(key, entry)| (key.clone(), entry.clone()))
        .collect();
    Some(Value::Object(kept))
}

fn mac(os: Os) -> bool {
    os == Os::MacOs
}

fn shortcut_default(slot: Slot, os: Os) -> Value {
    let platform = if mac(os) {
        Platform::MacOs
    } else {
        Platform::Linux
    };
    json!(slot.default_text(platform))
}

macro_rules! spec {
    ($key:literal, $kind:expr, $default:expr) => {
        Spec {
            key: $key,
            kind: $kind,
            internal: false,
            default: $default,
        }
    };
    (internal $key:literal, $kind:expr, $default:expr) => {
        Spec {
            key: $key,
            kind: $kind,
            internal: true,
            default: $default,
        }
    };
}

pub(super) static REGISTRY: &[Spec] = &[
    spec!("shortcut.hold", Kind::Text(64), |os| shortcut_default(
        Slot::Hold,
        os
    )),
    spec!("shortcut.handsFree", Kind::Text(64), |os| {
        shortcut_default(Slot::HandsFree, os)
    }),
    spec!("shortcut.pasteLast", Kind::Text(64), |os| {
        shortcut_default(Slot::PasteLast, os)
    }),
    spec!("shortcut.command", Kind::Text(64), |os| {
        shortcut_default(Slot::Command, os)
    }),
    spec!("shortcut.readAloud", Kind::Text(64), |os| {
        shortcut_default(Slot::ReadAloud, os)
    }),
    spec!("shortcut.enabled", Kind::BoolMap, |_| json!({})),
    spec!("dictation.modelId", Kind::OptionalText(128), |_| json!("")),
    spec!("dictation.language", Kind::Language, |_| json!("auto")),
    spec!(
        "dictation.recentLanguages",
        Kind::TextList(5),
        |_| json!([])
    ),
    spec!("dictation.maxMinutes", Kind::Int(2, 60), |_| json!(6)),
    spec!("audio.inputDeviceId", Kind::Text(256), |_| json!("default")),
    spec!("audio.inputGain", Kind::Float(1.0, 2.0), |_| json!(1.5)),
    spec!(
        "audio.enhance",
        Kind::Choice(&["off", "auto", "custom"]),
        |_| json!("auto")
    ),
    // The Custom values start at the Auto preset. A low cut of 0 is off.
    spec!("audio.enhance.lowCutHz", Kind::Int(0, 200), |_| json!(80)),
    spec!("audio.enhance.bassDb", Kind::Int(-12, 6), |_| json!(-6)),
    spec!("audio.enhance.trebleDb", Kind::Int(-6, 12), |_| json!(0)),
    spec!("audio.enhance.noiseReduction", Kind::Bool, |_| json!(false)),
    spec!("audio.cueSounds", Kind::Bool, |_| json!(true)),
    spec!("audio.muteWhileDictating", Kind::Bool, |_| json!(true)),
    spec!("audio.cueVolume", Kind::Float(0.0, 1.0), |_| json!(0.25)),
    spec!("cleanup.rules", Kind::Bool, |_| json!(true)),
    spec!("cleanup.spokenPunctuation", Kind::Bool, |_| json!(true)),
    spec!("cleanup.llm.enabled", Kind::Bool, |_| json!(false)),
    spec!(
        "cleanup.llm.provider",
        Kind::Choice(&["local", "endpoint"]),
        |_| json!("local")
    ),
    spec!("cleanup.llm.modelId", Kind::OptionalText(128), |_| json!(
        ""
    )),
    spec!("cleanup.llm.timeoutMs", Kind::Int(100, 120_000), |os| {
        json!(if mac(os) { 3000 } else { 8000 })
    }),
    spec!(
        "cleanup.llm.endpointTimeoutMs",
        Kind::Int(500, 120_000),
        |_| { json!(6000) }
    ),
    spec!(
        "cleanup.llm.idleUnloadMinutes",
        Kind::Int(1, 1440),
        |_| json!(5)
    ),
    spec!("cleanup.llm.endpointPreset", Kind::Text(64), |_| json!(
        "custom"
    )),
    spec!(
        "cleanup.llm.endpointUrl",
        Kind::OptionalText(2048),
        |_| json!("")
    ),
    spec!(
        "cleanup.llm.endpointModel",
        Kind::OptionalText(256),
        |_| json!("")
    ),
    spec!("insert.appChords", Kind::TextMap, |_| json!({})),
    spec!("overlay.enabled", Kind::Bool, |_| json!(true)),
    spec!("overlay.idleVisible", Kind::Bool, |_| json!(false)),
    spec!("overlay.position", Kind::Choice(&["top", "bottom"]), |_| {
        json!("bottom")
    }),
    spec!(
        "history.audioRetention",
        Kind::Choice(&["never", "30d", "forever"]),
        |_| json!("30d")
    ),
    spec!(
        "files.mode",
        Kind::Choice(&["transcribe", "readAloud"]),
        |_| json!("transcribe")
    ),
    spec!("files.language", Kind::Language, |_| json!("auto")),
    spec!("files.translate", Kind::Bool, |_| json!(false)),
    spec!(
        "readAloud.engine",
        Kind::Choice(&["system", "kokoro", "chatterbox-turbo", "chatterbox-nano"]),
        |_| json!("system")
    ),
    spec!(
        "readAloud.voice.system",
        Kind::OptionalText(256),
        |_| json!("")
    ),
    spec!(
        "readAloud.voice.kokoro",
        Kind::OptionalText(256),
        |_| json!("")
    ),
    // The Kokoro voice blended into the chosen one, by id. Empty is no mix.
    spec!(
        "readAloud.voice.kokoroMix",
        Kind::OptionalText(64),
        |_| json!("")
    ),
    // The share of the blended voice, in percent.
    spec!(
        "readAloud.voice.kokoroMixPercent",
        Kind::Int(10, 90),
        |_| json!(50)
    ),
    // One choice for both Chatterbox models, which read with the same voices. Empty is the
    // default voice of the voice library.
    spec!(
        "readAloud.voice.chatterbox",
        Kind::OptionalText(256),
        |_| json!("")
    ),
    // The voice library's default voice, by id. Empty is the voice that comes with the model.
    spec!("readAloud.defaultVoice", Kind::OptionalText(64), |_| json!(
        ""
    )),
    spec!("readAloud.speed", Kind::Float(0.5, 2.0), |_| json!(1.0)),
    spec!("readAloud.format", Kind::Choice(&["wav"]), |_| json!("wav")),
    spec!("engine.useGpu", Kind::Bool, |os| json!(mac(os))),
    spec!("engine.threads", Kind::AutoOrInt(1, 64), |_| json!("auto")),
    spec!(
        "engine.idlePolicy",
        Kind::Choice(&["current", "keep", "pageout", "unload"]),
        |_| json!("current")
    ),
    spec!("engine.idleMinutes", Kind::Int(5, 60), |_| json!(15)),
    spec!("startup.launchAtLogin", Kind::Bool, |_| json!(false)),
    spec!("startup.startHidden", Kind::Bool, |_| json!(false)),
    spec!("updates.check", Kind::Bool, |_| json!(false)),
    spec!("updates.autoInstall", Kind::Bool, |_| json!(false)),
    spec!(internal "onboarding.completed", Kind::Bool, |_| json!(false)),
    spec!(internal "onboarding.step", Kind::OptionalText(64), |_| json!("")),
    spec!(internal "permissions.lastGranted", Kind::BoolMap, |_| json!({})),
    spec!(internal "updates.lastCheckAt", Kind::Int(0, i64::MAX), |_| json!(0)),
    spec!(internal "updates.dismissedVersion", Kind::OptionalText(64), |_| {
        json!("")
    }),
    spec!(internal "overlay.customPos", Kind::Position, |_| Value::Null),
    spec!(internal "ui.lastView", Kind::Choice(VIEWS), |_| json!("home")),
    spec!(internal "ui.windowHeight", Kind::Int(600, 4000), |_| json!(600)),
];

/// The keys of the sidebar views, which the app saves as the last view. Manual keeps `files`,
/// its name before the rename.
const VIEWS: &[&str] = &[
    "home",
    "history",
    "files",
    "dictionary",
    "models",
    "settings",
    "about",
];

pub(super) fn find(key: &str) -> Option<&'static Spec> {
    REGISTRY.iter().find(|spec| spec.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_mute_and_idle_defaults_preserve_the_retention_policy() {
        for os in [Os::MacOs, Os::Linux] {
            assert_eq!(
                find("audio.muteWhileDictating").unwrap().default_value(os),
                json!(true)
            );
            assert_eq!(
                find("engine.idlePolicy").unwrap().default_value(os),
                json!("current")
            );
            assert_eq!(
                find("engine.idleMinutes").unwrap().default_value(os),
                json!(15)
            );
        }
        let policy = find("engine.idlePolicy").unwrap();
        for value in ["current", "keep", "pageout", "unload"] {
            assert_eq!(policy.sanitize(&json!(value)), Some(json!(value)));
        }
        assert_eq!(policy.sanitize(&json!("discard")), None);
        let minutes = find("engine.idleMinutes").unwrap();
        assert_eq!(minutes.sanitize(&json!(4)), None);
        assert_eq!(minutes.sanitize(&json!(61)), None);
        for value in [5, 15, 30, 60] {
            assert_eq!(minutes.sanitize(&json!(value)), Some(json!(value)));
        }
        assert_eq!(
            find("audio.muteWhileDictating")
                .unwrap()
                .sanitize(&json!("true")),
            None
        );
    }

    #[test]
    fn every_default_passes_its_own_rule_on_both_systems() {
        for os in [Os::MacOs, Os::Linux] {
            for spec in REGISTRY {
                let default = spec.default_value(os);
                assert_eq!(
                    spec.sanitize(&default),
                    Some(default.clone()),
                    "{}",
                    spec.key
                );
            }
        }
    }

    #[test]
    fn keys_are_unique() {
        for (index, spec) in REGISTRY.iter().enumerate() {
            assert!(
                REGISTRY[index + 1..]
                    .iter()
                    .all(|other| other.key != spec.key),
                "{}",
                spec.key
            );
        }
    }

    #[test]
    fn platform_defaults_differ_where_the_plan_says() {
        let timeout = find("cleanup.llm.timeoutMs").unwrap();
        assert_eq!(timeout.default_value(Os::MacOs), json!(3000));
        assert_eq!(timeout.default_value(Os::Linux), json!(8000));
        let hold = find("shortcut.hold").unwrap();
        assert_eq!(hold.default_value(Os::MacOs), json!("RightOption"));
        assert_eq!(hold.default_value(Os::Linux), json!("RightAlt"));
        let read = find("shortcut.readAloud").unwrap();
        assert_eq!(read.default_value(Os::MacOs), json!("Ctrl+Cmd+S"));
        assert_eq!(read.default_value(Os::Linux), json!("Ctrl+Alt+S"));
    }

    #[test]
    fn the_cleanup_timeout_accepts_100_ms_for_the_forced_timeout_check() {
        let timeout = find("cleanup.llm.timeoutMs").unwrap();
        assert_eq!(timeout.sanitize(&json!(100)), Some(json!(100)));
        assert_eq!(timeout.sanitize(&json!(99)), None);
        assert_eq!(timeout.sanitize(&json!(120_001)), None);
    }

    #[test]
    fn numeric_keys_report_their_range() {
        let range = |key| find(key).unwrap().range();
        assert_eq!(range("dictation.maxMinutes"), Some(2.0..=60.0));
        assert_eq!(range("audio.cueVolume"), Some(0.0..=1.0));
        assert_eq!(range("audio.inputGain"), Some(1.0..=2.0));
        assert_eq!(range("audio.enhance.lowCutHz"), Some(0.0..=200.0));
        assert_eq!(range("audio.enhance.bassDb"), Some(-12.0..=6.0));
        assert_eq!(range("audio.enhance.trebleDb"), Some(-6.0..=12.0));
        assert_eq!(range("engine.threads"), Some(1.0..=64.0));
        assert_eq!(range("audio.cueSounds"), None);
    }

    #[test]
    fn the_files_view_keys_refuse_values_outside_their_rule() {
        let sanitize = |key, value| find(key).unwrap().sanitize(&value);
        assert_eq!(
            sanitize("files.mode", json!("readAloud")),
            Some(json!("readAloud"))
        );
        assert_eq!(sanitize("files.mode", json!("import")), None);
        assert_eq!(sanitize("files.language", json!("es")), Some(json!("es")));
        assert_eq!(sanitize("files.language", json!("xx")), None);
        assert_eq!(sanitize("readAloud.engine", json!("piper")), None);
        assert_eq!(
            sanitize("readAloud.engine", json!("chatterbox-nano")),
            Some(json!("chatterbox-nano"))
        );
        assert_eq!(
            sanitize("readAloud.defaultVoice", json!("")),
            Some(json!(""))
        );
        assert_eq!(sanitize("readAloud.speed", json!(2.5)), None);
        assert_eq!(sanitize("readAloud.speed", json!(0.5)), Some(json!(0.5)));
        assert_eq!(sanitize("readAloud.format", json!("mp3")), None);
        assert_eq!(
            sanitize("readAloud.voice.kokoroMixPercent", json!(95)),
            None
        );
        assert_eq!(
            sanitize("readAloud.voice.kokoroMixPercent", json!(30)),
            Some(json!(30))
        );
    }

    #[test]
    fn the_last_view_is_internal_and_takes_only_a_view_key() {
        let spec = find("ui.lastView").unwrap();
        assert!(spec.internal);
        assert_eq!(spec.sanitize(&json!("files")), Some(json!("files")));
        assert_eq!(spec.sanitize(&json!("manual")), None);
        assert_eq!(spec.sanitize(&json!("onboarding")), None);
    }

    #[test]
    fn the_window_height_is_internal_and_stays_within_sane_bounds() {
        let spec = find("ui.windowHeight").unwrap();
        assert!(spec.internal);
        assert_eq!(spec.default_value(Os::MacOs), json!(600));
        assert_eq!(spec.sanitize(&json!(750)), Some(json!(750)));
        assert_eq!(spec.sanitize(&json!(599)), None);
        assert_eq!(spec.sanitize(&json!("tall")), None);
    }

    #[test]
    fn audio_enhance_defaults_to_auto_and_custom_starts_at_the_auto_values() {
        let default = |key| find(key).unwrap().default_value(Os::MacOs);
        assert_eq!(default("audio.enhance"), json!("auto"));
        assert_eq!(default("audio.enhance.lowCutHz"), json!(80));
        assert_eq!(default("audio.enhance.bassDb"), json!(-6));
        assert_eq!(default("audio.enhance.trebleDb"), json!(0));
        assert_eq!(default("audio.enhance.noiseReduction"), json!(false));
        let sanitize = |key, value| find(key).unwrap().sanitize(&value);
        assert_eq!(sanitize("audio.enhance", json!("loud")), None);
        assert_eq!(sanitize("audio.enhance.lowCutHz", json!(201)), None);
        assert_eq!(sanitize("audio.enhance.bassDb", json!(-13)), None);
        assert_eq!(
            sanitize("audio.enhance.trebleDb", json!(12)),
            Some(json!(12))
        );
    }

    #[test]
    fn a_position_keeps_only_x_and_y() {
        let spec = find("overlay.customPos").unwrap();
        let cleaned = spec.sanitize(&json!({"x": 1, "y": 2.5, "z": 9}));
        assert_eq!(cleaned, Some(json!({"x": 1.0, "y": 2.5})));
        assert_eq!(spec.sanitize(&json!({"x": 1})), None);
    }
}
