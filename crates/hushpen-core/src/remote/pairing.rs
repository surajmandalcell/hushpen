//! Pairing progresses only while an offer remains valid.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Pairing {
    #[default]
    Idle,
    Offered {
        payload: String,
        expires_at: u64,
    },
    Confirming {
        code: String,
        expires_at: u64,
    },
    Paired {
        device_id: String,
    },
    Failed(PairFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairFailure {
    Expired,
    InvalidOffer,
    InvalidCode,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairEvent {
    Offer { payload: String, expires_at: u64 },
    Confirm { code: String },
    Paired { device_id: String },
    Reject,
    Cancel,
    Tick,
}

impl Pairing {
    pub fn apply(&mut self, event: PairEvent, now: u64) {
        if event == PairEvent::Cancel {
            *self = Self::Idle;
            return;
        }
        if let PairEvent::Offer {
            payload,
            expires_at,
        } = event
        {
            *self = if payload.is_empty() || expires_at <= now {
                Self::Failed(PairFailure::InvalidOffer)
            } else {
                Self::Offered {
                    payload,
                    expires_at,
                }
            };
            return;
        }
        if matches!(self, Self::Offered { expires_at, .. } | Self::Confirming { expires_at, .. } if now >= *expires_at)
        {
            *self = Self::Failed(PairFailure::Expired);
            return;
        }
        match (&*self, event) {
            (Self::Offered { expires_at, .. }, PairEvent::Confirm { code }) => {
                *self = if code.len() == 6 && code.bytes().all(|digit| digit.is_ascii_digit()) {
                    Self::Confirming {
                        code,
                        expires_at: *expires_at,
                    }
                } else {
                    Self::Failed(PairFailure::InvalidCode)
                };
            }
            (Self::Confirming { .. }, PairEvent::Paired { device_id }) if !device_id.is_empty() => {
                *self = Self::Paired { device_id };
            }
            (Self::Offered { .. } | Self::Confirming { .. }, PairEvent::Reject) => {
                *self = Self::Failed(PairFailure::Rejected);
            }
            _ => {}
        }
    }
}
