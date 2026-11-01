//! How the flow bar moves. [`Motion::frame`] steps the bar's size, its coming and going, the
//! change from one state to the next, and the waveform from the bar's own clock, and dresses
//! the props of the state for that frame. A test drives every frame with plain numbers, and the
//! pixel tests draw any moment.
//!
//! A change of state is one calm move: the size eases to the new state with a critically damped
//! spring, the old content fades out before the new content fades in, a waveform turns into
//! the next one in place, and the marks and round buttons of the new state come in once the
//! size has settled.

use super::view::{self, Props, WaveBar};
use gpui_kit::{SpringConfig, SpringState};

/// How much of the motion plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Every change shows at once. For the tests of what the bar shows.
    Instant,
    /// Reduce Motion: nothing moves or scales. The content crossfades, and the size changes
    /// while the pill is empty between the two.
    Crossfade,
    Full,
}

/// The size follows a new state with a critically damped spring: it never overshoots, is 95%
/// there after about 210 ms, and settles in about 300 ms.
const SIZE: SpringConfig = SpringConfig::new(500.0, 44.721_36, 1.0);
/// The bar comes and goes over this time: its opacity and a small scale, eased out.
pub const PRESENCE_MS: f32 = 160.0;
/// The scale of the bar as it starts to come, and as it is gone.
pub const SCALE_FROM: f32 = 0.96;
/// The old content fades out over this time, then the new content fades in over
/// [`IN_MS`], so the two never show at once.
pub const OUT_MS: f32 = 100.0;
pub const IN_MS: f32 = 160.0;
/// The marks and round buttons of a new state come in once its size has settled: they fade
/// in and grow from [`MARK_FROM`] of their size.
pub const MARKS_AFTER_MS: f32 = 240.0;
pub const MARKS_MS: f32 = 180.0;
pub const MARK_FROM: f32 = 0.9;
/// A waveform turns into the next one over this time.
pub const MORPH_MS: f32 = 280.0;
/// A waveform bar rises fast and falls slower, so speech reads as a pulse and not as noise.
const WAVE_RISE_MS: f32 = 40.0;
const WAVE_FALL_MS: f32 = 140.0;
/// A step longer than this is a stall (a sleep, a busy main thread): the motion jumps on
/// instead of replaying it.
const MAX_STEP_S: f32 = 0.05;
/// One frame at 60 frames a second.
const FRAME_S: f32 = 1.0 / 60.0;
/// Half a logical pixel is less than any frame change a user can see.
const SETTLED_PX: f32 = 0.5;

/// One change from a look to the next.
#[derive(Debug, Clone)]
struct Change {
    since: u64,
    /// The content that goes, and the opacity it had when the change began.
    prev: Option<(Box<Props>, f32)>,
    /// The waveform as it was drawn when the change began, when both looks have one.
    morph_from: Option<Vec<WaveBar>>,
    /// The size changes, so the marks wait for it to settle.
    resized: bool,
    /// The size the bar keeps while the old content fades out (Reduce Motion).
    hold: (f32, f32),
}

impl Change {
    fn elapsed(&self, now: u64) -> f32 {
        now.saturating_sub(self.since) as f32
    }

    fn marks_from(&self) -> f32 {
        if self.resized { MARKS_AFTER_MS } else { OUT_MS }
    }

    fn is_done(&self, now: u64) -> bool {
        let e = self.elapsed(now);
        e >= OUT_MS + IN_MS
            && e >= self.marks_from() + MARKS_MS
            && (self.morph_from.is_none() || e >= MORPH_MS)
    }
}

#[derive(Debug, Default)]
pub struct Motion {
    width: SpringState,
    height: SpringState,
    /// How far the bar is there, 0 to 1, at an even pace. The drawn opacity eases it.
    there: f32,
    leaving: bool,
    wave: Vec<f32>,
    last_ms: Option<u64>,
    size_goal: (f32, f32),
    change: Option<Change>,
    /// The props drawn on the last frame: the look a new change starts from.
    drawn: Option<Props>,
}

fn at(position: f32) -> SpringState {
    SpringState {
        position,
        velocity: 0.0,
    }
}

