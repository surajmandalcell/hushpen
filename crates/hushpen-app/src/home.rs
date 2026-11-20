//! Home: the tab row of the title bar, the number tiles, the words-per-day map, and the five
//! newest transcripts. The numbers come from the daily totals of the history store, so they
//! stay after Clear all. They are read on this Mac and never sent anywhere.

use crate::files::{Files, Mode};
use crate::history::{History, panel};
use crate::hook;
use crate::motion;
use crate::shell::Shell;
use crate::theme::{self, HEADING, Palette, SECONDARY, StyledType, TITLE, radius, size, space};
use crate::ui;
use crate::views::View;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    AnyElement, App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, FontFeatures,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, Rgba, Role,
    StatefulInteractiveElement, Styled, Window, div, px, svg,
};
use hushpen_store::history::{self, DayTotal, Row};
use std::sync::Arc;
use std::time::Duration;

/// The typing speed the time saved compares with, in words per minute.
const TYPING_WPM: f64 = 40.0;
/// How many transcripts the Recent group shows.
const RECENT: usize = 5;
/// How many weeks the map shows, the current one last: as many as fill the fixed-width window.
const WEEKS: i64 = 37;
const CELL: f32 = 14.0;
const CELL_GAP: f32 = 3.0;
const CELL_RADIUS: f32 = 3.0;
/// Words in a day at which a cell takes the next stronger fill.
const LEVELS: [i64; 3] = [250, 550, 850];
const TAB_WIDTH: f32 = 92.0;
const COPY_WIDTH: f32 = 24.0;
const COUNT_UP: Duration = Duration::from_millis(900);
/// A map cell fades in over `CELL_IN`, each week column `CELL_STEP` after the one before.
const CELL_IN: f32 = 320.0;
const CELL_STEP: f32 = 14.0;
const COPIED_FOR: Duration = Duration::from_millis(1500);
const ROW_GROUP: &str = "home-recent-row";

pub enum HomeRequest {
    ShowHistory,
    Open(String),
}

pub struct Home {
    history: Entity<History>,
    more_focus: FocusHandle,
    copy_focus: Vec<FocusHandle>,
    /// The history revision and row count the numbers were read at.
    seen: Option<(u64, i64)>,
    days: Vec<DayTotal>,
    today: i64,
    recent: Vec<Row>,
    copied: Option<String>,
    copy_generation: u64,
}

impl EventEmitter<HomeRequest> for Home {}

impl Home {
    pub fn new(history: Entity<History>, cx: &mut Context<Self>) -> Self {
        cx.observe(&history, |home, _, cx| home.follow(cx)).detach();
        let mut home = Self {
            history,
            more_focus: cx.focus_handle().tab_stop(true),
            copy_focus: (0..RECENT)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            seen: None,
            days: Vec::new(),
            today: 0,
            recent: Vec::new(),
            copied: None,
            copy_generation: 0,
        };
        home.follow(cx);
        home
    }

