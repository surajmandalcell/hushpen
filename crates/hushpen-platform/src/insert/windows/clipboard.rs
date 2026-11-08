//! A clipboard owner whose delayed Unicode format records a target read.

use super::{Target, chord_keys, focused_field, process_id, same_target, send_keys};
use hushpen_core::insert::flow::{Backend, ChordError, Receipts};
use hushpen_core::insert::guard::Block;
use hushpen_core::insert::{Chord, Selection};
use std::cell::RefCell;
use std::mem::size_of;
use std::sync::OnceLock;
use std::time::Duration;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    DeleteEnhMetaFile, DeleteMetaFile, DeleteObject, HENHMETAFILE, HGDIOBJ,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardOwner,
    GetClipboardSequenceNumber, GetOpenClipboardWindow, METAFILEPICT, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::{CLIPBOARD_FORMAT, OleDuplicateData};
use windows::Win32::UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA,
    GetWindowLongPtrW, GetWindowThreadProcessId, HWND_MESSAGE, MSG, PM_REMOVE, PeekMessageW,
    RegisterClassW, SetWindowLongPtrW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_NCCREATE,
    WM_RENDERALLFORMATS, WM_RENDERFORMAT, WNDCLASSW,
};
use windows::core::w;

const UNICODE: u32 = 13;

struct Open;
impl Open {
    fn new(hwnd: HWND) -> Option<Self> {
        for _ in 0..20 {
            // SAFETY: The optional window belongs to this thread; opening grants clipboard access only.
            if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
                return Some(Self);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        None
    }
}
impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: This guard owns one successful OpenClipboard call on this thread.
        let _ = unsafe { CloseClipboard() };
    }
}

pub(super) struct Format {
    format: u32,
    handle: HANDLE,
}
impl Drop for Format {
    fn drop(&mut self) {
        if self.handle.is_invalid() {
            return;
        }
        // SAFETY: Each handle is an owned OleDuplicateData result, freed by its documented format type.
        unsafe {
            match self.format {
                2 | 9 | 130 => {
                    let _ = DeleteObject(HGDIOBJ(self.handle.0));
                }
                14 | 142 => {
                    let _ = DeleteEnhMetaFile(Some(HENHMETAFILE(self.handle.0)));
                }
                3 | 131 => {
                    let global = HGLOBAL(self.handle.0);
                    let value = GlobalLock(global).cast::<METAFILEPICT>();
                    if !value.is_null() {
                        let _ = DeleteMetaFile((*value).hMF);
                        let _ = GlobalUnlock(global);
                    }
                    let _ = GlobalFree(Some(global));
                }
                _ => {
                    let _ = GlobalFree(Some(HGLOBAL(self.handle.0)));
                }
            }
        }
    }
}

#[derive(Default)]
struct State {
    text: Vec<u16>,
    receipts: Receipts,
    sequence: u32,
    target_pid: Option<u32>,
    sent: bool,
    rendered: bool,
}

pub(super) struct Board {
    target: Target,
    hwnd: HWND,
    state: Box<RefCell<State>>,
    snapshot_ok: bool,
}

