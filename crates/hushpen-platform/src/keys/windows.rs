//! Windows global keys, decoded on the hook's own message-loop thread.

#![allow(unsafe_code)]

use super::hub::Hub;
use super::{Reason, RecordSink, Sink, Unavailable};
use hushpen_core::shortcut::{Bindings, Combo, Conflict, Key, Modifier, Named, Phys, Side};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyNameTextW, GetKeyboardLayout, HOT_KEY_MODIFIERS, MAPVK_VK_TO_VSC_EX, MOD_ALT,
    MOD_CONTROL, MOD_SHIFT, MOD_WIN, MapVirtualKeyExW, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LLKHF_INJECTED,
    MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

pub(super) struct WindowsKeys {
    hub: Arc<Hub>,
    thread_id: u32,
}

struct Listener {
    hub: Arc<Hub>,
    down: HashMap<(u32, u32, bool), Phys>,
}

thread_local! {
    static LISTENER: RefCell<Option<Listener>> = const { RefCell::new(None) };
}

fn unavailable(error: impl std::fmt::Display) -> Unavailable {
    Unavailable::new(
        Reason::Failed,
        format!("The keyboard hook could not start: {error}"),
    )
}

impl WindowsKeys {
    pub(super) fn start(
        bindings: Bindings,
        sink: Sink,
        record: RecordSink,
    ) -> Result<Self, Unavailable> {
        let hub = Arc::new(Hub::new(bindings, sink, record));
        let listener = Arc::clone(&hub);
        let (ready, receive) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("hushpen-keys".into())
            .spawn(move || run(listener, ready))
            .map_err(unavailable)?;
        let thread_id = receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(unavailable)??;
        hushpen_core::shortcut::set_code_names(code_name);
        Ok(Self { hub, thread_id })
    }

    pub(super) fn set_bindings(&self, bindings: Bindings) {
        self.hub.set_bindings(bindings);
    }

    pub(super) fn start_recording(&self) {
        self.hub.start_recording();
    }

    pub(super) fn stop_recording(&self) {
        self.hub.stop_recording();
    }

    pub(super) fn set_session_active(&self, active: bool) {
        self.hub.set_escape(active);
    }

    pub(super) fn conflict(&self, combo: &Combo) -> Option<Conflict> {
        let (modifiers, vk) = hotkey(combo)?;
        static PROBE: Mutex<()> = Mutex::new(());
        let _probe = PROBE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        const ID: i32 = 0x4850;
        // SAFETY: a null window associates the temporary registration with this thread.
        match unsafe { RegisterHotKey(None, ID, modifiers, vk) } {
            Ok(()) => {
                // SAFETY: this thread owns the registration just made with this id.
                if let Err(error) = unsafe { UnregisterHotKey(None, ID) } {
                    log::warn!("the shortcut probe could not release its registration: {error}");
                }
                None
            }
            Err(error) if error.code().0 as u32 == 0x8007_0581 => Some(Conflict {
                message: "Another app or Windows uses this shortcut.".to_owned(),
                steps: "Remove the shortcut in that app, or choose different keys here.".to_owned(),
                open: None,
            }),
            Err(error) => {
                log::debug!("the shortcut probe could not check {combo:?}: {error}");
                None
            }
        }
    }
}

impl Drop for WindowsKeys {
    fn drop(&mut self) {
        // SAFETY: the hook thread creates its queue before publishing its thread id.
        if let Err(error) =
            unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
        {
            log::debug!("the keyboard hook thread already stopped: {error}");
        }
    }
}

