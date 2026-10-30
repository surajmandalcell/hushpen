//! The row registry of the Settings list: every row in page order, its section, and the words a
//! search finds it by.

use super::Section::{self, Audio, Cleanup, Engine, FlowBar, General, Shortcuts, Storage, Updates};
use RowId::*;

/// One row of the Settings list. A row may draw more than one line, such as the options of an
/// open picker or a notice that belongs to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowId {
    LaunchAtLogin,
    StartHidden,
    HoldKey,
    HandsFree,
    PasteLast,
    CommandMode,
    ReadAloud,
    ResetShortcuts,
    Microphone,
    InputGain,
    Enhance,
    Sounds,
    CueVolume,
    MaxLength,
    Rules,
    SpokenPunctuation,
    AiCleanup,
    AiSource,
    AiModel,
    EndpointPreset,
    EndpointUrl,
    EndpointModel,
    EndpointKey,
    EndpointTest,
    Gpu,
    Threads,
    ShowFlowBar,
    FlowBarIdle,
    FlowBarPosition,
    DataFolder,
    Retention,
    UpdateCheck,
    AutoInstall,
    UpdateStatus,
}

pub struct RowMeta {
    pub id: RowId,
    pub section: Section,
    /// The name of the row in the `settings_view` hook state.
    pub key: &'static str,
    pub label: &'static str,
    /// Other words for the row, in lower case: aliases such as "mic" and "input".
    pub keywords: &'static [&'static str],
    /// The row this one shows under, such as the AI source for the endpoint fields. A search
    /// that finds this row shows that one too.
    pub requires: Option<RowId>,
    /// Whether this platform has the row.
    here: bool,
}

impl RowMeta {
    const fn new(
        id: RowId,
        section: Section,
        key: &'static str,
        label: &'static str,
        keywords: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            section,
            key,
            label,
            keywords,
            requires: None,
            here: true,
        }
    }

    const fn under(self, row: RowId) -> Self {
        Self {
            requires: Some(row),
            ..self
        }
    }

    const fn macos_only(self) -> Self {
        Self {
            here: cfg!(target_os = "macos"),
            ..self
        }
    }

    /// Whether the label, the section title, or a keyword holds `word`, which is lower case.
    fn finds(&self, word: &str) -> bool {
        self.label.to_lowercase().contains(word)
            || self.section.title().to_lowercase().contains(word)
            || self.keywords.iter().any(|keyword| keyword.contains(word))
    }
}

const SHORTCUT: &[&str] = &["shortcut", "hotkey", "keyboard", "key", "disable"];
const ENDPOINT: &[&str] = &["endpoint", "server", "openai", "ollama", "lm studio"];

