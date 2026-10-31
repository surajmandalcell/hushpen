//! What a model worker does with its model after a stretch without a job, and which memory a
//! page-out hands back to the system.
//!
//! `engine.idlePolicy` picks one rule for every worker: `keep`, `pageout`, or `unload` after
//! `engine.idleMinutes`. `current` keeps the rule each worker had before the setting: the
//! speech model stays loaded, the cleanup model unloads after `cleanup.llm.idleUnloadMinutes`,
//! and a voice model unloads after five minutes.
//!
//! A page-out keeps the model loaded. On Linux the worker calls `madvise(MADV_PAGEOUT)` on the
//! ranges [`model_ranges`] finds: anonymous pages go to swap and clean file pages are dropped,
//! and either comes back on the next touch with the same contents. Other systems have no public
//! call for this, so the model stays as it is and the kernel compresses or swaps idle pages under
//! memory pressure.

use crate::tts_protocol;
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
const CLEANUP_DEFAULT_MINUTES: u64 = 5;

/// The idle rule the settings give `worker`.
pub fn for_worker(settings: &Map<String, Value>, worker: Worker) -> Idle {
    let minutes = |key: &str, default: u64| {
        settings
            .get(key)
            .and_then(Value::as_u64)
            .unwrap_or(default)
            .saturating_mul(60)
    };
    let after = minutes("engine.idleMinutes", DEFAULT_MINUTES);
    let policy = settings.get("engine.idlePolicy").and_then(Value::as_str);
    match (policy, worker) {
        (Some("keep"), _) => Idle::KEEP,
        (Some("unload"), _) => Idle {
            secs: after,
            action: IdleAction::Unload,
        },
        // ONNX Runtime gives no handle on the weights of a voice model, so the system decides.
        (Some("pageout"), Worker::Voice) => Idle::KEEP,
        (Some("pageout"), _) => Idle {
            secs: after,
            action: IdleAction::PageOut,
        },
        (_, Worker::Speech) => Idle::KEEP,
        (_, Worker::Cleanup) => Idle {
            secs: minutes("cleanup.llm.idleUnloadMinutes", CLEANUP_DEFAULT_MINUTES),
            action: IdleAction::Unload,
        },
        (_, Worker::Voice) => Idle {
            secs: tts_protocol::DEFAULT_IDLE_SECS,
            action: IdleAction::Unload,
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
    fn current_keeps_the_rule_each_worker_had_before_the_setting() {
        let unset = settings(json!({}));
        let current = settings(json!({"engine.idlePolicy": "current", "engine.idleMinutes": 30}));
        for values in [unset, current] {
            assert_eq!(for_worker(&values, Worker::Speech), Idle::KEEP);
            assert_eq!(
                for_worker(&values, Worker::Cleanup),
                Idle {
                    secs: 300,
                    action: IdleAction::Unload
                }
            );
            assert_eq!(
                for_worker(&values, Worker::Voice),
                Idle {
                    secs: 300,
                    action: IdleAction::Unload
                }
            );
        }
        let cleanup = settings(json!({"cleanup.llm.idleUnloadMinutes": 7}));
        assert_eq!(for_worker(&cleanup, Worker::Cleanup).secs, 420);
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
            assert_eq!(
                for_worker(&values, worker),
                Idle {
                    secs: 900,
                    action: IdleAction::PageOut
                }
            );
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