fn run(hub: Arc<Hub>, ready: mpsc::SyncSender<Result<u32, Unavailable>>) {
    let mut message = MSG::default();
    // SAFETY: the message is writable, and PeekMessage creates this thread's message queue.
    unsafe { PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE) };
    LISTENER.with(|slot| {
        *slot.borrow_mut() = Some(Listener {
            hub,
            down: HashMap::new(),
        });
    });
    // SAFETY: the callback is a static function in the current module. A low-level hook runs
    // on this installing thread, which keeps pumping its message queue below.
    let hook = unsafe {
        GetModuleHandleW(None).and_then(|module| {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), Some(module.into()), 0)
        })
    };
    let hook = match hook {
        Ok(hook) => hook,
        Err(error) => {
            let _ = ready.send(Err(unavailable(error)));
            LISTENER.with(|slot| *slot.borrow_mut() = None);
            return;
        }
    };
    // SAFETY: this returns the identifier of the thread that owns the hook and message queue.
    if ready.send(Ok(unsafe { GetCurrentThreadId() })).is_ok() {
        loop {
            // SAFETY: message is writable and this call receives only this thread's queue.
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
            if result <= 0 {
                if result < 0 {
                    log::warn!("the keyboard hook message loop failed");
                }
                break;
            }
            // SAFETY: GetMessage filled this message and these calls dispatch it on its owner.
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    // SAFETY: the hook was installed by this thread and has not been released.
    if let Err(error) = unsafe { UnhookWindowsHookEx(hook) } {
        log::warn!("the keyboard hook could not stop: {error}");
    }
    LISTENER.with(|slot| {
        if let Some(listener) = slot.borrow_mut().take() {
            for (_, key) in listener.down {
                listener.hub.release(key);
            }
        }
    });
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        if matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
            // SAFETY: Windows supplies a KBDLLHOOKSTRUCT for a nonnegative keyboard hook code.
            let event = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if !event.flags.contains(LLKHF_INJECTED) {
                let swallowed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LISTENER.with(|slot| {
                        let mut listener = slot.borrow_mut();
                        let Some(listener) = listener.as_mut() else {
                            return false;
                        };
                        let extended = event.flags.contains(LLKHF_EXTENDED);
                        let id = (event.vkCode, event.scanCode, extended);
                        if matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN) {
                            let key = *listener.down.entry(id).or_insert_with(|| {
                                decode(
                                    event.vkCode,
                                    event.scanCode,
                                    extended,
                                    crate::keyboard_layout::character(event.vkCode),
                                )
                            });
                            listener.hub.press(key)
                        } else {
                            listener
                                .down
                                .remove(&id)
                                .is_some_and(|key| listener.hub.release(key))
                        }
                    })
                }));
                match swallowed {
                    Ok(true) => return LRESULT(1),
                    Ok(false) => {}
                    Err(_) => log::error!("the keyboard hook callback panicked"),
                }
            }
        }
    }
    // SAFETY: unhandled events must continue through the hook chain with their original values.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn decode(vk: u32, scan: u32, extended: bool, character: Option<char>) -> Phys {
    let modifier = match vk {
        0x10 => Some((
            Modifier::Shift,
            if scan == 0x36 {
                Side::Right
            } else {
                Side::Left
            },
        )),
        0x11 => Some((
            Modifier::Ctrl,
            if extended { Side::Right } else { Side::Left },
        )),
        0x12 => Some((
            Modifier::Alt,
            if extended { Side::Right } else { Side::Left },
        )),
        0xa0 => Some((Modifier::Shift, Side::Left)),
        0xa1 => Some((Modifier::Shift, Side::Right)),
        0xa2 => Some((Modifier::Ctrl, Side::Left)),
        0xa3 => Some((Modifier::Ctrl, Side::Right)),
        0xa4 => Some((Modifier::Alt, Side::Left)),
        0xa5 => Some((Modifier::Alt, Side::Right)),
        0x5b => Some((Modifier::Cmd, Side::Left)),
        0x5c => Some((Modifier::Cmd, Side::Right)),
        _ => None,
    };
    if let Some((modifier, side)) = modifier {
        return Phys::Modifier(modifier, side);
    }
    if vk == 0x1b {
        return Phys::Esc;
    }
    if vk == 0x0d && extended {
        return Phys::Named(Named::KeypadEnter);
    }
    if !extended && matches!(vk, 0x0c | 0x21..=0x28 | 0x2d | 0x2e) {
        let keypad = match scan {
            0x52 => Some('0'),
            0x4f => Some('1'),
            0x50 => Some('2'),
            0x51 => Some('3'),
            0x4b => Some('4'),
            0x4c => Some('5'),
            0x4d => Some('6'),
            0x47 => Some('7'),
            0x48 => Some('8'),
            0x49 => Some('9'),
            0x53 => Some('.'),
            _ => None,
        };
        if let Some(ch) = keypad {
            return Phys::Named(Named::Keypad(ch));
        }
    }
    if let Some(named) = named(vk) {
        return Phys::Named(named);
    }
    character.map(Phys::Char).unwrap_or(Phys::Code(vk))
}

