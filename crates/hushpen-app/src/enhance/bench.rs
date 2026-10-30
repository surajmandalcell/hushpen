//! The search behind "Find best settings": like an overclock tool, it changes one value at a
//! time, transcribes the take with it, keeps the change when fewer words come out wrong, and goes
//! round again until no change helps.

use hushpen_audio::Enhance;

/// The low cuts the search tries, in Hz; 0 is off.
const LOW_CUTS: [u16; 5] = [0, 60, 80, 120, 160];
const BASS: [f32; 5] = [3.0, 0.0, -3.0, -6.0, -9.0];
const TREBLE: [f32; 4] = [-3.0, 0.0, 3.0, 6.0];
const GAINS: [f32; 5] = [1.0, 1.25, 1.5, 1.75, 2.0];
/// The rounds over every value at most; the search stops sooner once a round changes nothing.
const MAX_PASSES: usize = 3;
/// The values one round tries.
const PER_PASS: usize = LOW_CUTS.len() + BASS.len() + TREBLE.len() + 2 + GAINS.len();
/// How many of the results the sheet shows.
pub const SHOWN: usize = 5;

/// One setting the search tries: the enhance and the input boost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub enhance: Enhance,
    pub gain: f32,
}

impl Candidate {
    /// Auto with the boost the user has now: where the search starts, and what ties prefer.
    pub fn auto(gain: f32) -> Self {
        Self {
            enhance: Enhance::AUTO,
            gain,
        }
    }

    fn same(&self, other: &Self) -> bool {
        self.enhance.to_bits() == other.enhance.to_bits() && (self.gain - other.gain).abs() < 0.001
    }

    /// How far this is from `auto`, in steps of the search: 40 Hz of low cut, 3 dB of a shelf,
    /// the noise reduction, or 25% of boost each count as one.
    fn distance(&self, auto: &Self) -> f32 {
        let (a, b) = (self.enhance, auto.enhance);
        (f32::from(a.low_cut_hz) - f32::from(b.low_cut_hz)).abs() / 40.0
            + (a.bass_db - b.bass_db).abs() / 3.0
            + (a.treble_db - b.treble_db).abs() / 3.0
            + f32::from(u8::from(a.noise_reduction != b.noise_reduction))
            + (self.gain - auto.gain).abs() / 0.25
    }

    /// The setting in a few words, such as `Low cut 80 Hz, bass -6 dB`. `gain` is the boost the
    /// user has now; a boost is named only when it differs.
    pub fn describe(&self, gain: f32) -> String {
        let enhance = self.enhance;
        let mut parts = Vec::new();
        if enhance.low_cut_hz > 0 {
            parts.push(format!("low cut {} Hz", enhance.low_cut_hz));
        }
        if enhance.bass_db != 0.0 {
            parts.push(format!("bass {}", signed_db(enhance.bass_db)));
        }
        if enhance.treble_db != 0.0 {
            parts.push(format!("treble {}", signed_db(enhance.treble_db)));
        }
        if enhance.noise_reduction {
            parts.push("noise reduction".to_owned());
        }
        if (self.gain - gain).abs() >= 0.001 {
            parts.push(format!("boost {:.0}%", self.gain * 100.0));
        }
        let text = if parts.is_empty() {
            "off".to_owned()
        } else {
            parts.join(", ")
        };
        let mut chars = text.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    }
}

/// `-6 dB`, `+3 dB`.
pub fn signed_db(db: f32) -> String {
    if db > 0.0 {
        format!("+{db:.0} dB")
    } else {
        format!("{db:.0} dB")
    }
}

/// One tried setting and how many words came out wrong with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub candidate: Candidate,
    pub errors: usize,
}

/// What the search found: every tried setting, best first. Ties go to the one closest to Auto.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub ranked: Vec<Scored>,
}

impl Outcome {
    /// Every tried setting got the same number of words wrong, so none is better than Auto.
    pub fn all_same(&self) -> bool {
        self.ranked
            .windows(2)
            .all(|pair| pair[0].errors == pair[1].errors)
    }

    pub fn top(&self) -> &[Scored] {
        &self.ranked[..self.ranked.len().min(SHOWN)]
    }
}

