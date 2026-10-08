//! The X11 clipboard as the insertion flow needs it.
//!
//! One connection and window of ours owns CLIPBOARD or PRIMARY while a text is pasted, and
//! answers every `SelectionRequest` on its own thread. The first request for the text is the
//! receipt. A second connection reads the selection that was there before (every target the owner
//! offers) so it can be offered again afterwards, and sends the paste chord through XTest.

use super::grab::GrabProbe;
use super::{Atoms, chord};
use crate::insert::mono_ms;
use hushpen_core::insert::flow::{Backend, ChordError, Receipts};
use hushpen_core::insert::guard::{Block, check};
use hushpen_core::insert::{Chord, Selection};
use std::error::Error;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Property,
    SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};

/// The most bytes of one clipboard that are saved. A larger target is not restored.
const SNAPSHOT_CAP_BYTES: usize = 32 * 1024 * 1024;
/// How long one target may take to arrive.
const FETCH_TIMEOUT: Duration = Duration::from_millis(400);
/// How long the whole snapshot may take. An owner that does not answer must not stall the paste.
const SNAPSHOT_BUDGET: Duration = Duration::from_millis(1_500);
const POLL: Duration = Duration::from_micros(300);

/// One format of a selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Item {
    target: Atom,
    ptype: Atom,
    format: u8,
    data: Vec<u8>,
}

/// Everything a selection offers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Offer {
    items: Vec<Item>,
}