fn named(vk: u32) -> Option<Named> {
    Some(match vk {
        0x08 => Named::Backspace,
        0x09 => Named::Tab,
        0x0c => Named::KeypadClear,
        0x0d => Named::Return,
        0x13 => Named::Pause,
        0x14 => Named::CapsLock,
        0x21 => Named::PageUp,
        0x22 => Named::PageDown,
        0x23 => Named::End,
        0x24 => Named::Home,
        0x25 => Named::Left,
        0x26 => Named::Up,
        0x27 => Named::Right,
        0x28 => Named::Down,
        0x2c => Named::PrintScreen,
        0x2d => Named::Insert,
        0x2e => Named::Delete,
        0x2f => Named::Help,
        0x5d => Named::Menu,
        0x60..=0x69 => Named::Keypad(char::from(b'0' + (vk - 0x60) as u8)),
        0x6a => Named::Keypad('*'),
        0x6b => Named::Keypad('+'),
        0x6c => Named::KeypadClear,
        0x6d => Named::Keypad('-'),
        0x6e => Named::Keypad('.'),
        0x6f => Named::Keypad('/'),
        0x70..=0x87 => Named::F((vk - 0x70 + 1) as u8),
        0x90 => Named::NumLock,
        0x91 => Named::ScrollLock,
        0xad => Named::Mute,
        0xae => Named::VolumeDown,
        0xaf => Named::VolumeUp,
        0xb0 => Named::NextTrack,
        0xb1 => Named::PreviousTrack,
        0xb3 => Named::PlayPause,
        _ => return None,
    })
}

fn virtual_key(key: Key) -> Option<u32> {
    match key {
        Key::Char(ch) => crate::keyboard_layout::virtual_key_for(ch),
        Key::Code(vk) if (1..=254).contains(&vk) => Some(vk),
        Key::Named(Named::Escape) => Some(0x1b),
        Key::Named(Named::KeypadEnter) => Some(0x0d),
        Key::Named(wanted) => (1..=254).find(|vk| named(*vk) == Some(wanted)),
        _ => None,
    }
}

fn hotkey(combo: &Combo) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    if combo.mains().count() != 1 {
        return None;
    }
    let mut modifiers = HOT_KEY_MODIFIERS(0);
    for key in combo.keys() {
        if let Key::Any(modifier) | Key::Right(modifier) = key {
            // RegisterHotKey cannot distinguish left and right modifiers.
            if combo.left_only(*modifier) {
                return None;
            }
            modifiers |= match modifier {
                Modifier::Ctrl => MOD_CONTROL,
                Modifier::Alt => MOD_ALT,
                Modifier::Shift => MOD_SHIFT,
                Modifier::Cmd => MOD_WIN,
            };
        }
    }
    Some((modifiers, virtual_key(combo.main()?)?))
}