pub static ROWS: &[RowMeta] = &[
    RowMeta::new(
        LaunchAtLogin,
        General,
        "general.launchAtLogin",
        "Launch at login",
        &["login", "startup", "autostart", "boot", "open at login"],
    ),
    RowMeta::new(
        StartHidden,
        General,
        "general.startHidden",
        "Start hidden",
        &["startup", "tray", "menu bar", "background", "minimized"],
    ),
    RowMeta::new(
        HoldKey,
        Shortcuts,
        "shortcut.hold",
        "Hold to dictate",
        SHORTCUT,
    ),
    RowMeta::new(
        HandsFree,
        Shortcuts,
        "shortcut.handsFree",
        "Hands-free",
        SHORTCUT,
    ),
    RowMeta::new(
        PasteLast,
        Shortcuts,
        "shortcut.pasteLast",
        "Paste last transcript",
        SHORTCUT,
    ),
    RowMeta::new(
        CommandMode,
        Shortcuts,
        "shortcut.command",
        "Command mode",
        SHORTCUT,
    ),
    RowMeta::new(
        ReadAloud,
        Shortcuts,
        "shortcut.readAloud",
        "Read aloud",
        &[
            "shortcut",
            "hotkey",
            "keyboard",
            "key",
            "disable",
            "speak",
            "voice",
            "selection",
        ],
    ),
    RowMeta::new(
        ResetShortcuts,
        Shortcuts,
        "shortcut.reset",
        "Default shortcuts",
        &["shortcut", "hotkey", "reset", "restore"],
    ),
    RowMeta::new(
        Microphone,
        Audio,
        "audio.mic",
        "Microphone",
        &["mic", "input", "device", "recording"],
    ),
    RowMeta::new(
        InputGain,
        Audio,
        "audio.inputGain",
        "Input boost",
        &[
            "mic",
            "microphone",
            "input",
            "gain",
            "boost",
            "volume",
            "louder",
            "quiet",
            "level",
        ],
    ),
    RowMeta::new(
        Enhance,
        Audio,
        "audio.enhance",
        "Audio enhance",
        &[
            "enhance",
            "equalizer",
            "eq",
            "bass",
            "treble",
            "low cut",
            "noise",
            "clearer",
            "accent",
        ],
    ),
    RowMeta::new(
        Sounds,
        Audio,
        "audio.sounds",
        "Sounds",
        &["cue", "beep", "chime", "sound"],
    ),
    RowMeta::new(
        CueVolume,
        Audio,
        "audio.cueVolume",
        "Sound volume",
        &["cue", "volume", "loud", "beep", "sound"],
    ),
    RowMeta::new(
        MaxLength,
        Audio,
        "audio.maxMinutes",
        "Maximum length",
        &[
            "maximum",
            "limit",
            "length",
            "minutes",
            "duration",
            "recording",
        ],
    ),
    RowMeta::new(
        Rules,
        Cleanup,
        "cleanup.rules",
        "Clean up text",
        &["filler", "um", "stutter", "rules", "format"],
    ),
    RowMeta::new(
        SpokenPunctuation,
        Cleanup,
        "cleanup.spokenPunctuation",
        "Spoken punctuation",
        &["comma", "period", "question mark", "new line"],
    ),
    RowMeta::new(
        AiCleanup,
        Cleanup,
        "cleanup.ai",
        "AI cleanup",
        &["llm", "language model"],
    ),
    RowMeta::new(
        AiSource,
        Cleanup,
        "cleanup.provider",
        "AI source",
        &["provider", "llm", "local", "endpoint", "server"],
    ),
    RowMeta::new(
        AiModel,
        Cleanup,
        "cleanup.aiModel",
        "AI model",
        &["llm", "download", "gemma", "qwen"],
    )
    .under(AiSource),
    RowMeta::new(
        EndpointPreset,
        Cleanup,
        "cleanup.endpointPreset",
        "Endpoint",
        ENDPOINT,
    )
    .under(AiSource),
    RowMeta::new(
        EndpointUrl,
        Cleanup,
        "cleanup.endpointUrl",
        "Address",
        &["endpoint", "url", "host", "server"],
    )
    .under(AiSource),
    RowMeta::new(
        EndpointModel,
        Cleanup,
        "cleanup.endpointModel",
        "Model",
        &["endpoint", "model name"],
    )
    .under(AiSource),
    RowMeta::new(
        EndpointKey,
        Cleanup,
        "cleanup.endpointKey",
        "API key",
        &[
            "endpoint",
            "key",
            "api",
            "credential",
            "token",
            "secret",
            "password",
        ],
    )
    .under(AiSource),
    RowMeta::new(
        EndpointTest,
        Cleanup,
        "cleanup.endpointTest",
        "Test connection",
        &["endpoint", "check", "connection"],
    )
    .under(AiSource),
    RowMeta::new(
        Gpu,
        Engine,
        "engine.gpu",
        "Use the GPU",
        &["gpu", "metal", "graphics", "acceleration", "speed"],
    )
    // The Linux engine is built for the CPU only (D-19), so the switch would do nothing there.
    .macos_only(),
    RowMeta::new(
        Threads,
        Engine,
        "engine.threads",
        "Threads",
        &["cpu", "cores", "threads", "speed", "performance"],
    ),
    RowMeta::new(
        ShowFlowBar,
        FlowBar,
        "flowbar.enabled",
        "Show the flow bar",
        &["overlay", "pill", "hud", "floating"],
    ),
    RowMeta::new(
        FlowBarIdle,
        FlowBar,
        "flowbar.idle",
        "Show when idle",
        &["overlay", "always", "visible"],
    ),
    RowMeta::new(
        FlowBarPosition,
        FlowBar,
        "flowbar.position",
        "Position",
        &["overlay", "top", "bottom", "place"],
    ),
    RowMeta::new(
        DataFolder,
        Storage,
        "storage.dataFolder",
        "Data folder",
        &["location", "path", "directory", "move", "disk"],
    ),
    RowMeta::new(
        Retention,
        Storage,
        "storage.retention",
        "Keep dictation audio",
        &["retention", "delete", "recordings", "history", "days"],
    ),
    RowMeta::new(
        UpdateCheck,
        Updates,
        "updates.check",
        "Check for updates",
        &["update", "version", "release", "github"],
    ),
    RowMeta::new(
        AutoInstall,
        Updates,
        "updates.autoInstall",
        "Install updates automatically",
        &["update", "auto", "download"],
    ),
    RowMeta::new(
        UpdateStatus,
        Updates,
        "updates.status",
        "Check now",
        &["update", "version", "release notes", "restart"],
    ),
];

