//! The keys that type letters and digits on the layout in use.
//!
//! Cmd+V is Cmd plus the key that carries `v`. On QWERTY that is key code 9; on Dvorak and
//! other layouts it is another key, and a fixed 9 would paste with the wrong shortcut. The key
//! listener needs the same table to turn key codes into letters for shortcuts.
//!
//! The Text Input Sources calls (TIS) are main-queue only: macOS aborts the process when one
//! runs on another thread. So the table is read on the main thread only, at start, when the
//! user changes the layout, and when the app comes to the front. The key listener thread and
//! the insert worker read the shared copy and never call TIS.

// UCKeyTranslate and the input source calls are plain C calls on memory this module owns.
#![allow(unsafe_code)]

use block2::RcBlock;
use core_foundation::base::{CFType, TCFType};
use core_foundation::data::{CFData, CFDataRef};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSOperationQueue,
    NSString,
};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, OnceLock, RwLock};

/// The key code of `v` on an ANSI keyboard, used when the layout cannot be read.
pub(crate) const ANSI_V: u16 = 9;
/// The key code of `c` on an ANSI keyboard, used when the layout cannot be read.
pub(crate) const ANSI_C: u16 = 8;
/// The key code of `z` on an ANSI keyboard, used when the layout cannot be read.
pub(crate) const ANSI_Z: u16 = 6;

/// `kTISNotifySelectedKeyboardInputSourceChanged`, posted to every app when the user picks
/// another layout.
const LAYOUT_CHANGED: &str = "com.apple.Carbon.TISNotifySelectedKeyboardInputSourceChanged";
const APP_ACTIVE: &str = "NSApplicationDidBecomeActiveNotification";

/// The letter and digit keys of one layout, by character.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Table {
    codes: HashMap<char, u16>,
}

impl Table {
    /// Builds the table from what each key code below 128 types unmodified. The first key that
    /// types a character wins.
    pub(crate) fn from_typed(mut typed: impl FnMut(u16) -> Option<char>) -> Self {
        let mut codes = HashMap::new();
        for code in 0..128u16 {
            let Some(ch) = typed(code) else { continue };
            let mut lower = ch.to_lowercase();
            if let (Some(lower), None) = (lower.next(), lower.next())
                && (lower.is_ascii_lowercase() || lower.is_ascii_digit())
            {
                codes.entry(lower).or_insert(code);
            }
        }
        Self { codes }
    }

    /// The key code that types `target` (a letter or digit).
    pub(crate) fn code(&self, target: char) -> Option<u16> {
        self.codes.get(&target.to_ascii_lowercase()).copied()
    }

    /// The reverse map for the key listener: key code to character, with Space added.
    pub(crate) fn chars(&self, space_code: i64) -> HashMap<i64, char> {
        let mut chars: HashMap<i64, char> = self
            .codes
            .iter()
            .map(|(ch, code)| (i64::from(*code), *ch))
            .collect();
        chars.insert(space_code, ' ');
        chars
    }
}

struct Published {
    generation: u64,
    table: Arc<Table>,
}

/// The table that the main thread publishes and the other threads read.
pub(crate) struct LayoutCache {
    state: RwLock<Published>,
}

impl LayoutCache {
    pub(crate) fn new() -> Self {
        Self {
            state: RwLock::new(Published {
                generation: 0,
                table: Arc::new(Table::default()),
            }),
        }
    }

    /// Replaces the table. The generation moves only when the table differs.
    pub(crate) fn publish(&self, table: Table) {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *state.table != table {
            state.generation += 1;
            state.table = Arc::new(table);
        }
    }

    /// Reads the layout from the system and publishes it. Main thread only.
    pub(crate) fn refresh_from_system(&self) {
        self.refresh_with(read_system);
    }

    /// Publishes what `read` returns. A read that finds no layout keeps the table it had.
    fn refresh_with(&self, read: impl FnOnce() -> Option<Table>) {
        if let Some(table) = read() {
            self.publish(table);
        }
    }

    /// The generation and the table now. Any thread; no system call.
    pub(crate) fn snapshot(&self) -> (u64, Arc<Table>) {
        let state = self
            .state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.generation, Arc::clone(&state.table))
    }

    /// The key code that types `target` on the layout in use. Any thread; no system call.
    pub(crate) fn key_code_for(&self, target: char) -> Option<u16> {
        self.snapshot().1.code(target)
    }

    /// The key code that types `v`. A layout with no Latin letters (Russian, for example) falls
    /// back to the Latin layout the system keeps for shortcuts, and an unread layout to ANSI.
    pub(crate) fn v_key_code(&self) -> u16 {
        self.key_code_for('v').unwrap_or(ANSI_V)
    }

    pub(crate) fn c_key_code(&self) -> u16 {
        self.key_code_for('c').unwrap_or(ANSI_C)
    }

    pub(crate) fn z_key_code(&self) -> u16 {
        self.key_code_for('z').unwrap_or(ANSI_Z)
    }
}

