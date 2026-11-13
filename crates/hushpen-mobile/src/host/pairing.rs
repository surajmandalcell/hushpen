//! D-57 on the phone: Server mode is the responder, so the phone lists in a computer's
//! Nearby list and shows the pairing number; the window is open only while a screen asks.

use crate::{MobileError, lock};
use hushpen_remote::{PairOffer, PairingWindow};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const WINDOW_SECS: u64 = 120;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum PairingState {
    Closed,
    Listening,
    /// Both screens show this number; the person taps Pair on each.
    Comparing {
        number: String,
    },
    Paired,
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PairedDesktop {
    pub id: String,
    pub name: String,
}

struct Inner {
    offer: Option<PairOffer>,
    window: Option<PairingWindow>,
    generation: u64,
    expires: u64,
    state: PairingState,
    answer: Option<bool>,
}

pub(super) struct Pairing {
    inner: Mutex<Inner>,
    changed: Condvar,
    names: PathBuf,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_secs())
}

impl Pairing {
    pub(super) fn new(config: PathBuf) -> Self {
        Self {
            inner: Mutex::new(Inner {
                offer: None,
                window: None,
                generation: 0,
                expires: 0,
                state: PairingState::Closed,
                answer: None,
            }),
            changed: Condvar::new(),
            names: config.join("desktops.json"),
        }
    }

    /// A fresh window with fresh codes; any earlier window and its connections close.
    pub(super) fn open(
        &self,
        phone_name: &str,
        port: u16,
        key: [u8; 32],
    ) -> Result<(), MobileError> {
        let expires = now() + WINDOW_SECS;
        let mut offer = PairOffer::new("phone.local".into(), port, key, expires)
            .map_err(MobileError::failed)?;
        offer.open_window(phone_name).map_err(MobileError::failed)?;
        let mut inner = lock(&self.inner);
        if let Some(window) = inner.window.take() {
            window.close();
        }
        inner.window = Some(offer.window());
        inner.offer = Some(offer);
        inner.generation += 1;
        inner.expires = expires;
        inner.state = PairingState::Listening;
        inner.answer = None;
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn close(&self) {
        let mut inner = lock(&self.inner);
        if let Some(window) = inner.window.take() {
            window.close();
        }
        inner.offer = None;
        inner.generation += 1;
        inner.state = PairingState::Closed;
        self.changed.notify_all();
    }

    pub(super) fn state(&self) -> PairingState {
        let expired = {
            let inner = lock(&self.inner);
            matches!(
                inner.state,
                PairingState::Listening | PairingState::Comparing { .. }
            ) && now() >= inner.expires
        };
        if expired {
            self.close();
        }
        lock(&self.inner).state.clone()
    }

    pub(super) fn answer(&self, approve: bool) {
        let mut inner = lock(&self.inner);
        if matches!(inner.state, PairingState::Comparing { .. }) {
            inner.answer = Some(approve);
            self.changed.notify_all();
        }
    }

    /// The offer for one connection. Nobody else can pair while it is out.
    pub(super) fn take(&self) -> Option<(PairOffer, u64)> {
        let mut inner = lock(&self.inner);
        let generation = inner.generation;
        inner.offer.take().map(|offer| (offer, generation))
    }

    pub(super) fn restore(&self, offer: PairOffer, generation: u64) {
        let mut inner = lock(&self.inner);
        if inner.generation == generation && inner.window.is_some() {
            inner.offer = Some(offer);
        }
    }

    /// Blocks the connection until the person taps Pair or Cancel, or the window closes.
    pub(super) fn compare(&self, number: &str) -> std::io::Result<bool> {
        let mut inner = lock(&self.inner);
        inner.state = PairingState::Comparing {
            number: number.into(),
        };
        inner.answer = None;
        let deadline = Instant::now() + Duration::from_secs(WINDOW_SECS);
        loop {
            if let Some(answer) = inner.answer {
                return Ok(answer);
            }
            let open = inner.window.as_ref().is_some_and(PairingWindow::is_open);
            if !open || Instant::now() >= deadline {
                return Ok(false);
            }
            inner = self
                .changed
                .wait_timeout(inner, Duration::from_millis(100))
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }

    /// The connection that compared numbers has ended; the next screen read shows why.
    pub(super) fn finish(&self, generation: u64, result: Result<(), String>) {
        let mut inner = lock(&self.inner);
        if inner.generation != generation {
            return;
        }
        inner.state = match result {
            Ok(()) => PairingState::Paired,
            Err(message) => PairingState::Failed { message },
        };
        if let Some(window) = inner.window.take() {
            window.close();
        }
    }

    pub(super) fn names(&self) -> HashMap<String, String> {
        std::fs::read(&self.names)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub(super) fn set_name(&self, id: &str, name: Option<&str>) -> Result<(), MobileError> {
        let mut names = self.names();
        match name {
            Some(name) if !name.is_empty() => names.insert(id.into(), name.into()),
            _ => names.remove(id),
        };
        std::fs::write(
            &self.names,
            serde_json::to_vec(&names).map_err(MobileError::failed)?,
        )
        .map_err(MobileError::failed)
    }
}