/// Fast at first, then slow into place: the bar coming and going.
pub fn ease_out(done: f32) -> f32 {
    1.0 - (1.0 - done.clamp(0.0, 1.0)).powi(3)
}

/// Slow at both ends. The content fades use it, so a short fade has no hard first frame.
pub fn smooth(done: f32) -> f32 {
    let t = done.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ease_in_out(done: f32) -> f32 {
    let t = done.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Whether two props show the same thing, apart from the hover and the moment of the wave.
fn same_look(a: &Props, b: &Props) -> bool {
    a.state == b.state
        && a.preparing() == b.preparing()
        && a.message == b.message
        && a.open_history == b.open_history
}

/// A look without the change it was part of, to keep as the content that goes.
fn bare(props: &Props) -> Props {
    Props {
        prev: None,
        prev_fade: 0.0,
        morph: None,
        fade: 1.0,
        marks: 1.0,
        mark_scale: 1.0,
        ..props.clone()
    }
}

impl Motion {
    /// Steps every value to `now_ms` and returns `props`, the look the bar wants, dressed for
    /// this frame. `shown` false sends the bar away with the look it has. `wave` is where each
    /// waveform bar wants to be, 0 to 1, and is empty when no waveform shows.
    pub fn frame(
        &mut self,
        now_ms: u64,
        mut props: Props,
        shown: bool,
        wave: &[f32],
        mode: Mode,
    ) -> Props {
        // A bar at rest ticks slowly, so the first step of a new move would be a long one: it
        // moves one frame instead.
        let resting = self.is_settled();
        let dt = match self.last_ms.replace(now_ms) {
            Some(last) => {
                let dt = (now_ms.saturating_sub(last) as f32 / 1000.0).min(MAX_STEP_S);
                if resting { dt.min(FRAME_S) } else { dt }
            }
            None => 0.0,
        };
        let goal = view::size_of(&props);
        if shown && self.is_gone() {
            // A bar that appears starts at the size of its state and fades in.
            self.width = at(goal.0);
            self.height = at(goal.1);
            self.change = None;
            self.wave = wave.to_vec();
        } else if shown
            && mode != Mode::Instant
            && let Some(drawn) = self
                .drawn
                .as_ref()
                .filter(|drawn| !same_look(drawn, &props))
        {
            self.begin_change(now_ms, drawn.clone(), &props, goal, mode);
        }
        if mode == Mode::Instant {
            self.change = None;
        }
        if self
            .change
            .as_ref()
            .is_some_and(|change| change.is_done(now_ms))
        {
            self.change = None;
        }
        self.size_goal = goal;
        self.wave.resize(wave.len(), 0.0);

        match mode {
            Mode::Instant | Mode::Crossfade => {
                let size = match &self.change {
                    Some(change) if mode == Mode::Crossfade && change.elapsed(now_ms) < OUT_MS => {
                        change.hold
                    }
                    _ => goal,
                };
                self.width = at(size.0);
                self.height = at(size.1);
                self.there = if shown { 1.0 } else { 0.0 };
                self.leaving = !shown;
                self.wave.copy_from_slice(wave);
            }
            Mode::Full => {
                self.width = SIZE.step(self.width, goal.0, dt);
                self.height = SIZE.step(self.height, goal.1, dt);
                // A settled bar stops ticking, so it must land on whole goal values.
                if SIZE.is_settled(self.width, goal.0, SETTLED_PX) {
                    self.width = at(goal.0);
                }
                if SIZE.is_settled(self.height, goal.1, SETTLED_PX) {
                    self.height = at(goal.1);
                }
                self.step_presence(shown, dt);
                for (drawn, wanted) in self.wave.iter_mut().zip(wave) {
                    let ms = if wanted > drawn {
                        WAVE_RISE_MS
                    } else {
                        WAVE_FALL_MS
                    };
                    *drawn += (wanted - *drawn) * (1.0 - (-dt * 1000.0 / ms).exp());
                }
            }
        }

        props.levels = self.wave.clone();
        props.frame = Some(self.size());
        let presence = self.presence();
        props.presence = presence;
        props.scale = if mode == Mode::Full {
            SCALE_FROM + (1.0 - SCALE_FROM) * presence
        } else {
            1.0
        };
        self.dress(&mut props, now_ms, mode);
        if shown {
            self.drawn = Some(props.clone());
        }
        props
    }

    fn begin_change(&mut self, now: u64, drawn: Props, next: &Props, goal: (f32, f32), mode: Mode) {
        let morphs = mode == Mode::Full && view::morphs(&drawn) && view::morphs(next);
        let resized =
            (self.size_goal.0 - goal.0).abs() >= 1.0 || (self.size_goal.1 - goal.1).abs() >= 1.0;
        // A change that comes while the old content is still fading out goes on from there:
        // the look in between never showed.
        if let Some(change) = self.change.as_mut()
            && change.elapsed(now) < OUT_MS
            && change.prev.is_some()
            && !morphs
        {
            change.resized |= resized;
            change.morph_from = None;
            return;
        }
        let shown_fade = if drawn.prev_fade > 0.0 {
            0.0
        } else {
            drawn.fade
        };
        let morph_from = morphs.then(|| view::drawn_wave(&drawn)).flatten();
        let hold = self.size();
        self.change = Some(Change {
            since: now,
            prev: Some((Box::new(bare(&drawn)), shown_fade)),
            morph_from,
            resized,
            hold,
        });
    }

    fn step_presence(&mut self, shown: bool, dt: f32) {
        let leaving = !shown;
        if leaving != self.leaving {
            // The opacity goes on from where it is, on the curve of the new direction.
            let opacity = self.presence();
            self.there = if leaving {
                opacity.cbrt()
            } else {
                1.0 - (1.0 - opacity).cbrt()
            };
            self.leaving = leaving;
        }
        let step = dt * 1000.0 / PRESENCE_MS;
        self.there = if shown {
            (self.there + step).min(1.0)
        } else {
            (self.there - step).max(0.0)
        };
    }

    /// The timed parts of a change: the old and the new content, the marks, and the wave.
    fn dress(&self, props: &mut Props, now: u64, mode: Mode) {
        let Some(change) = &self.change else {
            props.fade = 1.0;
            props.prev = None;
            props.prev_fade = 0.0;
            props.marks = 1.0;
            props.mark_scale = 1.0;
            props.morph = None;
            return;
        };
        let e = change.elapsed(now);
        let out = 1.0 - smooth(e / OUT_MS);
        props.prev = change.prev.as_ref().map(|(prev, _)| prev.clone());
        props.prev_fade = change.prev.as_ref().map_or(0.0, |(_, alpha)| alpha * out);
        if e >= OUT_MS {
            props.prev = None;
            props.prev_fade = 0.0;
        }
        props.fade = smooth((e - OUT_MS) / IN_MS);
        let marks = smooth((e - change.marks_from()) / MARKS_MS);
        props.marks = marks;
        props.mark_scale = if mode == Mode::Full {
            MARK_FROM + (1.0 - MARK_FROM) * marks
        } else {
            1.0
        };
        props.morph = change
            .morph_from
            .as_ref()
            .map(|from| (from.clone(), ease_in_out(e / MORPH_MS)));
    }

    /// The window size on this frame. Never smaller than one pixel.
    pub fn size(&self) -> (f32, f32) {
        (self.width.position.max(1.0), self.height.position.max(1.0))
    }

    /// The opacity of the whole bar: 0 when gone, 1 when fully shown.
    pub fn presence(&self) -> f32 {
        if self.leaving {
            self.there.powi(3)
        } else {
            ease_out(self.there)
        }
    }

    /// The bar has faded out to nothing, so its window can go.
    pub fn is_gone(&self) -> bool {
        self.there <= 0.0
    }

    /// Nothing moves until the goal changes, so the bar can tick slowly again.
    pub fn is_settled(&self) -> bool {
        SIZE.is_settled(self.width, self.size_goal.0, SETTLED_PX)
            && SIZE.is_settled(self.height, self.size_goal.1, SETTLED_PX)
            && if self.leaving {
                self.is_gone()
            } else {
                self.there >= 1.0
            }
            && self.change.is_none()
    }
}

#[cfg(test)]
mod tests;
