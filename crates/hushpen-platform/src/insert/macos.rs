//! macOS insertion: a promised pasteboard string and a Cmd+V key event.
//!
//! The text goes on the pasteboard as a promise, so the owner is asked for the data when the
//! target reads it. That request is the receipt the restore waits for. The paste key is posted
//! as a single key event that carries the Command flag, so no modifier key is ever left down.

// The pasteboard owner class and the AppKit constants need objc2's `unsafe` calls.
#![allow(unsafe_code)]

#[cfg(feature = "test-automation")]
mod background;
mod guard;

use super::{Inserter, SystemClock, Target, UndoError, mono_ms, plan, undo_guard};
use crate::keyboard_layout::{self, LayoutCache};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use hushpen_core::insert::flow::{self, Backend, ChordError, Receipts, Request, Step};
use hushpen_core::insert::guard::{Block, Probe as _, check};
use hushpen_core::insert::{Chord, Overrides, Report, Selection, choose_mac};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardType, NSPasteboardTypeString, NSRunningApplication,
    NSWorkspace,
};
use objc2_foundation::{NSArray, NSData, NSObject, NSString};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Clipboard managers that honor this type skip the entry.
const TRANSIENT_TYPE: &str = "org.nspasteboard.TransientType";
const CONCEALED_TYPE: &str = "org.nspasteboard.ConcealedType";

/// Times of the reads, as `mono_ms() + 1` so that 0 means "not yet".
#[derive(Default)]
struct Reads {
    first: AtomicU64,
    last: AtomicU64,
    text: Mutex<String>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "HushpenPasteboardOwner"]
    #[ivars = Reads]
    struct Owner;

    impl Owner {
        #[unsafe(method(pasteboard:provideDataForType:))]
        fn provide_data(&self, pasteboard: &NSPasteboard, kind: &NSPasteboardType) {
            let reads = self.ivars();
            let now = mono_ms() + 1;
            let _ = reads
                .first
                .compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
            reads.last.store(now, Ordering::Release);
            let text = reads
                .text
                .lock()
                .map(|text| text.clone())
                .unwrap_or_default();
            pasteboard.setString_forType(&NSString::from_str(&text), kind);
        }
    }
);

impl Owner {
    fn new(text: &str) -> Retained<Self> {
        let reads = Reads::default();
        if let Ok(mut slot) = reads.text.lock() {
            *slot = text.to_owned();
        }
        let this = Self::alloc().set_ivars(reads);
        // SAFETY: `init` of NSObject on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }

    fn receipts(&self) -> Receipts {
        let reads = self.ivars();
        let load = |slot: &AtomicU64| match slot.load(Ordering::Acquire) {
            0 => None,
            at => Some(at - 1),
        };
        Receipts {
            first: load(&reads.first),
            last: load(&reads.last),
        }
    }

    /// Test builds only: counts a read that the pasteboard did not report to the owner.
    #[cfg(feature = "test-automation")]
    fn mark_read(&self) {
        let reads = self.ivars();
        let now = mono_ms() + 1;
        let _ = reads
            .first
            .compare_exchange(0, now, Ordering::AcqRel, Ordering::Acquire);
        reads.last.store(now, Ordering::Release);
    }
}

type Item = Vec<(String, Vec<u8>)>;

struct MacBoard {
    pasteboard: Retained<NSPasteboard>,
    owner: Option<Retained<Owner>>,
    /// The change count right after our write.
    count: isize,
    /// The keys of the layout in use. The insert worker only reads this copy; the main thread
    /// refreshes it.
    layout: Arc<LayoutCache>,
    /// The process that gets the paste key, or `None` for the whole session.
    pid: Option<i32>,
}

impl MacBoard {
    fn general(layout: Arc<LayoutCache>) -> Self {
        Self::on(NSPasteboard::generalPasteboard(), layout)
    }

