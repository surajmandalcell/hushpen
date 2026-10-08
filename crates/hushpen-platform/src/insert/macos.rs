//! macOS insertion: a promised pasteboard string and a Cmd+V key event.
//!
//! The text goes on the pasteboard as a promise, so the owner is asked for the data when the
//! target reads it. That request is the receipt the restore waits for. The paste key is posted
//! as a single key event that carries the Command flag, so no modifier key is ever left down.

// The pasteboard owner class and the AppKit constants need objc2's `unsafe` calls.
#![allow(unsafe_code)]

mod guard;
mod layout;

use super::{Inserter, SystemClock, Target, mono_ms, plan};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use hushpen_core::insert::flow::{self, Backend, ChordError, Receipts, Request, Step};
use hushpen_core::insert::guard::{Block, check};
use hushpen_core::insert::{Chord, Overrides, Report, Selection, choose_mac};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardType, NSPasteboardTypeString, NSWorkspace,
};
use objc2_core_graphics::CGPreflightPostEventAccess;
use objc2_foundation::{NSArray, NSData, NSObject, NSString};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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
}

type Item = Vec<(String, Vec<u8>)>;

struct MacBoard {
    pasteboard: Retained<NSPasteboard>,
    owner: Option<Retained<Owner>>,
    /// The change count right after our write.
    count: isize,
}

impl MacBoard {
    fn general() -> Self {
        Self::on(NSPasteboard::generalPasteboard())
    }

    fn on(pasteboard: Retained<NSPasteboard>) -> Self {
        Self {
            pasteboard,
            owner: None,
            count: -1,
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
        if !CGPreflightPostEventAccess() {
            return Err(ChordError::NoPermission);
        }
        let post = |down: bool| -> Result<(), ChordError> {
            let source = CGEventSource::new(CGEventSourceStateID::Private)
                .map_err(|()| ChordError::Unavailable)?;
            let event = CGEvent::new_keyboard_event(source, layout::v_key_code(), down)
                .map_err(|()| ChordError::Unavailable)?;
            event.set_flags(CGEventFlags::CGEventFlagCommand);
            event.post(CGEventTapLocation::HID);
            Ok(())
        };
        post(true)?;
        std::thread::sleep(std::time::Duration::from_millis(8));
        post(false)
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

pub(super) struct MacInserter {
    one_at_a_time: Mutex<()>,
}

impl MacInserter {
    pub(super) fn new() -> Self {
        Self {
            one_at_a_time: Mutex::new(()),
        }
    }
}

impl Inserter for MacInserter {
    fn target(&self) -> Target {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication();
        let Some(app) = app else {
            return Target::default();
        };
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
        let mut board = MacBoard::general();
        let request = Request {
            text,
            target: &target.label,
            method,
            copy_note,
        };
        flow::run(&mut board, &SystemClock, &request, observe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A private pasteboard, so no test touches the user's clipboard.
    fn board() -> MacBoard {
        MacBoard::on(NSPasteboard::pasteboardWithUniqueName())
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
    fn nothing_gets_a_read_receipt_before_a_read() {
        let mut board = board();
        board.write(Selection::Clipboard, "x").unwrap();
        assert_eq!(board.receipts(), Receipts::default());
    }
}
