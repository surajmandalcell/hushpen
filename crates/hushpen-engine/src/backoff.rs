//! Restart delays for a child that keeps dying.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Backoff {
    steps: Vec<Duration>,
    healthy_after: Duration,
    streak: usize,
}

impl Backoff {
    /// `steps` are the delays after the first, second, third crash in a row. The last one
    /// repeats. A child that lived `healthy_after` or longer starts the count again.
    pub fn new(steps: Vec<Duration>, healthy_after: Duration) -> Self {
        let steps = if steps.is_empty() {
            vec![Duration::from_secs(1)]
        } else {
            steps
        };
        Self {
            steps,
            healthy_after,
            streak: 0,
        }
    }

    /// The delay before the next start, for a child that lived for `uptime`.
    pub fn next_delay(&mut self, uptime: Duration) -> Duration {
        if uptime >= self.healthy_after {
            self.streak = 0;
        }
        let delay = self.steps[self.streak.min(self.steps.len() - 1)];
        self.streak += 1;
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(values: &[u64]) -> Vec<Duration> {
        values.iter().map(|s| Duration::from_secs(*s)).collect()
    }

    #[test]
    fn crashes_in_a_row_wait_1_then_5_then_30_seconds_and_stay_at_30() {
        let mut backoff = Backoff::new(secs(&[1, 5, 30]), Duration::from_secs(60));
        let short = Duration::from_secs(2);
        let delays: Vec<_> = (0..5).map(|_| backoff.next_delay(short)).collect();
        assert_eq!(delays, secs(&[1, 5, 30, 30, 30]));
    }

    #[test]
    fn a_child_that_lived_60_seconds_starts_the_count_again() {
        let mut backoff = Backoff::new(secs(&[1, 5, 30]), Duration::from_secs(60));
        let short = Duration::from_secs(2);
        backoff.next_delay(short);
        backoff.next_delay(short);
        assert_eq!(
            backoff.next_delay(Duration::from_secs(60)),
            Duration::from_secs(1)
        );
        assert_eq!(backoff.next_delay(short), Duration::from_secs(5));
    }

    #[test]
    fn fifty_nine_seconds_is_not_long_enough() {
        let mut backoff = Backoff::new(secs(&[1, 5, 30]), Duration::from_secs(60));
        backoff.next_delay(Duration::from_secs(1));
        assert_eq!(
            backoff.next_delay(Duration::from_secs(59)),
            Duration::from_secs(5)
        );
    }
}