/// The one cache of the process.
pub(crate) fn shared() -> &'static Arc<LayoutCache> {
    static SHARED: OnceLock<Arc<LayoutCache>> = OnceLock::new();
    SHARED.get_or_init(|| Arc::new(LayoutCache::new()))
}

/// Whether the caller is the main thread of the process (`pthread_main_np`).
pub(crate) fn is_main_thread() -> bool {
    // SAFETY: a plain libSystem call with no arguments.
    unsafe { pthread_main_np() != 0 }
}

fn assert_main_thread(what: &str) {
    debug_assert!(
        is_main_thread(),
        "{what} must run on the main thread: Text Input Sources are main-queue only"
    );
}

/// Reads the layout now and keeps it fresh: on a layout change, and whenever the app comes to
/// the front (the system holds distributed notifications back while the app is in the
/// background). Runs once; main thread only.
pub(crate) fn follow_system() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    assert_main_thread("following the keyboard layout");
    ONCE.call_once(|| {
        shared().refresh_from_system();
        observe(
            &NSDistributedNotificationCenter::defaultCenter(),
            LAYOUT_CHANGED,
        );
        observe(&NSNotificationCenter::defaultCenter(), APP_ACTIVE);
    });
}

/// Registers a block that runs on the main queue. The observer lives as long as the process,
/// so its token is kept on purpose.
fn observe(center: &NSNotificationCenter, name: &str) {
    let name = NSString::from_str(name);
    let block = RcBlock::new(|_: NonNull<NSNotification>| shared().refresh_from_system());
    let queue = NSOperationQueue::mainQueue();
    // SAFETY: a nil object is allowed; the queue is the main queue, so the block runs on the
    // main thread whichever thread posts.
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(&name), None, Some(&queue), &block)
    };
    std::mem::forget(token);
}

unsafe extern "C" {
    fn pthread_main_np() -> i32;
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISCopyCurrentASCIICapableKeyboardLayoutInputSource() -> *const c_void;
    fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
    static kTISPropertyUnicodeKeyLayoutData: *const c_void;
    fn UCKeyTranslate(
        layout: *const u8,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    ) -> i32;
    fn LMGetKbdType() -> u8;
}

const KEY_ACTION_DOWN: u16 = 0;
const NO_DEAD_KEYS: u32 = 1;

/// The letter and digit keys of the layout in use, or `None` when the system has no Latin
/// layout to read. This is the only function that calls TIS.
pub(crate) fn read_system() -> Option<Table> {
    assert_main_thread("reading the keyboard layout");
    // SAFETY: the call returns an owned input source (create rule) or null.
    let source = unsafe {
        let raw = TISCopyCurrentASCIICapableKeyboardLayoutInputSource();
        if raw.is_null() {
            return None;
        }
        CFType::wrap_under_create_rule(raw.cast())
    };
    // SAFETY: the source is alive; the data it returns belongs to the source (get rule).
    let data = unsafe {
        let raw = TISGetInputSourceProperty(
            source.as_concrete_TypeRef().cast(),
            kTISPropertyUnicodeKeyLayoutData,
        );
        if raw.is_null() {
            return None;
        }
        CFData::wrap_under_get_rule(raw as CFDataRef)
    };
    let layout = data.bytes();
    // SAFETY: a plain C call with no arguments.
    let keyboard_type = u32::from(unsafe { LMGetKbdType() });
    Some(Table::from_typed(|code| {
        let mut dead = 0u32;
        let mut length = 0usize;
        let mut buffer = [0u16; 4];
        // SAFETY: `layout` is the `uchr` table of a live input source, and the buffer holds
        // the four code units that the length argument allows.
        let status = unsafe {
            UCKeyTranslate(
                layout.as_ptr(),
                code,
                KEY_ACTION_DOWN,
                0,
                keyboard_type,
                NO_DEAD_KEYS,
                &mut dead,
                buffer.len(),
                &mut length,
                buffer.as_mut_ptr(),
            )
        };
        if status != 0 || length != 1 {
            return None;
        }
        char::from_u32(u32::from(buffer[0]))
    }))
}

/// Two layouts as `typed` functions, for the tests of every module that reads the table.
#[cfg(test)]
pub(crate) mod layouts {
    pub(crate) fn qwerty(code: u16) -> Option<char> {
        let row = [(0, 'a'), (1, 's'), (2, 'd'), (9, 'v'), (11, 'b'), (12, 'q')];
        row.iter().find(|(key, _)| *key == code).map(|(_, ch)| *ch)
    }

