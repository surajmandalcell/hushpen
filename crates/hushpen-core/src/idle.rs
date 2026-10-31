//! What a model worker does with its model after a stretch without a job, and which memory a
//! page-out hands back to the system.
//!
//! `engine.idlePolicy` picks one rule for every worker: `keep`, `pageout`, or `unload` after
//! `engine.idleMinutes`. `keep` and `pageout` never unload a model. The default is `keep` on
//! macOS and `pageout` on Linux, and an empty or unknown value reads as the default.
//!
//! A page-out keeps the model loaded. On Linux the worker calls `madvise(MADV_PAGEOUT)` on the
//! ranges [`model_ranges`] finds: anonymous pages go to swap and clean file pages are dropped,
//! and either comes back on the next touch with the same contents. macOS has no public call for
//! this: the kernel compresses and swaps idle pages by itself, so there `pageout` reads as `keep`.

use serde_json::{Map, Value};
use std::ops::Range;

/// What happens when the idle time is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdleAction {
    /// Drop the model. The next job loads it again.
    #[default]
    Unload,
    /// Keep the model and hand its weight pages to the system (see the module comment).
    PageOut,
}

impl IdleAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unload => "unload",
            Self::PageOut => "pageout",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "unload" => Some(Self::Unload),
            "pageout" => Some(Self::PageOut),
            _ => None,
        }
    }
}

/// The idle rule of one worker. `secs == 0` keeps the model as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Idle {
    pub secs: u64,
    pub action: IdleAction,
}

impl Idle {
    pub const KEEP: Self = Self {
        secs: 0,
        action: IdleAction::Unload,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worker {
    Speech,
    Cleanup,
    Voice,
}

const DEFAULT_MINUTES: u64 = 15;

/// What `engine.idlePolicy` asks for on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    Keep,
    PageOut,
    Unload,
}

impl Policy {
    /// Whether this system takes a request to page a model out.
    pub const CAN_PAGE_OUT: bool = !cfg!(target_os = "macos");

    /// The policy a stored value means here. An empty or unknown value is the default.
    pub fn parse(value: Option<&str>) -> Self {
        Self::resolve(value, Self::CAN_PAGE_OUT)
    }

    fn resolve(value: Option<&str>, can_page_out: bool) -> Self {
        match value {
            Some("keep") => Self::Keep,
            Some("unload") => Self::Unload,
            _ if can_page_out => Self::PageOut,
            _ => Self::Keep,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::PageOut => "pageout",
            Self::Unload => "unload",
        }
    }
}

/// The idle rule the settings give `worker`.
pub fn for_worker(settings: &Map<String, Value>, worker: Worker) -> Idle {
    let after = settings
        .get("engine.idleMinutes")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_MINUTES)
        .saturating_mul(60);
    let policy = Policy::parse(settings.get("engine.idlePolicy").and_then(Value::as_str));
    match (policy, worker) {
        (Policy::Keep, _) => Idle::KEEP,
        (Policy::Unload, _) => Idle {
            secs: after,
            action: IdleAction::Unload,
        },
        // ONNX Runtime gives no handle on the weights of a voice model, so the system decides.
        (Policy::PageOut, Worker::Voice) => Idle::KEEP,
        (Policy::PageOut, _) => Idle {
            secs: after,
            action: IdleAction::PageOut,
        },
    }
}

/// One line of `/proc/self/maps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    pub range: Range<usize>,
    pub readable: bool,
    /// Empty for anonymous memory.
    pub path: String,
}

/// The mappings in the text of `/proc/self/maps`. Lines it cannot read are skipped.
pub fn parse_maps(text: &str) -> Vec<Mapping> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (start, end) = fields.next()?.split_once('-')?;
            let perms = fields.next()?;
            // offset, device, inode
            fields.nth(2)?;
            let path = fields.collect::<Vec<_>>().join(" ");
            Some(Mapping {
                range: usize::from_str_radix(start, 16).ok()?
                    ..usize::from_str_radix(end, 16).ok()?,
                readable: perms.starts_with('r'),
                path,
            })
        })
        .collect()
}

/// Shorter new ranges are heap growth, thread stacks, and tables, never a weight buffer.
pub const MIN_RANGE: usize = 16 << 20;