    fn on(pasteboard: Retained<NSPasteboard>, layout: Arc<LayoutCache>) -> Self {
        Self {
            pasteboard,
            owner: None,
            count: -1,
            layout,
            pid: None,
        }
    }

    /// Test builds only: pastes into the focused element of the app `pid` by setting its
    /// selected text, which an app in the background takes. The text comes from a normal read of
    /// the pasteboard, which is also the read that the receipt counts.
    #[cfg(feature = "test-automation")]
    fn ax_paste(&mut self, pid: i32) -> bool {
        // SAFETY: a constant of AppKit.
        let string_type = unsafe { NSPasteboardTypeString };
        let Some(text) = self
            .pasteboard
            .stringForType(string_type)
            .map(|text| text.to_string())
        else {
            log::info!(
                "TEST_INSERT ax-selected-text pid={pid} failed (the pasteboard has no text)"
            );
            return false;
        };
        match background::set_selected_text(pid, &text) {
            Ok(()) => {
                if let Some(owner) = &self.owner
                    && owner.receipts().first.is_none()
                {
                    // AppKit may answer a read in the owner's own process without asking the
                    // owner. The target took the text, so the receipt is counted here.
                    owner.mark_read();
                }
                log::info!("TEST_INSERT ax-selected-text pid={pid} ok");
                true
            }
            Err(miss) => {
                log::info!("TEST_INSERT ax-selected-text pid={pid} failed ({miss})");
                // The read above used up the promise and counted a receipt. A fresh promise lets
                // the key paths that follow earn their own.
                if let Err(error) = self.write(Selection::Clipboard, &text) {
                    log::warn!("TEST_INSERT the text was not put back on the pasteboard: {error}");
                }
                false
            }
        }
    }
}

impl Backend for MacBoard {
    type Snapshot = Vec<Item>;

    fn guard(&mut self) -> Option<Block> {
        check(&guard::MacProbe)
    }

    fn snapshot(&mut self, _selection: Selection) -> Self::Snapshot {
        let Some(items) = self.pasteboard.pasteboardItems() else {
            return Vec::new();
        };
        items
            .iter()
            .map(|item| {
                item.types()
                    .iter()
                    .filter_map(|kind| {
                        let data = item.dataForType(&kind)?;
                        Some((kind.to_string(), data.to_vec()))
                    })
                    .collect()
            })
            .collect()
    }

    fn write(&mut self, _selection: Selection, text: &str) -> Result<(), String> {
        let owner = Owner::new(text);
        let transient = NSString::from_str(TRANSIENT_TYPE);
        let concealed = NSString::from_str(CONCEALED_TYPE);
        // SAFETY: a constant of AppKit.
        let string_type = unsafe { NSPasteboardTypeString };
        let kinds = NSArray::from_slice(&[string_type, &*transient, &*concealed]);
        let any: &AnyObject = &owner;
        // SAFETY: the owner answers `pasteboard:provideDataForType:` and outlives the flow,
        // because `self.owner` keeps it.
        let count = unsafe { self.pasteboard.declareTypes_owner(&kinds, Some(any)) };
        if count == 0 {
            return Err("the pasteboard did not take the text".into());
        }
        self.pasteboard
            .setString_forType(&NSString::new(), &transient);
        self.pasteboard
            .setString_forType(&NSString::new(), &concealed);
        self.owner = Some(owner);
        self.count = self.pasteboard.changeCount();
        Ok(())
    }

    fn send_chord(&mut self, chord: Chord) -> Result<(), ChordError> {
        if chord != Chord::CmdV {
            return Err(ChordError::Unavailable);
        }
        if !crate::permissions::accessibility_trusted() {
            return Err(ChordError::NoPermission);
        }
        #[cfg(feature = "test-automation")]
        if let Some(pid) = self.pid
            && self.ax_paste(pid)
        {
            return Ok(());
        }
        command_key(self.layout.v_key_code(), 'V', self.pid)
    }