/// Runs the search from Off and `auto`. `score` transcribes the take with one setting and
/// returns its wrong words; an error from it ends the search with that error. `progress` hears
/// the runs so far and about how many there will be.
pub fn search<E>(
    auto: Candidate,
    mut score: impl FnMut(&Candidate) -> Result<usize, E>,
    mut progress: impl FnMut(usize, usize),
) -> Result<Outcome, E> {
    let mut tried: Vec<Scored> = Vec::new();
    let mut estimate = 0;
    let better = |a: &Scored, b: &Scored| {
        a.errors < b.errors
            || (a.errors == b.errors && a.candidate.distance(&auto) < b.candidate.distance(&auto))
    };
    let mut run = |candidate: Candidate, tried: &mut Vec<Scored>, left: usize| {
        if let Some(seen) = tried.iter().find(|seen| seen.candidate.same(&candidate)) {
            return Ok(*seen);
        }
        let scored = Scored {
            candidate,
            errors: score(&candidate)?,
        };
        tried.push(scored);
        estimate = estimate.max(tried.len() + left);
        progress(tried.len(), estimate);
        Ok(scored)
    };
    let off = run(
        Candidate {
            enhance: Enhance::OFF,
            gain: auto.gain,
        },
        &mut tried,
        1 + 2 * PER_PASS,
    )?;
    let start = run(auto, &mut tried, 2 * PER_PASS)?;
    let mut best = if better(&off, &start) { off } else { start };
    for pass in 0..MAX_PASSES {
        let mut changed = false;
        let mut done_in_pass = 0;
        for axis in 0..5 {
            for value in 0..axis_len(axis) {
                done_in_pass += 1;
                let next_pass = if pass + 1 < MAX_PASSES {
                    PER_PASS / 2
                } else {
                    0
                };
                let left = PER_PASS - done_in_pass + next_pass;
                let scored = run(with(best.candidate, axis, value), &mut tried, left)?;
                if better(&scored, &best) {
                    best = scored;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    tried.sort_by(|a, b| {
        a.errors.cmp(&b.errors).then(
            a.candidate
                .distance(&auto)
                .total_cmp(&b.candidate.distance(&auto)),
        )
    });
    Ok(Outcome { ranked: tried })
}

fn axis_len(axis: usize) -> usize {
    match axis {
        0 => LOW_CUTS.len(),
        1 => BASS.len(),
        2 => TREBLE.len(),
        3 => 2,
        _ => GAINS.len(),
    }
}

/// `base` with value `index` of `axis`: the low cut, the bass, the treble, the noise reduction,
/// then the boost.
fn with(base: Candidate, axis: usize, index: usize) -> Candidate {
    let mut next = base;
    match axis {
        0 => next.enhance.low_cut_hz = LOW_CUTS[index],
        1 => next.enhance.bass_db = BASS[index],
        2 => next.enhance.treble_db = TREBLE[index],
        3 => next.enhance.noise_reduction = index == 1,
        _ => next.gain = GAINS[index],
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(score: impl Fn(&Candidate) -> usize) -> (Outcome, Vec<Candidate>, Vec<(usize, usize)>) {
        let mut order = Vec::new();
        let mut steps = Vec::new();
        let outcome = search::<()>(
            Candidate::auto(1.5),
            |candidate| {
                order.push(*candidate);
                Ok(score(candidate))
            },
            |done, about| steps.push((done, about)),
        )
        .unwrap();
        (outcome, order, steps)
    }

    #[test]
    fn it_starts_from_off_and_auto_and_never_tries_a_setting_twice() {
        let (outcome, order, _) = run(|_| 3);
        assert_eq!(order[0].enhance, Enhance::OFF);
        assert_eq!(order[1], Candidate::auto(1.5));
        for (index, one) in order.iter().enumerate() {
            assert!(
                order[index + 1..].iter().all(|other| !one.same(other)),
                "{one:?} twice"
            );
        }
        assert_eq!(outcome.ranked.len(), order.len());
    }

    #[test]
    fn equal_scores_keep_auto_on_top_and_say_so() {
        let (outcome, order, _) = run(|_| 3);
        assert!(outcome.all_same());
        assert_eq!(outcome.top()[0].candidate, Candidate::auto(1.5));
        assert_eq!(outcome.top().len(), SHOWN);
        assert_eq!(order.len(), 2 + PER_PASS - 5, "one round, then it stops");
    }

    #[test]
    fn it_climbs_one_value_at_a_time_to_the_best_setting() {
        // Fewest errors at a low cut of 120 Hz, treble +3 dB, and a 175% boost.
        let score = |c: &Candidate| {
            usize::from(c.enhance.low_cut_hz != 120)
                + usize::from(c.enhance.treble_db != 3.0)
                + usize::from((c.gain - 1.75).abs() > 0.01)
                + usize::from(c.enhance.noise_reduction)
        };
        let (outcome, order, steps) = run(score);
        let best = outcome.top()[0];
        assert_eq!(best.errors, 0);
        assert_eq!(best.candidate.enhance.low_cut_hz, 120);
        assert_eq!(best.candidate.enhance.treble_db, 3.0);
        assert_eq!(
            best.candidate.enhance.bass_db, -6.0,
            "ties keep the Auto bass"
        );
        assert_eq!(best.candidate.gain, 1.75);
        assert!(!outcome.all_same());
        assert!((30..=80).contains(&order.len()), "{} runs", order.len());
        let (done, about) = *steps.last().unwrap();
        assert_eq!(done, order.len());
        assert!(about >= done);
        assert!(
            steps.windows(2).all(|pair| pair[1].1 >= pair[0].1),
            "the estimate only grows"
        );
    }

    #[test]
    fn an_error_ends_the_search() {
        let mut runs = 0;
        let ended = search(
            Candidate::auto(1.5),
            |_| {
                runs += 1;
                if runs == 4 { Err("cancelled") } else { Ok(1) }
            },
            |_, _| {},
        );
        assert_eq!(ended, Err("cancelled"));
        assert_eq!(runs, 4);
    }

    #[test]
    fn a_setting_reads_as_a_few_words() {
        assert_eq!(
            Candidate::auto(1.5).describe(1.5),
            "Low cut 80 Hz, bass -6 dB"
        );
        let other = Candidate {
            enhance: Enhance {
                low_cut_hz: 0,
                bass_db: 0.0,
                treble_db: 3.0,
                noise_reduction: true,
            },
            gain: 1.75,
        };
        assert_eq!(
            other.describe(1.5),
            "Treble +3 dB, noise reduction, boost 175%"
        );
        let off = Candidate {
            enhance: Enhance::OFF,
            gain: 1.5,
        };
        assert_eq!(off.describe(1.5), "Off");
    }
}