    pub(crate) fn dvorak(code: u16) -> Option<char> {
        let row = [
            (9, '.'),
            (11, 'x'),
            (14, 'p'),
            (45, 'b'),
            (40, 'v'),
            (8, 'j'),
        ];
        row.iter().find(|(key, _)| *key == code).map(|(_, ch)| *ch)
    }
}

#[cfg(test)]
mod tests {
    use super::layouts::{dvorak, qwerty};
    use super::*;

    #[test]
    fn qwerty_types_v_on_key_nine() {
        assert_eq!(Table::from_typed(qwerty).code('v'), Some(ANSI_V));
    }

    #[test]
    fn dvorak_types_v_on_another_key() {
        assert_eq!(Table::from_typed(dvorak).code('v'), Some(40));
    }

    #[test]
    fn a_layout_without_v_has_no_code_for_it() {
        assert_eq!(Table::from_typed(|_| Some('x')).code('v'), None);
    }

    #[test]
    fn capitals_and_digits_are_found_and_other_characters_are_not_kept() {
        let table = Table::from_typed(|code| match code {
            0 => Some('A'),
            18 => Some('1'),
            49 => Some(' '),
            43 => Some(','),
            _ => None,
        });
        assert_eq!(table.code('a'), Some(0));
        assert_eq!(table.code('1'), Some(18));
        assert_eq!(table.code(','), None);
    }

    #[test]
    fn the_listener_map_has_the_letters_and_space() {
        let chars = Table::from_typed(dvorak).chars(49);
        assert_eq!(chars.get(&40), Some(&'v'));
        assert_eq!(chars.get(&49), Some(&' '));
        assert_eq!(chars.get(&9), None);
    }

    #[test]
    fn publishing_the_same_table_keeps_the_generation() {
        let cache = LayoutCache::new();
        cache.publish(Table::from_typed(qwerty));
        let (first, _) = cache.snapshot();
        cache.publish(Table::from_typed(qwerty));
        assert_eq!(cache.snapshot().0, first);
        cache.publish(Table::from_typed(dvorak));
        assert_eq!(cache.snapshot().0, first + 1);
    }

    #[test]
    fn an_unread_layout_falls_back_to_the_ansi_keys() {
        let cache = LayoutCache::new();
        assert_eq!(
            (cache.v_key_code(), cache.c_key_code(), cache.z_key_code()),
            (ANSI_V, ANSI_C, ANSI_Z)
        );
    }

    /// A worker thread is never the main thread, so a TIS call there would trip the assertion
    /// in `read_system`. The reads of the shared table must work there without one.
    #[test]
    fn another_thread_reads_the_shared_table_and_never_calls_tis() {
        let cache = Arc::new(LayoutCache::new());
        cache.publish(Table::from_typed(dvorak));
        let reader = Arc::clone(&cache);
        let (v, c, snapshot) = std::thread::spawn(move || {
            assert!(!is_main_thread());
            (reader.v_key_code(), reader.c_key_code(), reader.snapshot())
        })
        .join()
        .expect("a read of the shared table does not touch TIS");
        assert_eq!((v, c), (40, ANSI_C));
        assert_eq!(snapshot.0, 1);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn the_system_read_refuses_to_run_off_the_main_thread() {
        let outcome = std::thread::spawn(read_system).join();
        let panic = outcome.expect_err("the read asserts the main thread");
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
            .unwrap_or_default();
        assert!(message.contains("main thread"), "{message}");
    }

    #[cfg(debug_assertions)]
    #[test]
    fn following_the_system_refuses_to_start_off_the_main_thread() {
        assert!(std::thread::spawn(follow_system).join().is_err());
    }

    #[test]
    fn a_refresh_publishes_the_new_layout_and_a_failed_read_keeps_the_old_one() {
        let cache = LayoutCache::new();
        cache.refresh_with(|| Some(Table::from_typed(qwerty)));
        assert_eq!(cache.v_key_code(), 9);
        // The layout-change notification runs this on the main thread.
        cache.refresh_with(|| Some(Table::from_typed(dvorak)));
        assert_eq!(cache.v_key_code(), 40);
        cache.refresh_with(|| None);
        assert_eq!(cache.v_key_code(), 40);
    }

    #[test]
    fn a_published_change_reaches_a_reader_that_looks_again() {
        let cache = Arc::new(LayoutCache::new());
        cache.publish(Table::from_typed(qwerty));
        let (seen, _) = cache.snapshot();
        // The layout-change notification ends in `refresh_from_system`, which publishes.
        cache.publish(Table::from_typed(dvorak));
        let reader = Arc::clone(&cache);
        let (generation, table) = std::thread::spawn(move || reader.snapshot())
            .join()
            .unwrap();
        assert!(generation > seen);
        assert_eq!(table.code('v'), Some(40));
    }
}