    fn receipts(&mut self) -> Receipts {
        self.owner
            .as_ref()
            .map(|owner| owner.receipts())
            .unwrap_or_default()
    }

    fn still_ours(&mut self, _selection: Selection) -> bool {
        self.pasteboard.changeCount() == self.count
    }

    fn restore(&mut self, _selection: Selection, snapshot: Self::Snapshot) {
        self.pasteboard.clearContents();
        if snapshot.is_empty() {
            return;
        }
        let items: Vec<Retained<NSPasteboardItem>> = snapshot
            .iter()
            .map(|formats| {
                let item = NSPasteboardItem::new();
                for (kind, bytes) in formats {
                    item.setData_forType(&NSData::with_bytes(bytes), &NSString::from_str(kind));
                }
                item
            })
            .collect();
        let objects = NSArray::from_retained_slice(
            &items
                .into_iter()
                .map(objc2::runtime::ProtocolObject::from_retained)
                .collect::<Vec<_>>(),
        );
        self.pasteboard.writeObjects(&objects);
    }
}

/// How long an app may take to answer Cmd+C with a new pasteboard.
const COPY_WAIT: std::time::Duration = std::time::Duration::from_millis(400);

/// One key press that carries the Command flag, so no modifier key is left down. With `pid` the
/// event goes to that process only, which works while the app is in the background.
fn post_command_key(key: u16, pid: Option<i32>) -> Result<(), ChordError> {
    let post = |down: bool| -> Result<(), ChordError> {
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|()| ChordError::Unavailable)?;
        let event =
            CGEvent::new_keyboard_event(source, key, down).map_err(|()| ChordError::Unavailable)?;
        event.set_flags(CGEventFlags::CGEventFlagCommand);
        match pid {
            Some(pid) => event.post_to_pid(pid),
            None => event.post(CGEventTapLocation::HID),
        }
        Ok(())
    };
    post(true)?;
    std::thread::sleep(std::time::Duration::from_millis(8));
    post(false)
}

