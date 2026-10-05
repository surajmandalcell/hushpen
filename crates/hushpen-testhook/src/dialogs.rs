//! Paths for the next file dialog. `hookctl paths <file...>` queues them; the
//! dialog code takes them instead of opening a native picker.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn queue() -> MutexGuard<'static, VecDeque<Vec<PathBuf>>> {
    static QUEUE: OnceLock<Mutex<VecDeque<Vec<PathBuf>>>> = OnceLock::new();
    QUEUE
        .get_or_init(|| Mutex::new(VecDeque::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Answers one future dialog with these paths.
pub fn answer_next_dialog(paths: Vec<PathBuf>) {
    queue().push_back(paths);
}

/// Called where a dialog would open. `Some` means the hook answered it.
pub fn take_answer() -> Option<Vec<PathBuf>> {
    queue().pop_front()
}

/// The queue is process-wide, so tests that touch it take this lock.
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_served_once_each_in_order() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        while take_answer().is_some() {}
        answer_next_dialog(vec![PathBuf::from("/a.txt")]);
        answer_next_dialog(vec![PathBuf::from("/b.txt"), PathBuf::from("/c.txt")]);
        assert_eq!(take_answer(), Some(vec![PathBuf::from("/a.txt")]));
        assert_eq!(take_answer().map(|paths| paths.len()), Some(2));
        assert_eq!(take_answer(), None);
    }
}