/// The rows a query shows, in page order: every row of this platform whose label, section, or
/// keywords hold each word of the query, and the rows those show under. An empty query shows
/// every row.
pub fn visible(query: &str) -> Vec<RowId> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let found: Vec<&RowMeta> = ROWS
        .iter()
        .filter(|meta| meta.here && words.iter().all(|word| meta.finds(word)))
        .collect();
    ROWS.iter()
        .filter(|meta| {
            found
                .iter()
                .any(|hit| hit.id == meta.id || hit.requires == Some(meta.id))
        })
        .map(|meta| meta.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(query: &str) -> Vec<&'static str> {
        visible(query)
            .into_iter()
            .filter_map(|id| ROWS.iter().find(|meta| meta.id == id))
            .map(|meta| meta.key)
            .collect()
    }

    #[test]
    fn an_empty_query_shows_every_row_of_this_platform_in_page_order() {
        let here = ROWS.iter().filter(|meta| meta.here).count();
        assert_eq!(visible("   ").len(), here);
        let order: Vec<usize> = ROWS
            .iter()
            .map(|meta| {
                Section::ALL
                    .iter()
                    .position(|s| *s == meta.section)
                    .unwrap_or(99)
            })
            .collect();
        assert!(order.windows(2).all(|pair| pair[0] <= pair[1]), "{order:?}");
    }

    #[test]
    fn a_label_matches_whatever_its_case() {
        assert_eq!(keys("LAUNCH"), ["general.launchAtLogin"]);
    }

    #[test]
    fn aliases_find_their_rows() {
        assert_eq!(keys("mic"), ["audio.mic", "audio.inputGain"]);
        assert_eq!(keys("input"), ["audio.mic", "audio.inputGain"]);
        assert_eq!(keys("boost"), ["audio.inputGain"]);
        assert_eq!(keys("gain"), ["audio.inputGain"]);
        assert_eq!(keys("autostart"), ["general.launchAtLogin"]);
        assert_eq!(
            keys("credential"),
            ["cleanup.provider", "cleanup.endpointKey"]
        );
        assert!(
            keys("hotkey")
                .iter()
                .all(|key| key.starts_with("shortcut."))
        );
        assert_eq!(keys("hotkey").len(), 6);
        assert_eq!(keys("speak"), ["shortcut.readAloud"]);
        for alias in [
            "enhance",
            "equalizer",
            "eq",
            "bass",
            "treble",
            "low cut",
            "noise",
        ] {
            assert_eq!(keys(alias), ["audio.enhance"], "{alias}");
        }
    }

    #[test]
    fn a_section_title_shows_the_whole_section() {
        assert_eq!(
            keys("flow bar"),
            ["flowbar.enabled", "flowbar.idle", "flowbar.position"]
        );
    }

    #[test]
    fn every_word_must_match() {
        assert_eq!(keys("api key"), ["cleanup.provider", "cleanup.endpointKey"]);
        assert!(keys("microphone gpu").is_empty());
    }

    #[test]
    fn a_row_under_another_brings_that_one_along() {
        assert_eq!(
            keys("ollama"),
            ["cleanup.provider", "cleanup.endpointPreset"]
        );
        assert_eq!(
            keys("endpoint"),
            [
                "cleanup.provider",
                "cleanup.endpointPreset",
                "cleanup.endpointUrl",
                "cleanup.endpointModel",
                "cleanup.endpointKey",
                "cleanup.endpointTest"
            ]
        );
    }

    #[test]
    fn the_gpu_row_is_found_on_macos_only() {
        let expected: &[&str] = if cfg!(target_os = "macos") {
            &["engine.gpu"]
        } else {
            &[]
        };
        assert_eq!(keys("gpu"), expected);
    }

    #[test]
    fn nothing_matches_a_word_no_row_knows() {
        assert!(visible("zebra").is_empty());
    }

    #[test]
    fn keys_and_ids_are_unique() {
        for (index, meta) in ROWS.iter().enumerate() {
            assert!(
                ROWS[index + 1..]
                    .iter()
                    .all(|other| other.key != meta.key && other.id != meta.id),
                "{}",
                meta.key
            );
        }
    }
}