/// Reads the selection of the focused app the way a user would: copy it. `copy` presses the
/// copy key. The pasteboard is written back as it was, whether or not the app answered.
fn copied_selection(
    board: &mut MacBoard,
    wait: std::time::Duration,
    copy: impl FnOnce() -> Result<(), ()>,
) -> Option<String> {
    let snapshot = board.snapshot(Selection::Clipboard);
    let before = board.pasteboard.changeCount();
    copy().ok()?;
    let end = std::time::Instant::now() + wait;
    let mut text = None;
    let mut changed = false;
    while std::time::Instant::now() < end {
        if board.pasteboard.changeCount() != before {
            changed = true;
            // SAFETY: a constant of AppKit.
            let string_type = unsafe { NSPasteboardTypeString };
            text = board
                .pasteboard
                .stringForType(string_type)
                .map(|text| text.to_string());
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if changed {
        board.restore(Selection::Clipboard, snapshot);
    }
    text.filter(|text| !text.trim().is_empty())
}

pub(super) struct MacInserter {
    one_at_a_time: Mutex<()>,
    layout: Arc<LayoutCache>,
}

impl MacInserter {
    /// Reads the layout and starts following its changes. Main thread only.
    pub(super) fn new() -> Self {
        keyboard_layout::follow_system();
        Self::with_layout(Arc::clone(keyboard_layout::shared()))
    }

    fn with_layout(layout: Arc<LayoutCache>) -> Self {
        Self {
            one_at_a_time: Mutex::new(()),
            layout,
        }
    }

    /// The app in front. It reads no layout, so a worker thread may call it.
    fn frontmost(&self) -> Target {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map_or_else(Target::default, |app| app_target(&app))
    }

    /// The app that gets the text: the app in front, or in a test build the app that the test
    /// hook names.
    #[cfg(feature = "test-automation")]
    fn focus(&self) -> Target {
        super::test_target::choose(super::test_target::bundle().as_deref(), running, || {
            self.frontmost()
        })
    }

    #[cfg(not(feature = "test-automation"))]
    fn focus(&self) -> Target {
        self.frontmost()
    }
}

fn app_target(app: &NSRunningApplication) -> Target {
    let pid = app.processIdentifier();
    let bundle = app
        .bundleIdentifier()
        .map(|id| id.to_string())
        .unwrap_or_default();
    Target {
        window: u64::try_from(pid).ok(),
        classes: vec![bundle.clone()],
        label: bundle,
        own: u32::try_from(pid).is_ok_and(|pid| pid == std::process::id()),
    }
}

/// A running app with this bundle id.
#[cfg(feature = "test-automation")]
pub(super) fn running(bundle: &str) -> Option<Target> {
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(bundle))
        .iter()
        .find(|app| !app.isTerminated())
        .map(|app| app_target(&app))
}

/// The process that gets the copy, paste, and undo keys for `target`, or `None` for the whole
/// session.
#[cfg(feature = "test-automation")]
fn key_pid(target: &Target) -> Option<i32> {
    super::test_target::key_pid(super::test_target::bundle().as_deref(), target)
}

#[cfg(not(feature = "test-automation"))]
fn key_pid(_target: &Target) -> Option<i32> {
    None
}

/// Command and `letter` (the key `key` of the layout in use). An app in the background gets
/// its menu item pressed, because it ignores a key posted to its process.
#[cfg(feature = "test-automation")]
fn command_key(key: u16, letter: char, pid: Option<i32>) -> Result<(), ChordError> {
    let Some(pid) = pid else {
        return post_command_key(key, None);
    };
    match background::press_command(pid, letter) {
        Ok(()) => {
            log::info!("TEST_INSERT menu-press cmd+{letter} pid={pid} ok");
            return Ok(());
        }
        Err(miss) => log::warn!(
            "TEST_INSERT menu-press cmd+{letter} pid={pid} failed ({miss}), \
             so the key goes to the process"
        ),
    }
    let result = post_command_key(key, Some(pid));
    log::info!(
        "TEST_INSERT pid-post cmd+{letter} pid={pid} {}",
        if result.is_ok() { "sent" } else { "failed" }
    );
    result
}

/// The text selected in the focused element: of the app in front, or of the app `pid`.
#[cfg(feature = "test-automation")]
fn selected_text(pid: Option<i32>) -> Option<String> {
    let Some(pid) = pid else {
        return guard::focused_selected_text();
    };
    match background::selected_text(pid) {
        Ok(text) => {
            log::info!(
                "TEST_INSERT ax-selected-text-read pid={pid} {}",
                if text.is_some() { "ok" } else { "empty" }
            );
            text
        }
        Err(miss) => {
            log::info!("TEST_INSERT ax-selected-text-read pid={pid} failed ({miss})");
            None
        }
    }
}

#[cfg(not(feature = "test-automation"))]
fn selected_text(_pid: Option<i32>) -> Option<String> {
    guard::focused_selected_text()
}

#[cfg(not(feature = "test-automation"))]
fn command_key(key: u16, _letter: char, pid: Option<i32>) -> Result<(), ChordError> {
    post_command_key(key, pid)
}

impl Inserter for MacInserter {
    /// Called on the main thread when the text is ready, which is also where the layout is read
    /// again, so the paste that follows on the worker thread uses the keys now in use.
    fn target(&self) -> Target {
        self.layout.refresh_from_system();
        self.focus()
    }

    fn insert(
        &self,
        text: &str,
        target: &Target,
        overrides: &Overrides,
        observe: &mut dyn FnMut(Step<'_>),
    ) -> Report {
        let (method, copy_note) = plan(target, |target| choose_mac(&target.label, overrides));
        let _guard = self
            .one_at_a_time
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut board = MacBoard::general(Arc::clone(&self.layout));
        board.pid = key_pid(target);
        let request = Request {
            text,
            target: &target.label,
            method,
            copy_note,
        };
        flow::run(&mut board, &SystemClock, &request, observe)
    }

    fn selection(&self, target: &Target) -> Option<String> {
        if target.window.is_none() || target.own {
            return None;
        }
        let probe = guard::MacProbe;
        if probe.secure_event_input() || probe.focused_secure_field() {
            return None;
        }
        let pid = key_pid(target);
        if let Some(text) = selected_text(pid) {
            return Some(text);
        }
        if !crate::permissions::accessibility_trusted() {
            return None;
        }
        let _guard = self
            .one_at_a_time
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let key = self.layout.c_key_code();
        copied_selection(
            &mut MacBoard::general(Arc::clone(&self.layout)),
            COPY_WAIT,
            || command_key(key, 'C', pid).map_err(|_| ()),
        )
    }

    fn undo(&self, target: &Target) -> Result<(), UndoError> {
        // The lock waits for a paste still in flight, so the focus is read after it.
        let _guard = self
            .one_at_a_time
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        undo_guard(target, &self.focus())?;
        if !crate::permissions::accessibility_trusted() {
            return Err(UndoError::NoPermission);
        }
        command_key(self.layout.z_key_code(), 'Z', key_pid(target)).map_err(|error| match error {
            ChordError::NoPermission => UndoError::NoPermission,
            _ => UndoError::Unavailable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private pasteboard, so no test touches the user's clipboard.
    fn board() -> MacBoard {
        MacBoard::on(
            NSPasteboard::pasteboardWithUniqueName(),
            Arc::new(LayoutCache::new()),
        )
    }

    fn put(board: &MacBoard, formats: &[(&str, &[u8])]) {
        board.pasteboard.clearContents();
        let item = NSPasteboardItem::new();
        for (kind, bytes) in formats {
            item.setData_forType(&NSData::with_bytes(bytes), &NSString::from_str(kind));
        }
        let objects =
            NSArray::from_retained_slice(&[objc2::runtime::ProtocolObject::from_retained(item)]);
        board.pasteboard.writeObjects(&objects);
    }

    #[test]
    fn every_format_of_the_old_item_comes_back_byte_for_byte() {
        let mut board = board();
        let formats: [(&str, &[u8]); 3] = [
            ("public.utf8-plain-text", "sentinel é".as_bytes()),
            ("public.html", b"<b>old</b>"),
            ("com.example.blob", &[0, 1, 2, 255]),
        ];
        put(&board, &formats);
        let snapshot = board.snapshot(Selection::Clipboard);
        board.write(Selection::Clipboard, "the transcript").unwrap();
        assert!(board.still_ours(Selection::Clipboard));
        board.restore(Selection::Clipboard, snapshot);
        let mut back = board.snapshot(Selection::Clipboard);
        assert_eq!(back.len(), 1);
        let mut item = back.remove(0);
        item.sort();
        let mut want: Item = formats
            .iter()
            .map(|(kind, bytes)| ((*kind).to_owned(), bytes.to_vec()))
            .collect();
        want.sort();
        assert_eq!(item, want);
    }

    #[test]
    fn a_copy_after_the_write_is_seen_as_a_newer_copy() {
        let mut board = board();
        board.write(Selection::Clipboard, "the transcript").unwrap();
        assert!(board.still_ours(Selection::Clipboard));
        put(&board, &[("public.utf8-plain-text", b"user copy")]);
        assert!(!board.still_ours(Selection::Clipboard));
    }

    #[test]
    fn an_empty_old_clipboard_is_empty_again_after_the_restore() {
        let mut board = board();
        board.pasteboard.clearContents();
        let snapshot = board.snapshot(Selection::Clipboard);
        assert!(snapshot.is_empty());
        board.write(Selection::Clipboard, "the transcript").unwrap();
        board.restore(Selection::Clipboard, snapshot);
        assert!(board.snapshot(Selection::Clipboard).is_empty());
    }

    #[test]
    fn a_copied_selection_is_read_and_the_old_clipboard_comes_back() {
        let mut board = board();
        put(&board, &[("public.utf8-plain-text", b"sentinel")]);
        let app = MacBoard::on(board.pasteboard.clone(), Arc::clone(&board.layout));
        let text = copied_selection(&mut board, std::time::Duration::from_secs(1), || {
            app.pasteboard.clearContents();
            app.pasteboard
                .setString_forType(&NSString::from_str("the selection"), unsafe {
                    NSPasteboardTypeString
                });
            Ok(())
        });
        assert_eq!(text.as_deref(), Some("the selection"));
        let back = board.snapshot(Selection::Clipboard);
        assert_eq!(
            back,
            vec![vec![(
                "public.utf8-plain-text".to_owned(),
                b"sentinel".to_vec()
            )]]
        );
    }

    #[test]
    fn an_app_that_does_not_copy_gives_no_selection_and_leaves_the_clipboard() {
        let mut board = board();
        put(&board, &[("public.utf8-plain-text", b"sentinel")]);
        let text = copied_selection(&mut board, std::time::Duration::from_millis(60), || Ok(()));
        assert_eq!(text, None);
        assert_eq!(board.snapshot(Selection::Clipboard).len(), 1);
    }

    /// A worker thread is never the main thread, so a TIS call there would trip the debug
    /// assertion of the layout read. The keys for paste, copy, and undo come from the table.
    #[test]
    fn the_insert_worker_gets_its_keys_from_the_shared_table() {
        let layout = Arc::new(LayoutCache::new());
        layout.publish(keyboard_layout::Table::from_typed(
            keyboard_layout::layouts::dvorak,
        ));
        let inserter = MacInserter::with_layout(layout);
        let layout = Arc::clone(&inserter.layout);
        let keys = std::thread::Builder::new()
            .name("insert-worker".into())
            .spawn(move || {
                assert!(!keyboard_layout::is_main_thread());
                let board = MacBoard::on(NSPasteboard::pasteboardWithUniqueName(), layout);
                (
                    board.layout.v_key_code(),
                    board.layout.c_key_code(),
                    board.layout.z_key_code(),
                )
            })
            .unwrap()
            .join()
            .expect("the worker reads the shared table and calls no TIS function");
        assert_eq!(keys, (40, keyboard_layout::ANSI_C, keyboard_layout::ANSI_Z));
    }

    /// The background paste of a test build relies on this: AppKit asks the owner even when the
    /// read comes from Hushpen itself.
    #[cfg(feature = "test-automation")]
    #[test]
    fn a_read_in_this_process_is_a_receipt() {
        let mut board = board();
        board.write(Selection::Clipboard, "the transcript").unwrap();
        let text = board
            .pasteboard
            .stringForType(unsafe { NSPasteboardTypeString })
            .map(|text| text.to_string());
        assert_eq!(text.as_deref(), Some("the transcript"));
        assert!(board.receipts().first.is_some());
    }

    #[cfg(feature = "test-automation")]
    #[test]
    fn a_failed_background_paste_leaves_a_fresh_promise_with_no_receipt() {
        let mut board = board();
        board.write(Selection::Clipboard, "the transcript").unwrap();
        assert!(!board.ax_paste(i32::MAX));
        assert_eq!(board.receipts(), Receipts::default());
        assert!(board.still_ours(Selection::Clipboard));
        let text = board
            .pasteboard
            .stringForType(unsafe { NSPasteboardTypeString })
            .map(|text| text.to_string());
        assert_eq!(text.as_deref(), Some("the transcript"));
    }

    #[test]
    fn nothing_gets_a_read_receipt_before_a_read() {
        let mut board = board();
        board.write(Selection::Clipboard, "x").unwrap();
        assert_eq!(board.receipts(), Receipts::default());
    }
}
