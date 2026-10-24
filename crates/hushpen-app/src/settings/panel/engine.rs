//! Engine: the GPU (on macOS, where the engine has Metal) and the thread count of the speech
//! engine.

use super::Page;
use crate::settings::{Settings, slots};
use gpui_kit::{AnyElement, App};

/// The thread counts offered besides Automatic, up to the CPUs of this computer.
const THREAD_CHOICES: [i64; 7] = [1, 2, 4, 6, 8, 12, 16];

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

pub fn threads(page: &Page, cx: &mut App) -> Vec<AnyElement> {
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
    let mut rows = vec![page.menu(
        "engine.threads",
        "Threads",
        "The CPU threads the speech engine uses.",
        current,
        slots::THREADS,
    )];
    if page.picker_open("engine.threads", cx) {
        let choices = std::iter::once(("auto".to_owned(), automatic, stored.is_none()))
            .chain(counts.into_iter().map(|count| {
                (
                    count.to_string(),
                    threads_label(count),
                    Some(count) == stored,
                )
            }))
            .collect();
        rows.extend(
            page.options("engine.threads", choices, &page.options, |me, value, cx| {
                me.set_threads(value.parse().ok(), cx)
            }),
        );
    }
    rows
}

fn threads_label(count: i64) -> String {
    match count {
        1 => "1 thread".to_owned(),
        count => format!("{count} threads"),
    }
}
