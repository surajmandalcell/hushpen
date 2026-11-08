#![allow(unsafe_code)]

use super::{ChromeError, Frame, Layout, Pointer, ScreenRect, SizeLimits};
use std::mem::size_of;
use std::sync::OnceLock;
use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::Foundation::{
    GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SetLastError, WIN32_ERROR, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::Shell::{
    DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

const CHROME: usize = 1;
const OVERLAY: usize = 2;

fn fail(what: &str, error: impl std::fmt::Display) -> ChromeError {
    ChromeError::new(format!("{what}: {error}"))
}

fn hwnd(window: usize) -> Result<HWND, ChromeError> {
    let handle = HWND(window as *mut _);
    // SAFETY: IsWindow accepts an arbitrary handle and checks it without dereferencing it here.
    if window != 0 && unsafe { IsWindow(Some(handle)).as_bool() } {
        Ok(handle)
    } else {
        Err(ChromeError::new("the Windows window is no longer open"))
    }
}

fn track_sizes(limits: SizeLimits, scale: f32, border: (i32, i32)) -> (POINT, POINT) {
    let pixels = |value| (f32::from(value) * scale).round() as i32;
    let width = pixels(limits.width) + border.0;
    (
        POINT {
            x: width,
            y: pixels(limits.min_height) + border.1,
        },
        POINT {
            x: width,
            y: pixels(limits.max_height) + border.1,
        },
    )
}

pub(super) fn lock(window: usize, limits: SizeLimits) -> Result<(), ChromeError> {
    let handle = hwnd(window)?;
    // SAFETY: The handle belongs to this UI thread and its current style can be read.
    let style = unsafe { GetWindowLongPtrW(handle, GWL_STYLE) };
    set_style(handle, GWL_STYLE, style & !(WS_MAXIMIZEBOX.0 as isize))?;
    // SAFETY: The handle belongs to this UI thread. The style remains a valid style bitmask.
    unsafe {
        let mut old = 0;
        if GetWindowSubclass(handle, Some(subclass), CHROME, Some(&mut old)).as_bool() {
            *(old as *mut SizeLimits) = limits;
        } else {
            let data = Box::into_raw(Box::new(limits));
            if !SetWindowSubclass(handle, Some(subclass), CHROME, data as usize).as_bool() {
                drop(Box::from_raw(data));
                return Err(fail(
                    "limit the window size",
                    std::io::Error::last_os_error(),
                ));
            }
        }
        SetWindowPos(
            handle,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )
        .map_err(|error| fail("apply the window style", error))?;
    }
    dark_title_bar(handle);
    Ok(())
}

unsafe extern "system" fn subclass(
    handle: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    if id == OVERLAY && message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    if id == CHROME && message == WM_SYSCOMMAND && wparam.0 & 0xfff0 == SC_MAXIMIZE as usize {
        return LRESULT(0);
    }
    if message == WM_NCDESTROY {
        // SAFETY: The subclass is removed once; only CHROME owns the Box stored in its reference data.
        unsafe {
            let _ = RemoveWindowSubclass(handle, Some(subclass), id);
            if id == CHROME {
                drop(Box::from_raw(data as *mut SizeLimits));
            }
        }
    }
    // SAFETY: Windows supplies this callback's message parameters; forward them to the next handler.
    let result = unsafe { DefSubclassProc(handle, message, wparam, lparam) };
    if id == CHROME && message == WM_GETMINMAXINFO && lparam.0 != 0 {
        let mut outer = RECT::default();
        let mut client = RECT::default();
        // SAFETY: The live window owns the limits; WM_GETMINMAXINFO supplies a writable MINMAXINFO.
        unsafe {
            if GetWindowRect(handle, &mut outer).is_ok()
                && GetClientRect(handle, &mut client).is_ok()
            {
                let border = (
                    (outer.right - outer.left) - (client.right - client.left),
                    (outer.bottom - outer.top) - (client.bottom - client.top),
                );
                let (min, max) = track_sizes(*(data as *const SizeLimits), scale(handle), border);
                let info = &mut *(lparam.0 as *mut MINMAXINFO);
                info.ptMinTrackSize = min;
                info.ptMaxTrackSize = max;
                info.ptMaxSize = max;
            }
        }
    }
    if id == CHROME && matches!(message, WM_SETTINGCHANGE | WM_THEMECHANGED) {
        dark_title_bar(handle);
    }
    result
}

fn set_style(handle: HWND, index: WINDOW_LONG_PTR_INDEX, value: isize) -> Result<(), ChromeError> {
    // SAFETY: The caller supplies a live window and a style index, never a window-procedure address.
    unsafe {
        SetLastError(WIN32_ERROR(0));
        if SetWindowLongPtrW(handle, index, value) == 0 {
            let error = GetLastError();
            if error.0 != 0 {
                return Err(fail(
                    "set the window style",
                    std::io::Error::from_raw_os_error(error.0 as i32),
                ));
            }
        }
    }
    Ok(())
}

fn supports_mica(build: u32) -> bool {
    build >= 22621
}

pub(super) fn mica_supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        // SAFETY: The structure has the documented size and stays writable for the call.
        unsafe { RtlGetVersion(&mut version).is_ok() && supports_mica(version.dwBuildNumber) }
    })
}

fn dark_title_bar(handle: HWND) {
    let mut light: u32 = 1;
    let mut bytes = size_of::<u32>() as u32;
    // SAFETY: The registry reads a DWORD into a sized buffer; DWM reads one i32 synchronously.
    unsafe {
        let _ = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut light as *mut u32).cast()),
            Some(&mut bytes),
        );
        let dark = i32::from(light == 0);
        if let Err(error) = DwmSetWindowAttribute(
            handle,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            size_of::<i32>() as u32,
        ) {
            log::debug!("the title bar appearance could not be set: {error}");
        }
    }
}

