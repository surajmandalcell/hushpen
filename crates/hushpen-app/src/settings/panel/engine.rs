//! Engine: the GPU (on macOS, where the engine has Metal) and the thread count of the speech
//! engine.

use super::Page;
use crate::settings::{Settings, slots};
use gpui_kit::{AnyElement, App};

/// The thread counts offered besides Automatic, up to the CPUs of this computer.
const THREAD_CHOICES: [i64; 7] = [1, 2, 4, 6, 8, 12, 16];

const IDLE_CHOICES: [(&str, &str); 3] = [
    ("keep", "Keep in memory"),
    ("pageout", "Let the system page out"),
    ("unload", "Unload"),
];

pub fn idle_policy(page: &Page, cx: &mut App) -> AnyElement {
    let stored = page.settings.read(cx).stored_str("engine.idlePolicy");
    let current = IDLE_CHOICES
        .iter()
        .find(|(key, _)| *key == stored)
        .map_or("Current behavior", |(_, label)| *label);
    let detail = match stored.as_str() {
        "keep" => "Keeps models loaded until you quit or choose another model.",
        "pageout" if cfg!(target_os = "macos") => {
            "macOS decides when idle memory is compressed or swapped."
        }
        "pageout" => "Speech and cleanup request pageout; voice paging is system-managed.",
        "unload" => "Releases idle models. The next use loads them again.",
        _ => "Speech stays loaded; cleanup and voices keep their current idle limits.",
    };
    let menu = page.picker_open("engine.idlePolicy", cx).then(|| {
        page.options(
            "engine.idlePolicy",
            IDLE_CHOICES
                .into_iter()
                .map(|(key, label)| (key.to_owned(), label.to_owned(), stored == key))
                .collect(),
            &page.options,
            Settings::set_idle_policy,
        )
    });
    page.menu(
        "engine.idlePolicy",
        "When idle",
        detail,
        current.to_owned(),
        slots::IDLE_POLICY,
        menu,
    )
}

pub fn idle_minutes(page: &Page, cx: &mut App) -> AnyElement {
    let me = page.settings.read(cx);
    let stored = me.stored_f64("engine.idleMinutes").unwrap_or(15.0) as i64;
    let policy = me.stored_str("engine.idlePolicy");
    let detail = match policy.as_str() {
        "pageout" if cfg!(target_os = "macos") => {
            "macOS controls paging. This time applies only to Unload."
        }
        "pageout" | "unload" => "Counted from the end of the last model job.",
        _ => "Used when you choose page out or unload.",
    };
    let menu = page.picker_open("engine.idleMinutes", cx).then(|| {
        let mut values = vec![5, 15, 30, 60, stored];
        values.sort_unstable();
        values.dedup();
        page.options(
            "engine.idleMinutes",
            values
                .into_iter()
                .map(|minutes| {
                    (
                        minutes.to_string(),
                        format!("{minutes} minutes"),
                        minutes == stored,
                    )
                })
                .collect(),
            &page.options,
            |me, value, cx| {
                if let Ok(minutes) = value.parse() {
                    me.set_idle_minutes(minutes, cx);
                }
            },
        )
    });
    page.menu(
        "engine.idleMinutes",
        "Idle time",
        detail,
        format!("{stored} minutes"),
        slots::IDLE_MINUTES,
        menu,
    )
}

pub fn gpu(page: &Page, cx: &mut App) -> AnyElement {
    let on = page.settings.read(cx).stored_bool("engine.useGpu");
    page.toggle(
        "engine.gpu",
        "Use the GPU",
        "Metal runs the speech and AI models faster.",
        on,
        slots::GPU,
        Settings::toggle_gpu,
    )
}

pub fn threads(page: &Page, cx: &mut App) -> AnyElement {
    let cpus = std::thread::available_parallelism().map_or(2, usize::from);
    // `auto` is text, so only a stored count reads as a number.
    let stored = page
        .settings
        .read(cx)
        .stored_f64("engine.threads")
        .map(|count| count as i64);
    let range = hushpen_store::settings::range("engine.threads");
    let mut counts: Vec<i64> = THREAD_CHOICES
        .into_iter()
        .filter(|count| usize::try_from(*count).is_ok_and(|count| count <= cpus))
        .chain(stored)
        .filter(|count| {
            range
                .as_ref()
                .is_some_and(|range| range.contains(&(*count as f64)))
        })
        .collect();
    counts.sort_unstable();
    counts.dedup();
    let automatic = format!("Automatic ({})", hushpen_core::threads::auto_threads(cpus));
    let current = stored.map_or_else(|| automatic.clone(), threads_label);
    let menu = page.picker_open("engine.threads", cx).then(|| {
        let choices = std::iter::once(("auto".to_owned(), automatic, stored.is_none()))
            .chain(counts.into_iter().map(|count| {
                (
                    count.to_string(),
                    threads_label(count),
                    Some(count) == stored,
                )
            }))
            .collect();
        page.options("engine.threads", choices, &page.options, |me, value, cx| {
            me.set_threads(value.parse().ok(), cx)
        })
    });
    page.menu(
        "engine.threads",
        "Threads",
        "The CPU threads the speech engine uses.",
        current,
        slots::THREADS,
        menu,
    )
}

fn threads_label(count: i64) -> String {
    match count {
        1 => "1 thread".to_owned(),
        count => format!("{count} threads"),
    }
}