fn code_name(vk: u32) -> Option<String> {
    let mut buffer = [0u16; 128];
    // SAFETY: this reads a virtual key's scan code and writes at most the supplied buffer.
    let length = unsafe {
        let scan = MapVirtualKeyExW(vk, MAPVK_VK_TO_VSC_EX, Some(GetKeyboardLayout(0)));
        let extended = u32::from(scan & 0xff00 != 0) << 24;
        GetKeyNameTextW(((scan & 0xff) << 16 | extended) as i32, &mut buffer)
    };
    let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
    String::from_utf16(buffer.get(..length)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::dictation::AppEvent;
    use hushpen_core::os::Os;
    use hushpen_core::shortcut::Recording;

    #[test]
    fn generic_modifiers_use_scan_codes_and_extended_flags() {
        for (vk, scan, extended, modifier, side) in [
            (0x10, 0x2a, false, Modifier::Shift, Side::Left),
            (0x10, 0x36, false, Modifier::Shift, Side::Right),
            (0x11, 0x1d, false, Modifier::Ctrl, Side::Left),
            (0x11, 0x1d, true, Modifier::Ctrl, Side::Right),
            (0x12, 0x38, false, Modifier::Alt, Side::Left),
            (0x12, 0x38, true, Modifier::Alt, Side::Right),
            (0x5b, 0x5b, true, Modifier::Cmd, Side::Left),
            (0x5c, 0x5c, true, Modifier::Cmd, Side::Right),
        ] {
            assert_eq!(
                decode(vk, scan, extended, None),
                Phys::Modifier(modifier, side)
            );
        }
    }

    #[test]
    fn keypad_and_navigation_keys_remain_distinct_with_num_lock_off() {
        assert_eq!(
            decode(0x0d, 0x1c, true, None),
            Phys::Named(Named::KeypadEnter)
        );
        assert_eq!(decode(0x0d, 0x1c, false, None), Phys::Named(Named::Return));
        assert_eq!(
            decode(0x23, 0x4f, false, None),
            Phys::Named(Named::Keypad('1'))
        );
        assert_eq!(decode(0x23, 0x4f, true, None), Phys::Named(Named::End));
        assert_eq!(
            decode(0x61, 0x4f, false, None),
            Phys::Named(Named::Keypad('1'))
        );
        assert_eq!(decode(0x41, 0x1e, false, Some('a')), Phys::Char('a'));
        assert_eq!(decode(0xe2, 0x56, false, None), Phys::Code(0xe2));
        assert_eq!(decode(0x1b, 1, false, None), Phys::Esc);
    }

    #[test]
    fn function_media_and_editing_virtual_keys_have_names() {
        for n in 1..=24 {
            assert_eq!(
                decode(0x6f + n, 0, false, None),
                Phys::Named(Named::F(n as u8))
            );
        }
        assert_eq!(decode(0xb3, 0, false, None), Phys::Named(Named::PlayPause));
        assert_eq!(decode(0xaf, 0, false, None), Phys::Named(Named::VolumeUp));
        assert_eq!(decode(0x2e, 0x53, true, None), Phys::Named(Named::Delete));
    }

    #[test]
    fn the_decoded_hold_and_recorder_use_the_shared_engine() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let records = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let record = Arc::clone(&records);
        let hub = Hub::new(
            Bindings {
                hold: Some(Combo::parse("RightAlt", Os::Windows).unwrap()),
                ..Bindings::default()
            },
            Arc::new(move |event| sink.lock().unwrap().push(event)),
            Arc::new(move |event| record.lock().unwrap().push(event)),
        );
        let key = decode(0x12, 0x38, true, None);
        hub.press(key);
        hub.release(key);
        assert_eq!(
            *events.lock().unwrap(),
            [AppEvent::HoldDown, AppEvent::HoldUp]
        );
        hub.start_recording();
        hub.press(key);
        hub.release(key);
        assert_eq!(
            records.lock().unwrap().last(),
            Some(&Recording::Captured(vec![Key::Right(Modifier::Alt)]))
        );
    }

    #[test]
    fn hotkey_probes_only_represent_chords_with_one_main_key() {
        assert_eq!(
            hotkey(&Combo::parse("Ctrl+Alt+F5", Os::Windows).unwrap()),
            Some((MOD_CONTROL | MOD_ALT, 0x74))
        );
        assert_eq!(
            hotkey(&Combo::parse("RightAlt", Os::Windows).unwrap()),
            None
        );
        assert_eq!(
            hotkey(&Combo::parse("Ctrl+RightCtrl+F5", Os::Windows).unwrap()),
            None
        );
        assert_eq!(hotkey(&Combo::parse("F5+F6", Os::Windows).unwrap()), None);
    }
}