/// Latin-1 for the `STRING` target; characters beyond it become `?`.
fn latin1(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

impl Offer {
    fn text(text: &str, atoms: &Atoms) -> Self {
        let utf8 = |target| Item {
            target,
            ptype: atoms.UTF8_STRING,
            format: 8,
            data: text.as_bytes().to_vec(),
        };
        Self {
            items: vec![
                utf8(atoms.UTF8_STRING),
                Item {
                    target: atoms.TEXT_PLAIN_UTF8,
                    ptype: atoms.TEXT_PLAIN_UTF8,
                    format: 8,
                    data: text.as_bytes().to_vec(),
                },
                Item {
                    target: atoms.TEXT_PLAIN,
                    ptype: atoms.TEXT_PLAIN,
                    format: 8,
                    data: text.as_bytes().to_vec(),
                },
                utf8(atoms.TEXT),
                Item {
                    target: atoms.STRING,
                    ptype: atoms.STRING,
                    format: 8,
                    data: latin1(text),
                },
            ],
        }
    }

    fn item(&self, target: Atom) -> Option<&Item> {
        self.items.iter().find(|item| item.target == target)
    }
}

#[derive(Default)]
struct Slot {
    offer: Option<Offer>,
    receipts: Receipts,
}

#[derive(Default)]
struct Slots {
    clipboard: Slot,
    primary: Slot,
    /// The X client whose reads count as receipts: the target app. A clipboard manager or any
    /// other program that reads the text is served but proves nothing about the paste.
    reader_client: Option<u32>,
}

impl Slots {
    fn get(&mut self, selection: Selection) -> &mut Slot {
        match selection {
            Selection::Clipboard => &mut self.clipboard,
            Selection::Primary => &mut self.primary,
        }
    }
}

/// The owner connection, shared with the thread that answers requests.
struct Shared {
    conn: RustConnection,
    window: Window,
    atoms: Atoms,
    slots: Mutex<Slots>,
    /// The bits of a resource id that the server gives to one client.
    resource_mask: u32,
}

/// The id bits that name the client a window belongs to.
fn client_of(window: Window, resource_mask: u32) -> u32 {
    window & !resource_mask
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn new_window(
    conn: &RustConnection,
    root: Window,
    properties: bool,
) -> Result<Window, Box<dyn Error>> {
    let window = conn.generate_id()?;
    let mut aux = CreateWindowAux::new();
    if properties {
        aux = aux.event_mask(EventMask::PROPERTY_CHANGE);
    }
    // An input-only window is never mapped, so no window manager or pager ever lists it.
    conn.create_window(
        0,
        window,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &aux,
    )?
    .check()?;
    Ok(window)
}

impl Shared {
    fn selection_atom(&self, selection: Selection) -> Atom {
        match selection {
            Selection::Clipboard => self.atoms.CLIPBOARD,
            Selection::Primary => self.atoms.PRIMARY,
        }
    }

    fn selection_of(&self, atom: Atom) -> Option<Selection> {
        if atom == self.atoms.CLIPBOARD {
            Some(Selection::Clipboard)
        } else if atom == self.atoms.PRIMARY {
            Some(Selection::Primary)
        } else {
            None
        }
    }

    fn serve(self: &Arc<Self>) {
        loop {
            match self.conn.wait_for_event() {
                Ok(Event::SelectionRequest(request)) => self.answer(&request),
                Ok(_) => {}
                Err(error) => {
                    log::warn!("the clipboard owner lost the X connection: {error}");
                    return;
                }
            }
        }
    }

    fn answer(&self, request: &SelectionRequestEvent) {
        let property = if request.property == NONE {
            request.target
        } else {
            request.property
        };
        let served = self.fill(
            request.selection,
            request.requestor,
            request.target,
            property,
        );
        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if served { property } else { NONE },
        };
        let _ = self
            .conn
            .send_event(false, request.requestor, EventMask::NO_EVENT, notify);
        let _ = self.conn.flush();
    }

    /// Writes the answer for `target` into `property` of the requestor. False when there is none.
    fn fill(&self, selection: Atom, requestor: Window, target: Atom, property: Atom) -> bool {
        let Some(selection) = self.selection_of(selection) else {
            return false;
        };
        if target == self.atoms.MULTIPLE {
            return self.fill_multiple(selection, requestor, property);
        }
        let mut slots = lock(&self.slots);
        let counts = slots
            .reader_client
            .is_none_or(|client| client == client_of(requestor, self.resource_mask));
        let slot = slots.get(selection);
        let Some(offer) = slot.offer.as_ref() else {
            return false;
        };
        if target == self.atoms.TARGETS {
            let mut list = vec![self.atoms.TARGETS, self.atoms.MULTIPLE];
            list.extend(offer.items.iter().map(|item| item.target));
            return self
                .conn
                .change_property32(
                    PropMode::REPLACE,
                    requestor,
                    property,
                    AtomEnum::ATOM,
                    &list,
                )
                .is_ok();
        }
        let Some(item) = offer.item(target) else {
            return false;
        };
        let unit = usize::from(item.format / 8).max(1);
        let Ok(len) = u32::try_from(item.data.len() / unit) else {
            return false;
        };
        if item.data.len() >= self.conn.maximum_request_bytes().saturating_sub(64) {
            log::warn!("a clipboard format is too large to offer without INCR and was skipped");
            return false;
        }
        let sent = self
            .conn
            .change_property(
                PropMode::REPLACE,
                requestor,
                property,
                item.ptype,
                item.format,
                len,
                &item.data,
            )
            .is_ok();
        if sent && counts {
            let now = mono_ms();
            slot.receipts.first.get_or_insert(now);
            slot.receipts.last = Some(now);
        }
        sent
    }

    /// `MULTIPLE`: the property holds (target, property) pairs. Each pair that cannot be served
    /// gets `None` as its property.
    fn fill_multiple(&self, selection: Selection, requestor: Window, property: Atom) -> bool {
        let Some(pairs) = self
            .conn
            .get_property(false, requestor, property, self.atoms.ATOM_PAIR, 0, 1024)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().map(Iterator::collect::<Vec<u32>>))
        else {
            return false;
        };
        let selection_atom = self.selection_atom(selection);
        let mut answered = pairs.clone();
        let mut any = false;
        for pair in answered.as_chunks_mut::<2>().0 {
            let [target, target_property] = *pair;
            if self.fill(selection_atom, requestor, target, target_property) {
                any = true;
            } else {
                pair[1] = NONE;
            }
        }
        let wrote = self
            .conn
            .change_property32(
                PropMode::REPLACE,
                requestor,
                property,
                self.atoms.ATOM_PAIR,
                &answered,
            )
            .is_ok();
        wrote && any
    }
}

/// What came back for one target of a selection.
struct Fetched {
    ptype: Atom,
    format: u8,
    data: Vec<u8>,
}

/// The connection that reads other owners' selections and sends the paste chord.
struct Reader {
    conn: RustConnection,
    root: Window,
    window: Window,
}

