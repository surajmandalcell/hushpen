//! How the flow bar moves. The bar's frame, its fade, and its waveform are springs and easings
//! that [`Motion::update`] steps from the bar's own clock, so a test drives every frame with
//! plain numbers and the pixel tests draw a still frame.

use gpui_kit::{SpringConfig, SpringState};

/// The frame follows a new state size with a short, slightly soft spring.
const SIZE: SpringConfig = SpringConfig::new(560.0, 40.0, 1.0);
/// Coming and going: no overshoot, so the opacity never passes 1.
const PRESENCE: SpringConfig = SpringConfig::new(420.0, 41.0, 1.0);
/// The content of a new state fades in over this time.
const FADE_MS: f32 = 140.0;
/// A waveform bar rises fast and falls slower, so speech reads as a pulse and not as noise.
const WAVE_RISE_MS: f32 = 40.0;
const WAVE_FALL_MS: f32 = 140.0;
/// A step longer than this is a stall (a sleep, a busy main thread): the motion jumps on
/// instead of replaying it.
const MAX_STEP_S: f32 = 0.05;
/// Below this the bar counts as gone and its window can close.
const GONE: f32 = 0.01;
/// Half a logical pixel is less than any frame change a user can see.
const SETTLED_PX: f32 = 0.5;

/// What the bar wants to look like on this frame.
pub struct Goal<'a> {
    pub size: (f32, f32),
    pub shown: bool,
    /// The bar shows a new state: its content fades in again.
    pub new_state: bool,
    /// Where each waveform bar wants to be, 0 to 1. Empty when no waveform shows.
    pub wave: &'a [f32],
}

#[derive(Debug, Default)]
pub struct Motion {
    width: SpringState,
    height: SpringState,
    presence: SpringState,
    fade: f32,
    wave: Vec<f32>,
    last_ms: Option<u64>,
    size_goal: (f32, f32),
}

fn at(position: f32) -> SpringState {
    SpringState {
        position,
        velocity: 0.0,
    }
}

impl Motion {
    /// Steps every value to `now_ms`. With `animate` off (Reduce Motion, tests) every value
    /// jumps to its goal.
    pub fn update(&mut self, now_ms: u64, goal: Goal, animate: bool) {
        let dt = match self.last_ms.replace(now_ms) {
            Some(last) => (now_ms.saturating_sub(last) as f32 / 1000.0).min(MAX_STEP_S),
            None => 0.0,
        };
        if goal.shown && self.is_gone() {
            // A bar that appears grows out of a circle of its own height.
            self.width = at(goal.size.1);
            self.height = at(goal.size.1);
            self.fade = 0.0;
        }
        if goal.new_state {
            self.fade = 0.0;
        }
        // A bar that goes away shrinks back to a circle while it fades.
        let width_goal = if goal.shown {
            goal.size.0
        } else {
            self.height.position
        };
        self.size_goal = (width_goal, goal.size.1);
        let presence_goal = if goal.shown { 1.0 } else { 0.0 };
        self.wave.resize(goal.wave.len(), 0.0);

        if !animate {
            self.width = at(width_goal);
            self.height = at(goal.size.1);
            self.presence = at(presence_goal);
            self.fade = 1.0;
            self.wave.copy_from_slice(goal.wave);
            return;
        }
        self.width = SIZE.step(self.width, width_goal, dt);
        self.height = SIZE.step(self.height, goal.size.1, dt);
        self.presence = PRESENCE.step(self.presence, presence_goal, dt);
        // A settled bar stops ticking, so it must land on whole goal values: 111.6 px stays
        // 111 px wide on screen.
        if SIZE.is_settled(self.width, width_goal, SETTLED_PX) {
            self.width = at(width_goal);
        }
        if SIZE.is_settled(self.height, goal.size.1, SETTLED_PX) {
            self.height = at(goal.size.1);
        }
        if PRESENCE.is_settled(self.presence, presence_goal, GONE) {
            self.presence = at(presence_goal);
        }
        self.fade = (self.fade + dt * 1000.0 / FADE_MS).min(1.0);
        for (shown, wanted) in self.wave.iter_mut().zip(goal.wave) {
            let ms = if wanted > shown {
                WAVE_RISE_MS
            } else {
                WAVE_FALL_MS
            };
            *shown += (wanted - *shown) * (1.0 - (-dt * 1000.0 / ms).exp());
        }
    }

    /// The window size on this frame. Never smaller than one pixel.
    pub fn size(&self) -> (f32, f32) {
        (self.width.position.max(1.0), self.height.position.max(1.0))
    }

    /// 0 when gone, 1 when fully shown.
    pub fn presence(&self) -> f32 {
        self.presence.position.clamp(0.0, 1.0)
    }

    /// The opacity of the content: it waits for the bar to be half there, then fades in.
    pub fn fade(&self) -> f32 {
        let ease = 1.0 - (1.0 - self.fade).powi(3);
        ease * ((self.presence() - 0.5) * 2.0).clamp(0.0, 1.0)
    }

