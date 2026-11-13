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
        number: String,
        computer_name: String,
        local_confirmed: bool,
        peer_confirmed: bool,
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
    InvalidNumber,
    InvalidName,
    Mismatch,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairEvent {
    Offer {
        payload: String,
        expires_at: u64,
    },
    /// Supplied only after the protocol verifies commitments and the computer name.
    Confirm {
        number: String,
        computer_name: String,
    },
    LocalConfirm {
        number: String,
    },
    /// Supplied only after the protocol authenticates the peer's approval.
    PeerConfirm {
        number: String,
    },
    Paired {
        device_id: String,
    },
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
            *self = if payload.trim().is_empty() || expires_at <= now {
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
        match (&mut *self, event) {
            (
                Self::Offered { expires_at, .. },
                PairEvent::Confirm {
                    number,
                    computer_name,
                },
            ) => {
                *self = if number.len() != 6 || !number.bytes().all(|digit| digit.is_ascii_digit())
                {
                    Self::Failed(PairFailure::InvalidNumber)
                } else if computer_name.trim().is_empty()
                    || computer_name.len() > 128
                    || computer_name.chars().any(char::is_control)
                {
                    Self::Failed(PairFailure::InvalidName)
                } else {
                    Self::Confirming {
                        number,
                        computer_name,
                        local_confirmed: false,
                        peer_confirmed: false,
                        expires_at: *expires_at,
                    }
                };
            }
            (Self::Confirming { number, .. }, PairEvent::LocalConfirm { number: approved })
            | (Self::Confirming { number, .. }, PairEvent::PeerConfirm { number: approved })
                if *number != approved =>
            {
                *self = Self::Failed(PairFailure::Mismatch);
            }
            (
                Self::Confirming {
                    local_confirmed, ..
                },
                PairEvent::LocalConfirm { .. },
            ) => {
                *local_confirmed = true;
            }
            (Self::Confirming { peer_confirmed, .. }, PairEvent::PeerConfirm { .. }) => {
                *peer_confirmed = true;
            }
            (
                Self::Confirming {
                    local_confirmed: true,
                    peer_confirmed: true,
                    ..
                },
                PairEvent::Paired { device_id },
            ) if !device_id.is_empty() => {
                *self = Self::Paired { device_id };
            }
            (Self::Offered { .. } | Self::Confirming { .. }, PairEvent::Reject) => {
                *self = Self::Failed(PairFailure::Rejected);
            }
            _ => {}
        }
    }
}
