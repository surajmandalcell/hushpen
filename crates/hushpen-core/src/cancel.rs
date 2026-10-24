//! A flag that a running job reads to stop early. Clones share one flag.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// The flag itself, for a native library that polls an atomic boolean.
    pub fn as_atomic(&self) -> &Arc<AtomicBool> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::CancelFlag;

    #[test]
    fn a_clone_sees_the_cancel() {
        let flag = CancelFlag::new();
        let clone = flag.clone();
        assert!(!clone.is_cancelled());
        flag.cancel();
        assert!(clone.is_cancelled());
    }
}
