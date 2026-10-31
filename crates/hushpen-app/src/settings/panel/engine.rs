//! Engine: the GPU (on macOS, where the engine has Metal) and the thread count of the speech
//! engine.

use super::Page;
use crate::settings::{Settings, slots};
use gpui_kit::{AnyElement, App};
use hushpen_core::idle::Policy;

/// The thread counts offered besides Automatic, up to the CPUs of this computer.
const THREAD_CHOICES: [i64; 7] = [1, 2, 4, 6, 8, 12, 16];

/// The choices of this system with the line under the row for each.
const IDLE_CHOICES: &[(Policy, &str, &str)] = &[
    (
        Policy::Keep,
        "Keep in memory",
        if Policy::CAN_PAGE_OUT {
            "Stays loaded in memory."
        } else {
            "Stays loaded. macOS swaps it out when memory runs low."
        },
    ),
    #[cfg(not(target_os = "macos"))]
    (
        Policy::PageOut,
        "Page out",
        "After the idle time, the model moves to swap and stays loaded.",
    ),
    (
        Policy::Unload,
        "Unload",
        "Frees the memory after the idle time. The next use loads it again.",
    ),
];

fn stored_policy(page: &Page, cx: &App) -> Policy {
    let stored = page.settings.read(cx).stored_str("engine.idlePolicy");
    Policy::parse(Some(&stored))
}

pub fn idle_policy(page: &Page, cx: &mut App) -> AnyElement {
    let policy = stored_policy(page, cx);
    let (_, label, detail) = IDLE_CHOICES
        .iter()
        .find(|(choice, ..)| *choice == policy)
        .copied()
        .unwrap_or(IDLE_CHOICES[0]);
    let menu = page.picker_open("engine.idlePolicy", cx).then(|| {
        page.options(
            "engine.idlePolicy",
            IDLE_CHOICES
                .iter()
                .map(|(choice, label, _)| {
                    (
                        choice.as_str().to_owned(),
                        (*label).to_owned(),
                        *choice == policy,
                    )
                })
                .collect(),
            &page.options,
            Settings::set_idle_policy,
        )
    });
    page.menu(
        "engine.idlePolicy",
        "When idle",
        detail,
        label.to_owned(),
        slots::IDLE_POLICY,
        menu,
    )
}

/// The idle time counts only for Page out and Unload, so Keep in memory hides the row.
pub fn idle_minutes(page: &Page, cx: &mut App) -> Vec<AnyElement> {
    if stored_policy(page, cx) == Policy::Keep {
        return Vec::new();
    }
    let stored = page
        .settings
        .read(cx)
        .stored_f64("engine.idleMinutes")
        .unwrap_or(15.0) as i64;
    let detail = "Counted from the end of the last dictation or read-aloud.";
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
    vec![page.menu(
        "engine.idleMinutes",
        "Idle time",
        detail,
        format!("{stored} minutes"),
        slots::IDLE_MINUTES,
        menu,
    )]
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