pub(super) fn add_sidebar_material(window: usize) -> Result<(), ChromeError> {
    let handle = hwnd(window)?;
    dark_title_bar(handle);
    if !mica_supported() {
        return Ok(());
    }
    let backdrop = DWMSBT_MAINWINDOW.0;
    // SAFETY: DWM reads the backdrop value synchronously for the live window.
    unsafe {
        DwmSetWindowAttribute(
            handle,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&backdrop as *const i32).cast(),
            size_of::<i32>() as u32,
        )
    }
    .map_err(|error| fail("set the sidebar material", error))
}

fn show_as(window: usize, command: SHOW_WINDOW_CMD) -> Result<(), ChromeError> {
    let handle = hwnd(window)?;
    // SAFETY: The handle is live; these show commands do not activate the window.
    let _ = unsafe { ShowWindow(handle, command) };
    Ok(())
}

pub(super) fn hide(window: usize) -> Result<(), ChromeError> {
    show_as(window, SW_HIDE)
}

pub(super) fn show(window: usize) -> Result<(), ChromeError> {
    show_as(window, SW_SHOWNOACTIVATE)
}

pub(super) fn minimize(window: usize) -> Result<(), ChromeError> {
    show_as(window, SW_SHOWMINNOACTIVE)
}

fn scale(handle: HWND) -> f32 {
    // SAFETY: Reading the DPI does not change this live window.
    let dpi = unsafe { GetDpiForWindow(handle) };
    dpi.max(96) as f32 / 96.0
}

pub(super) fn place(window: usize, frame: Frame) -> Result<(), ChromeError> {
    let handle = hwnd(window)?;
    let scale = scale(handle);
    let pixels = |value: f32| (value * scale).round() as i32;
    // SAFETY: The handle is live and the frame only moves/sizes it without activation.
    unsafe {
        SetWindowPos(
            handle,
            Some(HWND_TOPMOST),
            pixels(frame.x as f32),
            pixels(frame.y as f32),
            pixels(frame.width as f32),
            pixels(frame.height as f32),
            SWP_NOACTIVATE,
        )
    }
    .map_err(|error| fail("place the flow bar", error))
}

pub(super) fn pin(window: usize) -> Result<(), ChromeError> {
    let handle = hwnd(window)?;
    // SAFETY: The handle belongs to this UI thread and its current extended style can be read.
    let style = unsafe { GetWindowLongPtrW(handle, GWL_EXSTYLE) } as u32;
    set_style(
        handle,
        GWL_EXSTYLE,
        (style | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize,
    )?;
    // SAFETY: The handle belongs to this UI thread; the subclass has no allocated reference data.
    unsafe {
        if !SetWindowSubclass(handle, Some(subclass), OVERLAY, 0).as_bool() {
            return Err(fail(
                "keep the flow bar unfocused",
                std::io::Error::last_os_error(),
            ));
        }
        SetWindowPos(
            handle,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        )
        .map_err(|error| fail("pin the flow bar", error))
    }
}

pub(super) fn pointer(window: usize) -> Result<Pointer, ChromeError> {
    let handle = hwnd(window)?;
    let mut point = POINT::default();
    // SAFETY: The point is writable; GetAsyncKeyState reads the global button state.
    let left = unsafe {
        GetCursorPos(&mut point).map_err(|error| fail("read the pointer", error))?;
        GetAsyncKeyState(i32::from(VK_LBUTTON.0)) < 0
    };
    let scale = scale(handle);
    Ok(Pointer {
        x: point.x as f32 / scale,
        y: point.y as f32 / scale,
        left,
    })
}

pub(super) fn screens() -> Result<Layout, ChromeError> {
    let mut point = POINT::default();
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let mut dpi_x = 96;
    let mut dpi_y = 96;
    // SAFETY: All output buffers are sized and writable; the monitor comes from the current pointer.
    unsafe {
        GetCursorPos(&mut point).map_err(|error| fail("read the pointer", error))?;
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err(fail("read the monitor", std::io::Error::last_os_error()));
        }
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
            .map_err(|error| fail("read the monitor scale", error))?;
    }
    let scale = dpi_x.max(96) as f32 / 96.0;
    let rect = info.rcMonitor;
    let screen = ScreenRect {
        x: rect.left as f32 / scale,
        y: rect.top as f32 / scale,
        width: (rect.right - rect.left) as f32 / scale,
        height: (rect.bottom - rect.top) as f32 / scale,
    };
    Ok(Layout {
        screens: vec![screen],
        pointer: (point.x as f32 / scale, point.y as f32 / scale),
        desktop_width: screen.width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_follow_dpi_and_keep_the_width_fixed() {
        let limits = SizeLimits {
            width: 940,
            min_height: 600,
            max_height: 900,
        };
        for (scale, border, width, min_height, max_height) in [
            (1.0, (0, 0), 940, 600, 900),
            (1.5, (16, 8), 1426, 908, 1358),
            (2.0, (16, 8), 1896, 1208, 1808),
        ] {
            let (min, max) = track_sizes(limits, scale, border);
            assert_eq!(
                (min.x, max.x, min.y, max.y),
                (width, width, min_height, max_height)
            );
        }
    }

    #[test]
    fn mica_starts_at_windows_11_22h2() {
        assert!(!supports_mica(19045));
        assert!(!supports_mica(22000));
        assert!(!supports_mica(22620));
        assert!(supports_mica(22621));
        assert!(supports_mica(26100));
    }
}
