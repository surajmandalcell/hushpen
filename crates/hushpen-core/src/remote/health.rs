//! Probe outcomes and bounded retries. Times are supplied in milliseconds.

const READY_MS: u64 = 30_000;
const MAX_BACKOFF_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Unknown,
    Ready,
    Asleep,
    Unreachable,
    ServerModeOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Ready { latency_ms: u32 },
    Asleep,
    Unreachable,
    ServerModeOff,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Health {
    pub status: Status,
    pub latency_ms: Option<u32>,
    next_probe_at: u64,
    ready_until: u64,
    failures: u32,
}

impl Health {
    pub fn ready(&self, now: u64) -> bool {
        self.status == Status::Ready && now < self.ready_until
    }

    pub fn probe_due(&self, now: u64) -> bool {
        now >= self.next_probe_at
    }

    pub fn record(&mut self, probe: Probe, now: u64) {
        match probe {
            Probe::Ready { latency_ms } => {
                self.status = Status::Ready;
                self.latency_ms = Some(latency_ms);
                self.failures = 0;
                self.ready_until = now.saturating_add(READY_MS);
                self.next_probe_at = self.ready_until;
            }
            probe => {
                self.status = match probe {
                    Probe::Asleep => Status::Asleep,
                    Probe::Unreachable => Status::Unreachable,
                    Probe::ServerModeOff => Status::ServerModeOff,
                    Probe::Ready { .. } => unreachable!(),
                };
                self.latency_ms = None;
                self.ready_until = 0;
                self.failures = self.failures.saturating_add(1);
                let delay = 1000u64
                    .saturating_mul(1u64 << self.failures.saturating_sub(1).min(6))
                    .min(MAX_BACKOFF_MS);
                self.next_probe_at = now.saturating_add(delay);
            }
        }
    }

    pub fn tick(&mut self, now: u64) {
        if self.status == Status::Ready && !self.ready(now) {
            self.status = Status::Unknown;
            self.latency_ms = None;
        }
    }
}