impl Reader {
    fn wait_for(&self, deadline: Instant, wanted: impl Fn(&Event) -> bool) -> Option<Event> {
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(event)) if wanted(&event) => return Some(event),
                Ok(Some(_)) => {}
                Ok(None) => {
                    if Instant::now() >= deadline {
                        return None;
                    }
                    thread::sleep(POLL);
                }
                Err(_) => return None,
            }
        }
    }

    fn read_property(&self, atoms: &Atoms) -> Option<Fetched> {
        let words = u32::try_from(SNAPSHOT_CAP_BYTES / 4).ok()?;
        let reply = self
            .conn
            .get_property(
                true,
                self.window,
                atoms.HUSHPEN_SELECTION,
                AtomEnum::ANY,
                0,
                words,
            )
            .ok()?
            .reply()
            .ok()?;
        if reply.bytes_after > 0 {
            let _ = self
                .conn
                .delete_property(self.window, atoms.HUSHPEN_SELECTION);
            return None;
        }
        Some(Fetched {
            ptype: reply.type_,
            format: reply.format,
            data: reply.value,
        })
    }

    /// Asks the owner of `selection` for `target` and reads the answer, chunks included.
    fn fetch(
        &self,
        atoms: &Atoms,
        selection: Atom,
        target: Atom,
        budget_end: Instant,
    ) -> Option<Fetched> {
        let deadline = (Instant::now() + FETCH_TIMEOUT).min(budget_end);
        let window = self.window;
        let _ = self.conn.delete_property(window, atoms.HUSHPEN_SELECTION);
        self.conn
            .convert_selection(
                window,
                selection,
                target,
                atoms.HUSHPEN_SELECTION,
                CURRENT_TIME,
            )
            .ok()?;
        self.conn.flush().ok()?;
        let notify = self.wait_for(deadline, |event| {
            matches!(event, Event::SelectionNotify(n)
                if n.requestor == window && n.selection == selection && n.target == target)
        })?;
        let Event::SelectionNotify(notify) = notify else {
            return None;
        };
        if notify.property == NONE {
            return None;
        }
        let first = self.read_property(atoms)?;
        if first.ptype != atoms.INCR {
            return Some(first);
        }
        // The owner sends the data in chunks. Each new chunk is announced by a property change;
        // an empty chunk ends the transfer.
        let mut chunks = Fetched {
            ptype: AtomEnum::ANY.into(),
            format: 8,
            data: Vec::new(),
        };
        let mut typed = false;
        loop {
            let deadline = (Instant::now() + FETCH_TIMEOUT).min(budget_end);
            self.wait_for(deadline, |event| {
                matches!(event, Event::PropertyNotify(p)
                    if p.window == window
                        && p.atom == atoms.HUSHPEN_SELECTION
                        && p.state == Property::NEW_VALUE)
            })?;
            let chunk = self.read_property(atoms)?;
            if chunk.data.is_empty() {
                return typed.then_some(chunks);
            }
            if !typed {
                chunks.ptype = chunk.ptype;
                chunks.format = chunk.format;
                typed = true;
            }
            if chunks.data.len() + chunk.data.len() > SNAPSHOT_CAP_BYTES {
                return None;
            }
            chunks.data.extend_from_slice(&chunk.data);
        }
    }

    /// Every format the current owner of `selection` offers. `None` when it cannot be read.
    fn read_offer(&self, atoms: &Atoms, selection: Atom) -> Option<Offer> {
        let budget_end = Instant::now() + SNAPSHOT_BUDGET;
        let list = self.fetch(atoms, selection, atoms.TARGETS, budget_end)?;
        let targets: Vec<Atom> = list
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_ne_bytes(*word))
            .collect();
        let skipped = [
            atoms.TARGETS,
            atoms.MULTIPLE,
            atoms.TIMESTAMP,
            atoms.SAVE_TARGETS,
            atoms.DELETE,
            atoms.INCR,
            NONE,
        ];
        let mut offer = Offer::default();
        let mut total = 0usize;
        for target in targets.into_iter().filter(|t| !skipped.contains(t)) {
            if Instant::now() >= budget_end {
                break;
            }
            let Some(fetched) = self.fetch(atoms, selection, target, budget_end) else {
                continue;
            };
            total += fetched.data.len();
            if total > SNAPSHOT_CAP_BYTES {
                break;
            }
            offer.items.push(Item {
                target,
                ptype: fetched.ptype,
                format: fetched.format,
                data: fetched.data,
            });
        }
        Some(offer)
    }

    fn owner(&self, selection: Atom) -> Option<Window> {
        self.conn
            .get_selection_owner(selection)
            .ok()?
            .reply()
            .ok()
            .map(|reply| reply.owner)
    }
}