    pub fn wave(&self) -> &[f32] {
        &self.wave
    }

    pub fn is_gone(&self) -> bool {
        self.presence.position <= GONE && self.presence.velocity.abs() <= GONE
    }

    /// Nothing moves until the goal changes, so the bar can tick slowly again.
    pub fn is_settled(&self) -> bool {
        SIZE.is_settled(self.width, self.size_goal.0, SETTLED_PX)
            && SIZE.is_settled(self.height, self.size_goal.1, SETTLED_PX)
            && (PRESENCE.is_settled(self.presence, 1.0, GONE) || self.is_gone())
            && self.fade >= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_MS: u64 = 16;

    fn shown(size: (f32, f32)) -> Goal<'static> {
        Goal {
            size,
            shown: true,
            new_state: false,
            wave: &[],
        }
    }

    /// Steps `frames` frames of 16 ms from `from_ms` and returns the time after them.
    fn run(
        motion: &mut Motion,
        from_ms: u64,
        frames: u64,
        goal: impl Fn() -> Goal<'static>,
    ) -> u64 {
        let mut now = from_ms;
        for _ in 0..frames {
            now += FRAME_MS;
            motion.update(now, goal(), true);
        }
        now
    }

    #[test]
    fn a_new_bar_grows_out_of_a_circle_and_settles_at_its_size() {
        let mut motion = Motion::default();
        motion.update(0, shown((112.0, 32.0)), true);
        assert_eq!(motion.size(), (32.0, 32.0));
        assert_eq!(motion.fade(), 0.0);

        let now = run(&mut motion, 0, 3, || shown((112.0, 32.0)));
        let (width, _) = motion.size();
        assert!(width > 32.0 && width < 112.0, "on its way: {width}");

        run(&mut motion, now, 40, || shown((112.0, 32.0)));
        assert_eq!(motion.size(), (112.0, 32.0), "it lands on the goal exactly");
        assert_eq!(motion.presence(), 1.0);
        assert!(motion.fade() > 0.99);
        assert!(motion.is_settled());
    }

    #[test]
    fn a_bar_that_goes_shrinks_fades_and_is_gone_within_half_a_second() {
        let mut motion = Motion::default();
        let now = run(&mut motion, 0, 40, || shown((112.0, 32.0)));
        let hidden = || Goal {
            size: (112.0, 32.0),
            shown: false,
            new_state: false,
            wave: &[],
        };
        let mut t = now;
        while !motion.is_gone() {
            t = run(&mut motion, t, 1, hidden);
            assert!(t - now < 500, "still there after {} ms", t - now);
        }
        assert!(motion.size().0 < 40.0, "it shrank: {:?}", motion.size());
        assert_eq!(motion.fade(), 0.0);
    }

    #[test]
    fn a_new_state_fades_its_content_in_and_moves_the_size_without_a_jump() {
        let mut motion = Motion::default();
        let now = run(&mut motion, 0, 40, || shown((112.0, 32.0)));
        motion.update(
            now + FRAME_MS,
            Goal {
                size: (236.0, 32.0),
                shown: true,
                new_state: true,
                wave: &[],
            },
            true,
        );
        let (width, _) = motion.size();
        assert!(width < 140.0, "the first frame moves a little: {width}");
        assert!(motion.fade() < 0.5);
        run(&mut motion, now + FRAME_MS, 40, || shown((236.0, 32.0)));
        assert_eq!(motion.size(), (236.0, 32.0));
    }

    #[test]
    fn the_waveform_rises_fast_and_falls_slow() {
        let mut motion = Motion::default();
        let loud = || Goal {
            size: (112.0, 32.0),
            shown: true,
            new_state: false,
            wave: &[1.0],
        };
        motion.update(0, loud(), true);
        run(&mut motion, 0, 4, loud);
        let up = motion.wave()[0];
        assert!(up > 0.7, "rose to {up}");
        let quiet = || Goal {
            size: (112.0, 32.0),
            shown: true,
            new_state: false,
            wave: &[0.0],
        };
        run(&mut motion, 64, 4, quiet);
        let down = motion.wave()[0];
        assert!(down > 0.2 && down < up, "fell to {down}");
    }

    #[test]
    fn a_stall_does_not_replay_and_no_animation_jumps_to_the_goal() {
        let mut motion = Motion::default();
        motion.update(0, shown((112.0, 32.0)), true);
        motion.update(10_000, shown((112.0, 32.0)), true);
        assert!(motion.size().0 < 112.0, "one stalled step is at most 50 ms");

        let mut still = Motion::default();
        still.update(0, shown((236.0, 32.0)), false);
        assert_eq!(still.size(), (236.0, 32.0));
        assert_eq!(still.presence(), 1.0);
        assert_eq!(still.fade(), 1.0);
        assert!(still.is_settled());
    }
}