    /// Reads the totals and the newest rows again, only when the history changed.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let history = self.history.read(cx);
        let key = (
            history.controller().read(cx).history_revision(),
            history.total(),
        );
        if self.seen == Some(key) {
            return;
        }
        let db = &history.storage().database;
        let days = history::daily_totals(db);
        let today = history::today(db);
        let page = history::page(db, "", None, RECENT * 4);
        self.seen = Some(key);
        match (days, today) {
            (Ok(days), Ok(today)) => (self.days, self.today) = (days, today),
            (Err(error), _) | (_, Err(error)) => log::warn!("could not read the totals: {error}"),
        }
        match page {
            Ok(page) => {
                self.recent = page
                    .rows
                    .into_iter()
                    .filter(|row| row.kind != "read" && row.status == "completed")
                    .take(RECENT)
                    .collect();
            }
            Err(error) => log::warn!("could not read the newest transcripts: {error}"),
        }
        cx.notify();
    }

    fn copy(&mut self, id: String, cx: &mut Context<Self>) {
        let copied = self
            .history
            .update(cx, |history, cx| history.copy(Some(&id), cx));
        if copied.is_err() {
            return;
        }
        self.copied = Some(id);
        self.copy_generation += 1;
        let generation = self.copy_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            let _ = this.update(cx, |home, cx| {
                if home.copy_generation == generation {
                    home.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
}

/// The numbers of the tiles, from the daily totals.
#[derive(Debug, Default, PartialEq)]
struct Stats {
    words: i64,
    words_week: i64,
    speech_ms: i64,
    streak: i64,
    best: i64,
}

fn stats(days: &[DayTotal], today: i64) -> Stats {
    let mut stats = Stats::default();
    let mut run = 0;
    let mut last = None;
    for day in days {
        stats.words += day.words;
        stats.speech_ms += day.speech_ms;
        if day.day > today - 7 {
            stats.words_week += day.words;
        }
        run = if last == Some(day.day - 1) {
            run + 1
        } else {
            1
        };
        stats.best = stats.best.max(run);
        last = Some(day.day);
    }
    // A streak still counts today before the first dictation of the day.
    if last.is_some_and(|last| last >= today - 1) {
        stats.streak = run;
    }
    stats
}

impl Stats {
    fn wpm(&self) -> f64 {
        if self.speech_ms == 0 {
            return 0.0;
        }
        self.words as f64 / (self.speech_ms as f64 / 60_000.0)
    }

    fn saved_minutes(&self) -> f64 {
        (self.words as f64 / TYPING_WPM - self.speech_ms as f64 / 60_000.0).max(0.0)
    }
}

/// 12480 as "12,480".
fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{} {}", grouped(n), if n == 1 { one } else { many })
}

/// The time saved as a number and its unit, such as ("18.6", "h").
fn saved(minutes: f64) -> (String, String) {
    if minutes < 60.0 {
        ((minutes.round() as i64).to_string(), "min".to_owned())
    } else {
        (format!("{:.1}", minutes / 60.0), "h".to_owned())
    }
}

/// The local day of a time in milliseconds, as a [`DayTotal::day`].
fn local_day(created_at: i64) -> i64 {
    let seconds = created_at.div_euclid(1000);
    (seconds + hushpen_platform::local_time::utc_offset_seconds(seconds)).div_euclid(86_400)
}

/// "Today 09:42 · Slack · 6.2 s".
fn recent_meta(row: &Row, today: i64) -> String {
    let stamp = panel::when(row.created_at);
    let day = local_day(row.created_at);
    let when = match today - day {
        0 => format!("Today {}", &stamp[11..]),
        1 => format!("Yesterday {}", &stamp[11..]),
        _ => stamp,
    };
    let mut parts = vec![when];
    if row.kind == "import" {
        parts.push(panel::FILE_SOURCE.to_owned());
    } else if let Some(app) = &row.target_app {
        parts.push(hushpen_platform::app_name::display_name(app));
    }
    if row.duration_ms > 0 {
        parts.push(panel::seconds(row.duration_ms));
    }
    parts.join(" \u{b7} ")
}

fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

fn faded(color: Rgba, alpha: f32) -> Rgba {
    Rgba {
        a: color.a * alpha,
        ..color
    }
}

/// The Home | Transcribe | Read aloud tabs at the right of the title bar, on Home and on the
/// Manual view behind the two other tabs. The control keeps the hook id of the old mode
/// control, so `files.mode.transcribe` and `files.mode.readAloud` still name the same tabs.
pub fn tabs(
    shell: &Entity<Shell>,
    files: Option<&Entity<Files>>,
    active: View,
    focus: &FocusHandle,
    cx: &App,
) -> Option<impl IntoElement + use<>> {
    let files = files?.read(cx);
    let mut choices = vec![("home", "Home"), (Mode::Transcribe.key(), "Transcribe")];
    if files.read_aloud().is_some() {
        choices.push((Mode::ReadAloud.key(), "Read aloud"));
    }
    let chosen = if active == View::Home {
        "home"
    } else {
        files.mode().key()
    };
    let shell = shell.clone();
    Some(ui::segmented(
        theme::palette(cx),
        hook::id("files", "mode"),
        "Home tabs",
        &choices,
        TAB_WIDTH,
        chosen,
        focus,
        move |key, _, cx| shell.update(cx, |shell, cx| shell.open_tab(key, cx)),
    ))
}

impl Render for Home {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let stats = stats(&self.days, self.today);
        let count = motion::enter("home.count", COUNT_UP, window, cx);
        let map_length = Duration::from_millis((CELL_IN + CELL_STEP * (WEEKS - 1) as f32) as u64);
        // `enter` eases; the stagger needs the plain time again.
        let map_time = 1.0 - (1.0 - motion::enter("home.map", map_length, window, cx)).cbrt();
        div()
            .id(hook::id("home", "view"))
            .test_support()
            .flex()
            .flex_col()
            .gap(px(space::XXL))
            .w_full()
            .child(self.tiles(palette, &stats, count))
            .child(self.map(palette, map_time * map_length.as_millis() as f32))
            .child(self.recent_list(palette, cx))
    }
}

impl Home {
    fn tiles(&self, palette: Palette, stats: &Stats, count: f32) -> impl IntoElement {
        let up = |n: i64| (n as f32 * count).round() as i64;
        let wpm = stats.wpm();
        let streak = up(stats.streak);
        let (saved_number, saved_unit) = saved(stats.saved_minutes() * f64::from(count));
        // (hook key, icon, label, number, unit, secondary line), as in the owner pick B1.
        let tiles = [
            (
                "words",
                IconName::TextAlignStart,
                "Words",
                grouped(up(stats.words)),
                String::new(),
                format!("{} this week", grouped(stats.words_week)),
            ),
            (
                "speed",
                IconName::Gauge,
                "Speed",
                ((wpm * f64::from(count)).round() as i64).to_string(),
                "wpm".to_owned(),
                format!("{:.1}x your typing", wpm / TYPING_WPM),
            ),
            (
                "streak",
                IconName::Flame,
                "Streak",
                grouped(streak),
                if streak == 1 { "day" } else { "days" }.to_owned(),
                format!("Best: {}", plural(stats.best, "day", "days")),
            ),
            (
                "saved",
                IconName::Hourglass,
                "Time saved",
                saved_number,
                saved_unit,
                format!("vs typing at {TYPING_WPM} wpm"),
            ),
        ];
        div()
            .flex()
            .gap(px(space::MD))
            .w_full()
            .children(
                tiles
                    .into_iter()
                    .map(|(key, icon, label, number, unit, sub)| {
                        div()
                            .id(hook::id("home", key))
                            .test_support()
                            .role(Role::Group)
                            .aria_label(format!("{label}: {number} {unit}"))
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(space::XS))
                            .p(px(space::MD))
                            .bg(palette.group)
                            .rounded(px(radius::GROUP))
                            .border(theme::hairline())
                            .border_color(palette.separator)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.0))
                                    .min_w_0()
                                    .text_token(SECONDARY)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(palette.label_secondary)
                                    .child(
                                        svg()
                                            .path(icon.path())
                                            .size(px(13.0))
                                            .flex_none()
                                            .text_color(palette.label_secondary),
                                    )
                                    .child(div().min_w_0().truncate().child(label)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap(px(4.0))
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_token(TITLE)
                                            .font_features(tabular())
                                            .text_color(palette.label)
                                            .truncate()
                                            .child(number),
                                    )
                                    .children(
                                        (!unit.is_empty()).then(|| {
                                            ui::secondary(palette).flex_none().child(unit)
                                        }),
                                    ),
                            )
                            .child(ui::secondary(palette).child(sub))
                    }),
            )
    }

    /// One column a week, Monday on top, this week last. `elapsed` is how far the draw-in has
    /// come, in milliseconds.
    fn map(&self, palette: Palette, elapsed: f32) -> impl IntoElement {
        let empty = palette.control;
        let dark = palette.appearance.is_dark();
        let level = |words: i64| -> Rgba {
            let alpha = match LEVELS.iter().position(|limit| words < *limit) {
                Some(0) => {
                    if dark {
                        0.28
                    } else {
                        0.22
                    }
                }
                Some(1) => {
                    if dark {
                        0.5
                    } else {
                        0.45
                    }
                }
                Some(_) => {
                    if dark {
                        0.75
                    } else {
                        0.7
                    }
                }
                None => 1.0,
            };
            faded(palette.accent, alpha)
        };
        // 1970-01-01 was a Thursday, so day + 3 counts from a Monday.
        let start = self.today - (self.today + 3).rem_euclid(7) - (WEEKS - 1) * 7;
        let words_on = |day: i64| {
            self.days
                .binary_search_by_key(&day, |total| total.day)
                .map_or(0, |at| self.days[at].words)
        };
        let columns = (0..WEEKS).map(|week| {
            let shown = ((elapsed - week as f32 * CELL_STEP) / CELL_IN).clamp(0.0, 1.0);
            div()
                .flex()
                .flex_col()
                .gap(px(CELL_GAP))
                .flex_none()
                .children((0..7).map(move |weekday| {
                    let day = start + week * 7 + weekday;
                    let words = words_on(day);
                    let cell = div().size(px(CELL)).rounded(px(CELL_RADIUS));
                    if day > self.today {
                        return cell;
                    }
                    let cell = cell.bg(if words > 0 { level(words) } else { empty });
                    if shown < 1.0 {
                        cell.opacity(shown)
                    } else {
                        cell
                    }
                }))
        });
        let today_words = words_on(self.today);
        let legend_cell = |fill: Rgba| div().size(px(10.0)).rounded(px(2.0)).bg(fill);
        div()
            .id(hook::id("home", "map"))
            .test_support()
            .role(Role::Group)
            .aria_label("Words per day")
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .w_full()
            .p(px(space::MD))
            .bg(palette.group)
            .rounded(px(radius::GROUP))
            .border(theme::hairline())
            .border_color(palette.separator)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_token(HEADING)
                            .text_color(palette.label)
                            .child("Words per day"),
                    )
                    .child(ui::secondary(palette).child(format!("Last {WEEKS} weeks"))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap(px(CELL_GAP))
                    .w_full()
                    .overflow_hidden()
                    .children(columns),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_token(SECONDARY)
                    .text_color(palette.label_secondary)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::XS))
                            .child("Less")
                            .child(legend_cell(empty))
                            .child(legend_cell(level(LEVELS[1])))
                            .child(legend_cell(level(LEVELS[2] + 1)))
                            .child("More"),
                    )
                    .child(
                        div()
                            .font_features(tabular())
                            .child(format!("Today: {}", plural(today_words, "word", "words"))),
                    ),
            )
    }

    fn recent_list(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let more = div()
            .id(hook::id("home", "more"))
            .test_support()
            .role(Role::Link)
            .aria_label("Show more")
            .track_focus(&self.more_focus)
            .flex_none()
            .cursor_pointer()
            .text_token(SECONDARY)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(palette.accent)
            .hover(|style| style.underline())
            .focus_visible(|style| style.underline())
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(HomeRequest::ShowHistory)))
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.emit(HomeRequest::ShowHistory);
                    cx.stop_propagation();
                }
            }))
            .child("Show more");
        let header = div()
            .flex()
            .justify_between()
            .items_center()
            .px(px(size::ROW_PADDING))
            .child(
                div()
                    .text_token(SECONDARY)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.label_secondary)
                    .child("Recent"),
            )
            .child(more);
        let rows: Vec<AnyElement> = if self.recent.is_empty() {
            vec![
                ui::row(palette)
                    .child(ui::secondary(palette).child("Your dictations show up here."))
                    .into_any_element(),
            ]
        } else {
            self.recent
                .iter()
                .enumerate()
                .map(|(index, row)| self.recent_row(palette, index, row, cx))
                .collect()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(space::XS + space::XXS))
            .w_full()
            .child(header)
            .child(ui::group(hook::id("home", "recent"), palette, rows))
    }

    fn recent_row(
        &self,
        palette: Palette,
        index: usize,
        row: &Row,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let copied = self.copied.as_deref() == Some(row.id.as_str());
        let id = row.id.clone();
        let copy = div()
            .id(hook::indexed("home", "copy", index))
            .test_support()
            .role(Role::Button)
            .aria_label(if copied { "Copied" } else { "Copy" })
            .track_focus(&self.copy_focus[index])
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .w(px(COPY_WIDTH))
            .h(px(size::BUTTON_HEIGHT))
            .rounded(px(radius::BUTTON))
            .bg(palette.control)
            .cursor_pointer()
            .child(
                svg()
                    .path(
                        if copied {
                            IconName::Check
                        } else {
                            IconName::Copy
                        }
                        .path(),
                    )
                    .size(px(size::QUIET_ICON))
                    .text_color(if copied {
                        palette.success
                    } else {
                        palette.label
                    }),
            )
            .on_click(cx.listener({
                let id = id.clone();
                move |home, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    home.copy(id.clone(), cx);
                }
            }))
            .on_key_down(cx.listener({
                let id = id.clone();
                move |home, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        cx.stop_propagation();
                        home.copy(id.clone(), cx);
                    }
                }
            }));
        // The button shows under the pointer, under keyboard focus, and while it says Copied.
        let copy = if copied {
            copy
        } else {
            copy.invisible()
                .group_hover(ROW_GROUP, |style| style.visible())
                .focus_visible(|style| style.visible())
        };
        div()
            .id(hook::indexed("home", "row", index))
            .test_support()
            .group(ROW_GROUP)
            .flex()
            .items_center()
            .gap(px(space::SM))
            .w_full()
            .min_h(px(size::ROW_MIN_HEIGHT))
            .px(px(size::ROW_PADDING))
            .py(px(space::SM))
            .cursor_pointer()
            .hover(move |style| style.bg(palette.hover()))
            .on_click(
                cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(HomeRequest::Open(id.clone()))),
            )
            .child(
                ui::text_column()
                    .child(ui::title(palette).child(panel::title(row)))
                    .child(ui::secondary(palette).child(recent_meta(row, self.today))),
            )
            .child(copy)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(day: i64, words: i64) -> DayTotal {
        DayTotal {
            day,
            words,
            dictations: 1,
            speech_ms: 60_000,
        }
    }

    #[test]
    fn the_streak_runs_to_today_or_yesterday_and_the_best_run_stays() {
        let days = [day(1, 10), day(2, 10), day(3, 10), day(9, 5), day(10, 5)];
        let today = stats(&days, 10);
        assert_eq!((today.streak, today.best), (2, 3));
        assert_eq!(stats(&days, 11).streak, 2, "no dictation yet today");
        assert_eq!(stats(&days, 12).streak, 0);
        assert_eq!((today.words, today.words_week), (40, 10));
        assert_eq!(saved(45.0), ("45".to_owned(), "min".to_owned()));
        assert_eq!(saved(90.0), ("1.5".to_owned(), "h".to_owned()));
        assert_eq!(grouped(12_480), "12,480");
        assert_eq!(grouped(999), "999");
    }
}