pub(super) struct X11Board {
    shared: Arc<Shared>,
    reader: Reader,
    /// The selection of the last write: the one whose receipts count.
    current: Selection,
}

impl X11Board {
    pub(super) fn start() -> Result<Self, Box<dyn Error>> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms::new(&conn)?.reply()?;
        let window = new_window(&conn, root, false)?;
        let resource_mask = conn.setup().resource_id_mask;
        let shared = Arc::new(Shared {
            conn,
            window,
            atoms,
            slots: Mutex::default(),
            resource_mask,
        });
        let server = Arc::clone(&shared);
        thread::Builder::new()
            .name("hushpen-clipboard".into())
            .spawn(move || server.serve())?;

        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots[screen].root;
        let window = new_window(&conn, root, true)?;
        Ok(Self {
            shared,
            reader: Reader { conn, root, window },
            current: Selection::Clipboard,
        })
    }

    /// Only reads by the client that owns `window` count as receipts. `None` counts every read.
    pub(super) fn count_reads_of(&self, window: Option<Window>) {
        let mask = self.shared.resource_mask;
        lock(&self.shared.slots).reader_client = window.map(|window| client_of(window, mask));
    }

    fn own(&self, selection: Selection, offer: Option<Offer>) -> Result<(), String> {
        let atom = self.shared.selection_atom(selection);
        {
            let mut slots = lock(&self.shared.slots);
            let slot = slots.get(selection);
            slot.receipts = Receipts::default();
            slot.offer = offer.clone();
        }
        let owner = if offer.is_some() {
            self.shared.window
        } else {
            NONE
        };
        let conn = &self.shared.conn;
        conn.set_selection_owner(owner, atom, CURRENT_TIME)
            .map_err(|e| e.to_string())?;
        conn.flush().map_err(|e| e.to_string())?;
        // Asked on the connection that took the selection: another connection can be served
        // before the request that set the owner, and would still name the old owner.
        let taken = conn
            .get_selection_owner(atom)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map(|reply| reply.owner);
        if offer.is_some() && taken != Some(self.shared.window) {
            return Err("the clipboard was taken at once by another app".into());
        }
        Ok(())
    }
}

impl Backend for X11Board {
    type Snapshot = Option<Offer>;

    fn guard(&mut self) -> Option<Block> {
        check(&GrabProbe {
            conn: &self.reader.conn,
            root: self.reader.root,
        })
    }

    fn snapshot(&mut self, selection: Selection) -> Self::Snapshot {
        let atom = self.shared.selection_atom(selection);
        match self.reader.owner(atom) {
            Some(NONE) | None => None,
            Some(owner) if owner == self.shared.window => {
                lock(&self.shared.slots).get(selection).offer.clone()
            }
            Some(_) => self.reader.read_offer(&self.shared.atoms, atom),
        }
    }

    fn write(&mut self, selection: Selection, text: &str) -> Result<(), String> {
        self.current = selection;
        let offer = Offer::text(text, &self.shared.atoms);
        self.own(selection, Some(offer))
    }

    fn send_chord(&mut self, chord: Chord) -> Result<(), ChordError> {
        chord::send(&self.reader.conn, self.reader.root, chord)
    }

    fn receipts(&mut self) -> Receipts {
        lock(&self.shared.slots).get(self.current).receipts
    }

    fn still_ours(&mut self, selection: Selection) -> bool {
        let atom = self.shared.selection_atom(selection);
        self.reader.owner(atom) == Some(self.shared.window)
    }

    fn restore(&mut self, selection: Selection, snapshot: Self::Snapshot) {
        // With nothing to offer, giving up the selection leaves it empty, as it was.
        let offer = snapshot.filter(|offer| !offer.items.is_empty());
        if let Err(error) = self.own(selection, offer) {
            log::warn!("the clipboard could not be restored: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_is_latin_1_and_other_characters_become_question_marks() {
        assert_eq!(latin1("café"), [b'c', b'a', b'f', 0xe9]);
        assert_eq!(latin1("a🎤ß"), [b'a', b'?', 0xdf]);
    }

    #[test]
    fn windows_of_one_client_share_the_id_bits_above_the_resource_mask() {
        let mask = 0x001f_ffff;
        assert_eq!(client_of(0x0460_0001, mask), client_of(0x0460_1234, mask));
        assert_ne!(client_of(0x0460_0001, mask), client_of(0x0480_0001, mask));
    }
}