/// The memory a model load mapped: what `after` maps and `before` did not, in anonymous
/// mappings or mappings of `model_file`, in pieces of at least [`MIN_RANGE`]. Empty unless the
/// pieces add up to about the size of the file, so a load that kept its weights elsewhere
/// (the heap, or a GPU) is never advised.
pub fn model_ranges(
    before: &[Mapping],
    after: &[Mapping],
    model_file: &str,
    file_len: u64,
) -> Vec<Range<usize>> {
    let mut old: Vec<Range<usize>> = before.iter().map(|m| m.range.clone()).collect();
    old.sort_by_key(|range| range.start);
    let mut found = Vec::new();
    for mapping in after {
        if !mapping.readable || !(mapping.path.is_empty() || mapping.path == model_file) {
            continue;
        }
        let mut cursor = mapping.range.start;
        for gone in &old {
            if gone.end <= cursor || gone.start >= mapping.range.end {
                continue;
            }
            if gone.start > cursor {
                found.push(cursor..gone.start);
            }
            cursor = gone.end;
        }
        if cursor < mapping.range.end {
            found.push(cursor..mapping.range.end);
        }
    }
    found.retain(|range| range.len() >= MIN_RANGE);
    let total = found.iter().map(|range| range.len() as u64).sum::<u64>();
    if total < file_len / 2 || total > file_len.saturating_mul(3) {
        return Vec::new();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings(value: Value) -> Map<String, Value> {
        let Value::Object(map) = value else {
            unreachable!()
        };
        map
    }

    const ALL: [Worker; 3] = [Worker::Speech, Worker::Cleanup, Worker::Voice];

    #[test]
    fn an_empty_or_unknown_value_reads_as_the_default_of_the_system() {
        for value in [None, Some(""), Some("current"), Some("discard")] {
            assert_eq!(Policy::resolve(value, true), Policy::PageOut);
            assert_eq!(Policy::resolve(value, false), Policy::Keep);
        }
        assert_eq!(Policy::resolve(Some("pageout"), true), Policy::PageOut);
        assert_eq!(Policy::resolve(Some("pageout"), false), Policy::Keep);
        for can_page_out in [true, false] {
            assert_eq!(Policy::resolve(Some("keep"), can_page_out), Policy::Keep);
            assert_eq!(
                Policy::resolve(Some("unload"), can_page_out),
                Policy::Unload
            );
        }
    }

    #[test]
    fn nothing_stored_gives_the_default_rule_for_every_worker() {
        let unset = settings(json!({}));
        for worker in ALL {
            let idle = for_worker(&unset, worker);
            match Policy::parse(None) {
                Policy::PageOut if worker != Worker::Voice => assert_eq!(
                    idle,
                    Idle {
                        secs: 900,
                        action: IdleAction::PageOut
                    }
                ),
                _ => assert_eq!(idle, Idle::KEEP),
            }
        }
    }

    #[test]
    fn keep_and_unload_apply_to_every_worker_with_the_idle_minutes() {
        let keep = settings(json!({"engine.idlePolicy": "keep", "engine.idleMinutes": 5}));
        let unload = settings(json!({"engine.idlePolicy": "unload", "engine.idleMinutes": 30}));
        for worker in ALL {
            assert_eq!(for_worker(&keep, worker), Idle::KEEP);
            assert_eq!(
                for_worker(&unload, worker),
                Idle {
                    secs: 1800,
                    action: IdleAction::Unload
                }
            );
        }
    }

    #[test]
    fn page_out_applies_to_the_ggml_workers_and_keeps_a_voice_model() {
        let values = settings(json!({"engine.idlePolicy": "pageout"}));
        for worker in [Worker::Speech, Worker::Cleanup] {
            let expected = if Policy::CAN_PAGE_OUT {
                Idle {
                    secs: 900,
                    action: IdleAction::PageOut,
                }
            } else {
                Idle::KEEP
            };
            assert_eq!(for_worker(&values, worker), expected);
        }
        assert_eq!(for_worker(&values, Worker::Voice), Idle::KEEP);
    }

    #[test]
    fn actions_round_trip_through_their_names() {
        for action in [IdleAction::Unload, IdleAction::PageOut] {
            assert_eq!(IdleAction::parse(action.as_str()), Some(action));
        }
        assert_eq!(IdleAction::parse("discard"), None);
    }

    const MAPS: &str = "\
5600a000-5600b000 r-xp 00000000 103:02 1234 /usr/lib/hushpen/hushpen
5600c000-5610c000 rw-p 00000000 00:00 0 [heap]
7f0000000000-7f0009000000 rw-p 00000000 00:00 0
7f1000000000-7f1000001000 ---p 00000000 00:00 0
7f2000000000-7f201e000000 r--s 00000000 103:02 99 /models/s1 mini.gguf
";

    #[test]
    fn maps_lines_give_the_range_the_access_and_the_path() {
        let maps = parse_maps(MAPS);
        assert_eq!(maps.len(), 5);
        assert_eq!(maps[1].path, "[heap]");
        assert_eq!(maps[2].range, 0x7f00_0000_0000..0x7f00_0900_0000);
        assert!(maps[2].readable && maps[2].path.is_empty());
        assert!(!maps[3].readable);
        assert_eq!(maps[4].path, "/models/s1 mini.gguf");
        assert!(parse_maps("not a maps line\n").is_empty());
    }

    fn anon(range: Range<usize>) -> Mapping {
        Mapping {
            range,
            readable: true,
            path: String::new(),
        }
    }

    const MB: usize = 1 << 20;

    #[test]
    fn the_weights_are_the_new_large_anonymous_and_model_file_ranges() {
        let before = vec![anon(0..10 * MB)];
        let mut file = anon(400 * MB..480 * MB);
        file.path = "/m/model.gguf".into();
        let after = vec![
            // Grew by merging with the old mapping: only the new part counts.
            anon(0..110 * MB),
            // A thread stack.
            anon(200 * MB..208 * MB),
            Mapping {
                path: "[heap]".into(),
                ..anon(300 * MB..340 * MB)
            },
            Mapping {
                path: "/usr/lib/libggml.so".into(),
                ..anon(500 * MB..540 * MB)
            },
            file,
        ];
        assert_eq!(
            model_ranges(&before, &after, "/m/model.gguf", 180 * MB as u64),
            vec![10 * MB..110 * MB, 400 * MB..480 * MB]
        );
    }

    #[test]
    fn nothing_is_advised_when_the_new_ranges_do_not_match_the_file_size() {
        let after = vec![anon(0..100 * MB)];
        assert!(model_ranges(&[], &after, "/m/x.bin", 1000 * MB as u64).is_empty());
        assert!(model_ranges(&[], &after, "/m/x.bin", 20 * MB as u64).is_empty());
        assert_eq!(
            model_ranges(&[], &after, "/m/x.bin", 100 * MB as u64),
            vec![0..100 * MB]
        );
    }

    #[test]
    fn a_range_split_by_an_old_mapping_keeps_both_new_sides() {
        let before = vec![anon(40 * MB..50 * MB)];
        let after = vec![anon(0..100 * MB)];
        assert_eq!(
            model_ranges(&before, &after, "/m/x.bin", 90 * MB as u64),
            vec![0..40 * MB, 50 * MB..100 * MB]
        );
    }
}
