//! The text of an export: TXT, SRT, VTT, and JSON files made from history rows.
//!
//! Rows go into one file in creation order. In SRT and VTT each row's cue times are moved
//! by the durations of the rows before it, so the cues of the file run on one clock. A row
//! with no segments gives one cue that spans its duration. A row with no final text gives no
//! text and no cue; JSON still lists it.

use serde_json::{Value, json};

/// Version of the JSON layout. A reader checks it before it trusts the fields.
pub const JSON_VERSION: u32 = 1;

/// A cue that has no segment to take its length from lasts this long.
const FALLBACK_CUE_MS: i64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Txt,
    Srt,
    Vtt,
    Json,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Txt, Format::Srt, Format::Vtt, Format::Json];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Txt => "txt",
            Format::Srt => "srt",
            Format::Vtt => "vtt",
            Format::Json => "json",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Txt => "TXT",
            Format::Srt => "SRT",
            Format::Vtt => "VTT",
            Format::Json => "JSON",
        }
    }

    /// The format for a key such as `srt` or `SRT`.
    pub fn from_key(key: &str) -> Option<Format> {
        Format::ALL
            .into_iter()
            .find(|format| format.extension().eq_ignore_ascii_case(key.trim()))
    }
}

/// One timed piece of a row, in milliseconds from the start of the row's audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// What an export needs to know about one history row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Item {
    pub id: String,
    /// Unix milliseconds.
    pub created_at_ms: i64,
    pub kind: String,
    pub status: String,
    pub error_code: Option<String>,
    pub duration_ms: i64,
    pub model_id: Option<String>,
    pub language_requested: Option<String>,
    pub language_detected: Option<String>,
    pub target_app: Option<String>,
    pub raw_text: Option<String>,
    pub final_text: Option<String>,
    pub segments: Vec<Segment>,
}

impl Item {
    /// The final text, or `None` when the row has none worth writing.
    pub fn text(&self) -> Option<&str> {
        self.final_text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    }
}

/// True when at least one item has text, so a TXT, SRT, or VTT file would not be empty.
pub fn has_text(items: &[Item]) -> bool {
    items.iter().any(|item| item.text().is_some())
}

/// The file content for `items`, oldest row first.
pub fn render(format: Format, items: &[Item]) -> String {
    let mut ordered: Vec<&Item> = items.iter().collect();
    ordered.sort_by(|a, b| (a.created_at_ms, &a.id).cmp(&(b.created_at_ms, &b.id)));
    match format {
        Format::Txt => txt(&ordered),
        Format::Srt => srt(&ordered),
        Format::Vtt => vtt(&ordered),
        Format::Json => json_file(&ordered),
    }
}

fn txt(items: &[&Item]) -> String {
    let texts: Vec<&str> = items.iter().filter_map(|item| item.text()).collect();
    if texts.is_empty() {
        return String::new();
    }
    format!("{}\n", texts.join("\n\n"))
}

struct Cue {
    start_ms: i64,
    end_ms: i64,
    text: String,
}

/// The cues of all items on one clock.
fn cues(items: &[&Item], escape: fn(&str) -> String) -> Vec<Cue> {
    let mut cues = Vec::new();
    let mut offset = 0_i64;
    for item in items {
        let Some(text) = item.text() else {
            continue;
        };
        let mut own: Vec<Cue> = item
            .segments
            .iter()
            .filter_map(|segment| {
                let text = cue_text(&segment.text);
                if text.is_empty() {
                    return None;
                }
                let start_ms = segment.start_ms.max(0);
                Some(Cue {
                    start_ms: offset + start_ms,
                    end_ms: offset + segment.end_ms.max(start_ms),
                    text: escape(&text),
                })
            })
            .collect();
        if own.is_empty() {
            let length = match item.duration_ms {
                length if length > 0 => length,
                _ => FALLBACK_CUE_MS,
            };
            own.push(Cue {
                start_ms: offset,
                end_ms: offset + length,
                text: escape(&cue_text(text)),
            });
        }
        cues.append(&mut own);
        offset += item.duration_ms.max(0);
    }
    cues
}

/// Lines trimmed and blank lines dropped: a blank line ends a cue in both formats.
fn cue_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn plain(text: &str) -> String {
    text.to_owned()
}

fn vtt_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace("-->", "--&gt;")
}

fn srt(items: &[&Item]) -> String {
    let blocks: Vec<String> = cues(items, plain)
        .iter()
        .enumerate()
        .map(|(index, cue)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                index + 1,
                clock(cue.start_ms, ','),
                clock(cue.end_ms, ','),
                cue.text
            )
        })
        .collect();
    blocks.join("\n")
}

fn vtt(items: &[&Item]) -> String {
    let mut out = String::from("WEBVTT\n");
    for cue in cues(items, vtt_escape) {
        out.push_str(&format!(
            "\n{} --> {}\n{}\n",
            clock(cue.start_ms, '.'),
            clock(cue.end_ms, '.'),
            cue.text
        ));
    }
    out
}

/// `HH:MM:SS<sep>mmm`. The hours grow past 99 when they have to.
fn clock(ms: i64, separator: char) -> String {
    let ms = ms.max(0);
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1_000 % 60,
        ms % 1_000
    )
}

fn json_file(items: &[&Item]) -> String {
    let rows: Vec<Value> = items.iter().map(|item| json_row(item)).collect();
    let file = json!({ "format_version": JSON_VERSION, "rows": rows });
    let mut text = serde_json::to_string_pretty(&file).unwrap_or_default();
    text.push('\n');
    text
}

fn json_row(item: &Item) -> Value {
    json!({
        "id": item.id,
        "created_at": item.created_at_ms,
        "created_at_iso": iso_utc(item.created_at_ms),
        "kind": item.kind,
        "status": item.status,
        "error_code": item.error_code,
        "duration_ms": item.duration_ms,
        "model": item.model_id,
        "language_requested": item.language_requested,
        "language_detected": item.language_detected,
        "target_app": item.target_app,
        "raw_text": item.raw_text,
        "final_text": item.final_text,
        "segments": item.segments.iter().map(|segment| json!({
            "start_ms": segment.start_ms,
            "end_ms": segment.end_ms,
            "text": segment.text.trim(),
        })).collect::<Vec<_>>(),
    })
}

/// `2023-11-14T22:13:20.000Z` for Unix milliseconds.
pub fn iso_utc(unix_ms: i64) -> String {
    let days = unix_ms.div_euclid(86_400_000);
    let in_day = unix_ms.rem_euclid(86_400_000);
    // Days since 0000-03-01, then the civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        in_day / 3_600_000,
        in_day / 60_000 % 60,
        in_day / 1_000 % 60,
        in_day % 1_000
    )
}

#[cfg(test)]
mod tests;
