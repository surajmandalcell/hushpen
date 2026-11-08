//! Windows insertion, guarded by foreground window, process, and integrity level.

#![allow(unsafe_code)]

mod clipboard;

use super::{Inserter, Target, UndoError, plan, undo_guard};
use clipboard::Board;
use hushpen_core::insert::flow::{self, ChordError, Request, Step};
use hushpen_core::insert::{Chord, Overrides, Report, choose_windows};
use std::mem::size_of;
use std::sync::Mutex;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_INSERT, VK_LWIN, VK_MENU, VK_RWIN,
    VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
};
use windows::core::PWSTR;

#[derive(Default)]
pub(super) struct WindowsInserter {
    one_at_a_time: Mutex<()>,
}

impl Inserter for WindowsInserter {
    fn target(&self) -> Target {
        focus()
    }

    fn insert(
        &self,
        text: &str,
        target: &Target,
        overrides: &Overrides,
        observe: &mut dyn FnMut(Step<'_>),
    ) -> Report {
        let _lock = self.one_at_a_time.lock().unwrap_or_else(|p| p.into_inner());
        let (method, copy_note) = plan(target, |target| choose_windows(&target.classes, overrides));
        let mut board = Board::new(target.clone());
        flow::run(
            &mut board,
            &super::SystemClock,
            &Request {
                text,
                target: &target.label,
                method,
                copy_note,
            },
            observe,
        )
    }

    fn selection(&self, target: &Target) -> Option<String> {
        let _lock = self.one_at_a_time.lock().unwrap_or_else(|p| p.into_inner());
        if !same_target(target, &focus()) || target.own {
            return None;
        }
        let field = focused_field(target)?;
        if field.password {
            return None;
        }
        if let Some(text) = field.text {
            return Some(text);
        }
        Board::new(target.clone()).copy_selection()
    }

    fn undo(&self, target: &Target) -> Result<(), UndoError> {
        let _lock = self.one_at_a_time.lock().unwrap_or_else(|p| p.into_inner());
        undo_guard(target, &focus())?;
        send_keys(target, &[VK_CONTROL, VIRTUAL_KEY(b'Z' as u16)]).map_err(|error| match error {
            ChordError::HigherIntegrity | ChordError::NoPermission => UndoError::NoPermission,
            ChordError::Unavailable => UndoError::Unavailable,
        })
    }
}

fn focus() -> Target {
    // SAFETY: These queries accept a null foreground window and write only to local buffers.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return Target::default();
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut class = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        let exe = process_name(pid).unwrap_or_default();
        Target {
            window: Some(hwnd.0 as usize as u64),
            label: if exe.is_empty() {
                class.clone()
            } else {
                exe.clone()
            },
            classes: vec![exe, class, format!("pid:{pid}")],
            own: pid == GetCurrentProcessId(),
        }
    }
}

fn process_name(pid: u32) -> Option<String> {
    // SAFETY: The process is opened for queries only; the buffer and length cover the full path.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 32768];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        Some(path.rsplit(['\\', '/']).next()?.to_owned())
    }
}

fn process_id(target: &Target) -> Option<u32> {
    target
        .classes
        .iter()
        .find_map(|class| class.strip_prefix("pid:")?.parse().ok())
}

fn same_target(target: &Target, now: &Target) -> bool {
    target.window.is_some()
        && target.window == now.window
        && process_id(target).is_some()
        && process_id(target) == process_id(now)
        && !now.own
}

fn integrity(process: HANDLE) -> Option<u32> {
    // SAFETY: Token storage is pointer-aligned and remains live while its embedded SID is read.
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut bytes = 0;
        let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut bytes);
        let mut storage = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        let result = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(storage.as_mut_ptr().cast()),
            bytes,
            &mut bytes,
        );
        let _ = CloseHandle(token);
        result.ok()?;
        if (bytes as usize) < size_of::<TOKEN_MANDATORY_LABEL>() {
            return None;
        }
        let label = &*storage.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let count = *GetSidSubAuthorityCount(label.Label.Sid);
        if count == 0 {
            return None;
        }
        Some(*GetSidSubAuthority(label.Label.Sid, u32::from(count - 1)))
    }
}

fn target_integrity(target: &Target) -> Result<(), ChordError> {
    let pid = process_id(target).ok_or(ChordError::Unavailable)?;
    // SAFETY: Both handles permit token queries; the owned target process handle is closed once.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| ChordError::Unavailable)?;
        let target = integrity(process);
        let _ = CloseHandle(process);
        match (integrity(GetCurrentProcess()), target) {
            (Some(ours), Some(theirs)) if theirs > ours => Err(ChordError::HigherIntegrity),
            (Some(_), Some(_)) => Ok(()),
            _ => Err(ChordError::Unavailable),
        }
    }
}

