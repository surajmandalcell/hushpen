//! The settings registry: every key, its default, and its validation rule.

use crate::data_dir::Os;
use serde_json::{Map, Value, json};

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
    spec!("shortcut.hold", Kind::Text(64), |os| {
        json!(if mac(os) { "RightOption" } else { "RightAlt" })
    }),
    spec!("shortcut.handsFree", Kind::Text(64), |_| {
        json!("DoubleTap+Hold+Space")
    }),
    spec!("shortcut.pasteLast", Kind::Text(64), |os| {
        json!(if mac(os) { "Ctrl+Cmd+V" } else { "Ctrl+Alt+V" })
    }),
    spec!("shortcut.command", Kind::Text(64), |os| {
        json!(if mac(os) {
            "RightOption+RightShift"
        } else {
            "RightAlt+RightShift"
        })
    }),
    spec!("dictation.modelId", Kind::OptionalText(128), |_| json!("")),
    spec!("dictation.language", Kind::Language, |_| json!("auto")),
    spec!(
        "dictation.recentLanguages",
        Kind::TextList(5),
        |_| json!([])
    ),
    spec!("dictation.maxMinutes", Kind::Int(2, 60), |_| json!(6)),
    spec!("audio.inputDeviceId", Kind::Text(256), |_| json!("default")),
    spec!("audio.cueSounds", Kind::Bool, |_| json!(true)),
    spec!("audio.cueVolume", Kind::Float(0.0, 1.0), |_| json!(0.5)),
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
    spec!("cleanup.llm.timeoutMs", Kind::Int(500, 120_000), |os| {
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
    spec!("overlay.idleVisible", Kind::Bool, |_| json!(true)),
    spec!("overlay.position", Kind::Choice(&["top", "bottom"]), |_| {
        json!("bottom")
    }),
    spec!(
        "history.audioRetention",
        Kind::Choice(&["never", "30d", "forever"]),
        |_| json!("30d")
    ),
    spec!("engine.useGpu", Kind::Bool, |os| json!(mac(os))),
    spec!("engine.threads", Kind::AutoOrInt(1, 64), |_| json!("auto")),
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
];

pub(super) fn find(key: &str) -> Option<&'static Spec> {
    REGISTRY.iter().find(|spec| spec.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn a_position_keeps_only_x_and_y() {
        let spec = find("overlay.customPos").unwrap();
        let cleaned = spec.sanitize(&json!({"x": 1, "y": 2.5, "z": 9}));
        assert_eq!(cleaned, Some(json!({"x": 1.0, "y": 2.5})));
        assert_eq!(spec.sanitize(&json!({"x": 1})), None);
    }
}