impl Board {
    pub(super) fn new(target: Target) -> Self {
        static CLASS: OnceLock<bool> = OnceLock::new();
        let registered = *CLASS.get_or_init(|| {
            // SAFETY: The module query returns the current executable's borrowed module handle.
            let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default().into();
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                lpszClassName: w!("HushpenClipboard"),
                hInstance: instance,
                ..Default::default()
            };
            // SAFETY: The class uses static name storage and a process-lifetime callback.
            unsafe { RegisterClassW(&class) != 0 }
        });
        let mut state = Box::new(RefCell::new(State {
            target_pid: process_id(&target),
            ..Default::default()
        }));
        let hwnd = if registered {
            // SAFETY: The module remains loaded for the lifetime of every clipboard window.
            let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default().into();
            // SAFETY: The boxed state stays at this address until this thread destroys its message-only window.
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("HushpenClipboard"),
                    w!(""),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    Some(instance),
                    Some((&mut *state as *mut RefCell<State>).cast()),
                )
            }
            .unwrap_or_default()
        } else {
            HWND::default()
        };
        Self {
            target,
            hwnd,
            state,
            snapshot_ok: true,
        }
    }

    fn pump(&self) {
        let mut message = MSG::default();
        // SAFETY: Only messages for this thread's clipboard window are removed and dispatched.
        unsafe {
            while PeekMessageW(&mut message, Some(self.hwnd), 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&message);
            }
        }
    }

    pub(super) fn copy_selection(&mut self) -> Option<String> {
        if self.hwnd.is_invalid() || !same_target(&self.target, &super::focus()) {
            return None;
        }
        let snapshot = self.snapshot(Selection::Clipboard);
        if !self.snapshot_ok {
            return None;
        }
        // SAFETY: This query reads the shared clipboard counter without changing its contents.
        let before = unsafe { GetClipboardSequenceNumber() };
        send_keys(&self.target, &[VK_CONTROL, VIRTUAL_KEY(b'C' as u16)]).ok()?;
        let deadline = std::time::Instant::now() + Duration::from_millis(700);
        let copied = loop {
            // SAFETY: The counter is a value query; a changed value means the target produced clipboard data.
            let now = unsafe { GetClipboardSequenceNumber() };
            if now != before {
                let mut owner_pid = 0;
                // SAFETY: Ownership is read only; the process output is local and checked before restoring.
                unsafe {
                    if let Ok(owner) = GetClipboardOwner() {
                        GetWindowThreadProcessId(owner, Some(&mut owner_pid));
                    }
                }
                if Some(owner_pid) != process_id(&self.target) {
                    return None;
                }
                break now;
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let _open = Open::new(self.hwnd)?;
        // SAFETY: The counter is read with exclusive clipboard access before any restore.
        if unsafe { GetClipboardSequenceNumber() } != copied {
            return None;
        }
        // SAFETY: Clipboard memory remains valid and locked until the copied UTF-16 units are owned.
        let text = (|| unsafe {
            let handle = GetClipboardData(UNICODE).ok()?;
            let global = HGLOBAL(handle.0);
            let units = GlobalSize(global) / size_of::<u16>();
            let data = GlobalLock(global).cast::<u16>();
            if data.is_null() {
                return None;
            }
            let data_slice = std::slice::from_raw_parts(data, units);
            let end = data_slice
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(units);
            let text = String::from_utf16_lossy(&data_slice[..end]);
            let _ = GlobalUnlock(global);
            (!text.is_empty()).then_some(text)
        })();
        // The counter is checked while the clipboard is open, so a newer copy cannot be overwritten.
        restore_open(snapshot);
        text
    }
}

impl Backend for Board {
    type Snapshot = Vec<Format>;

    fn guard(&mut self) -> Option<Block> {
        if focused_field(&self.target).is_some_and(|field| field.password) {
            Some(Block::SecureField)
        } else {
            None
        }
    }

    fn snapshot(&mut self, _selection: Selection) -> Self::Snapshot {
        let Some(_open) = Open::new(self.hwnd) else {
            self.snapshot_ok = false;
            return Vec::new();
        };
        let mut formats = Vec::new();
        let mut format = 0;
        // SAFETY: The clipboard is open; each duplicate is independent of the clipboard owner's handle.
        unsafe {
            loop {
                format = EnumClipboardFormats(format);
                if format == 0 {
                    break;
                }
                if !copyable_format(format) {
                    continue;
                }
                let Ok(handle) = GetClipboardData(format) else {
                    continue;
                };
                let duplicate =
                    OleDuplicateData(handle, CLIPBOARD_FORMAT(format as u16), GMEM_MOVEABLE);
                if !duplicate.is_invalid() {
                    formats.push(Format {
                        format,
                        handle: duplicate,
                    });
                }
            }
        }
        formats
    }

    fn write(&mut self, _selection: Selection, text: &str) -> Result<(), String> {
        if self.hwnd.is_invalid() || !self.snapshot_ok {
            return Err("The clipboard could not be saved.".into());
        }
        let _open = Open::new(self.hwnd).ok_or("The clipboard is busy.")?;
        {
            let state = self.state.get_mut();
            state.text = text.encode_utf16().chain(Some(0)).collect();
            state.receipts = Receipts::default();
            state.rendered = false;
            state.sent = false;
        }
        // SAFETY: This window owns the open clipboard; its callback supplies the delayed format.
        unsafe {
            EmptyClipboard().map_err(|error| error.to_string())?;
            // A null handle is the successful delayed-render path; the wrapper treats it as an error.
            let _ = SetClipboardData(UNICODE, None);
            self.state.get_mut().sequence = GetClipboardSequenceNumber();
        }
        Ok(())
    }

    fn send_chord(&mut self, chord: Chord) -> Result<(), ChordError> {
        self.state.get_mut().sent = true;
        send_keys(&self.target, chord_keys(chord))
    }

    fn receipts(&mut self) -> Receipts {
        self.pump();
        self.state.borrow().receipts
    }

    fn still_ours(&mut self, _selection: Selection) -> bool {
        // SAFETY: Both calls only read ownership and sequence values.
        unsafe {
            GetClipboardOwner().ok() == Some(self.hwnd)
                && GetClipboardSequenceNumber() == self.state.borrow().sequence
        }
    }

    fn restore(&mut self, _selection: Selection, snapshot: Self::Snapshot) {
        let Some(_open) = Open::new(self.hwnd) else {
            log::warn!("The clipboard was busy during restore");
            return;
        };
        if self.still_ours(Selection::Clipboard) {
            restore_open(snapshot);
        }
    }
}

impl Drop for Board {
    fn drop(&mut self) {
        if self.hwnd.is_invalid() {
            return;
        }
        if let Some(_open) = Open::new(self.hwnd) {
            // SAFETY: The clipboard is open; a kept copy must become real data before its owner window goes away.
            unsafe {
                if GetClipboardOwner().ok() == Some(self.hwnd) && !self.state.borrow().rendered {
                    render(&mut self.state.borrow_mut());
                }
            }
        }
        // SAFETY: This thread created the window and its boxed state is still live during destruction.
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}

fn copyable_format(format: u32) -> bool {
    format != 128 && !(0x300..=0x3ff).contains(&format) && u16::try_from(format).is_ok()
}

fn restore_open(snapshot: Vec<Format>) {
    // SAFETY: The caller holds the clipboard open. Successful transfers give Windows ownership of each handle.
    unsafe {
        if let Err(error) = EmptyClipboard() {
            log::warn!("Could not clear the clipboard during restore: {error}");
            return;
        }
        for mut format in snapshot {
            if SetClipboardData(format.format, Some(format.handle)).is_ok() {
                format.handle = HANDLE::default();
            } else {
                log::warn!("Could not restore clipboard format {}", format.format);
            }
        }
    }
}

fn render(state: &mut State) {
    if state.rendered || state.text.is_empty() {
        return;
    }
    // SAFETY: The movable allocation holds the complete UTF-16 string and transfers to Windows only on success.
    unsafe {
        let Ok(memory) = GlobalAlloc(GMEM_MOVEABLE, state.text.len() * size_of::<u16>()) else {
            return;
        };
        let data = GlobalLock(memory).cast::<u16>();
        if data.is_null() {
            let _ = GlobalFree(Some(memory));
            return;
        }
        std::ptr::copy_nonoverlapping(state.text.as_ptr(), data, state.text.len());
        let _ = GlobalUnlock(memory);
        if SetClipboardData(UNICODE, Some(HANDLE(memory.0))).is_ok() {
            state.rendered = true;
            state.sequence = GetClipboardSequenceNumber();
        } else {
            let _ = GlobalFree(Some(memory));
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Windows invokes this callback with valid message pointers. User data is our live boxed State.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RefCell<State>;
        if !state.is_null() {
            if message == WM_RENDERFORMAT && wparam.0 as u32 == UNICODE {
                let Ok(mut state) = (*state).try_borrow_mut() else {
                    return LRESULT(0);
                };
                let mut reader = 0;
                if let Ok(window) = GetOpenClipboardWindow() {
                    GetWindowThreadProcessId(window, Some(&mut reader));
                }
                render(&mut state);
                if state.rendered && state.sent && Some(reader) == state.target_pid {
                    let now = super::super::mono_ms();
                    state.receipts.first.get_or_insert(now);
                    state.receipts.last = Some(now);
                }
                return LRESULT(0);
            }
            if message == WM_RENDERALLFORMATS {
                if let Some(_open) = Open::new(hwnd)
                    && GetClipboardOwner().ok() == Some(hwnd)
                    && let Ok(mut state) = (*state).try_borrow_mut()
                {
                    render(&mut state);
                }
                return LRESULT(0);
            }
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_copyable_formats_are_saved() {
        for format in [
            1, 2, 3, 8, 9, 13, 14, 15, 17, 129, 130, 131, 142, 0xc001, 0xffff,
        ] {
            assert!(copyable_format(format), "{format}");
        }
        for format in [128, 0x300, 0x3ff, 0x10000] {
            assert!(!copyable_format(format), "{format}");
        }
    }
}