struct Field {
    password: bool,
    text: Option<String>,
}

struct Com(bool);
impl Com {
    fn start() -> Option<Self> {
        // SAFETY: This thread balances each successful COM initialization with one uninitialization.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_ok() {
            Some(Self(true))
        } else if result.0 == 0x80010106u32 as i32 {
            Some(Self(false))
        } else {
            None
        }
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: This guard owns a successful initialization on the same thread.
            unsafe { CoUninitialize() };
        }
    }
}

fn focused_field(target: &Target) -> Option<Field> {
    let _com = Com::start()?;
    // SAFETY: COM is initialized on this thread and the returned interfaces do not escape it.
    unsafe {
        let automation: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let element = automation.GetFocusedElement().ok()?;
        if u32::try_from(element.CurrentProcessId().ok()?).ok()? != process_id(target)? {
            return None;
        }
        let password = element.CurrentIsPassword().ok()?.as_bool();
        let text = if password {
            None
        } else {
            (|| {
                let pattern: IUIAutomationTextPattern =
                    element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
                let selection = pattern.GetSelection().ok()?;
                let mut text = String::new();
                for index in 0..selection.Length().ok()? {
                    text.push_str(
                        &selection
                            .GetElement(index)
                            .ok()?
                            .GetText(-1)
                            .ok()?
                            .to_string(),
                    );
                }
                (!text.is_empty()).then_some(text)
            })()
        };
        Some(Field { password, text })
    }
}

fn input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                ..Default::default()
            },
        },
    }
}

fn send_keys(target: &Target, keys: &[VIRTUAL_KEY]) -> Result<(), ChordError> {
    target_integrity(target)?;
    if !same_target(target, &focus()) {
        return Err(ChordError::Unavailable);
    }
    let field = focused_field(target).ok_or(ChordError::Unavailable)?;
    if field.password {
        return Err(ChordError::NoPermission);
    }
    // SAFETY: Key-state queries have no side effects and use defined virtual-key codes.
    if [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
    {
        return Err(ChordError::Unavailable);
    }
    let events: Vec<_> = keys
        .iter()
        .map(|key| input(*key, false))
        .chain(keys.iter().rev().map(|key| input(*key, true)))
        .collect();
    if !same_target(target, &focus()) {
        return Err(ChordError::Unavailable);
    }
    // SAFETY: The input slice contains complete keyboard events with matching releases.
    let sent = unsafe { SendInput(&events, size_of::<INPUT>() as i32) };
    if sent as usize == events.len() {
        return Ok(());
    }
    let released: Vec<_> = keys.iter().rev().map(|key| input(*key, true)).collect();
    // SAFETY: The recovery releases only the keys this operation attempted to press.
    let _ = unsafe { SendInput(&released, size_of::<INPUT>() as i32) };
    Err(ChordError::Unavailable)
}

fn chord_keys(chord: Chord) -> &'static [VIRTUAL_KEY] {
    match chord {
        Chord::CtrlShiftV => &[VK_CONTROL, VK_SHIFT, VIRTUAL_KEY(b'V' as u16)],
        Chord::ShiftInsert => &[VK_SHIFT, VK_INSERT],
        Chord::CtrlV | Chord::CmdV => &[VK_CONTROL, VIRTUAL_KEY(b'V' as u16)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reused_window_with_a_new_process_is_not_the_target() {
        let target = Target {
            window: Some(5),
            classes: vec!["pid:10".into()],
            ..Default::default()
        };
        assert!(same_target(&target, &target));
        let mut other = target.clone();
        other.classes = vec!["pid:11".into()];
        assert!(!same_target(&target, &other));
        other = target.clone();
        other.own = true;
        assert!(!same_target(&target, &other));
        assert!(!same_target(&Target::default(), &Target::default()));
    }

    #[test]
    fn paste_chords_use_windows_virtual_keys() {
        assert_eq!(chord_keys(Chord::CtrlV), &[VK_CONTROL, VIRTUAL_KEY(0x56)]);
        assert_eq!(
            chord_keys(Chord::CtrlShiftV),
            &[VK_CONTROL, VK_SHIFT, VIRTUAL_KEY(0x56)]
        );
        assert_eq!(chord_keys(Chord::ShiftInsert), &[VK_SHIFT, VK_INSERT]);
    }
}
