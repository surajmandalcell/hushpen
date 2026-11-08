#![allow(unsafe_code)]

use super::Restore;
use std::io;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};
use windows::core::PCWSTR;

struct Apartment;

impl Apartment {
    fn new() -> io::Result<Self> {
        // SAFETY: the cue worker uses MTA and each successful initialization is balanced.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(io::Error::other)?;
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: this thread owns one successful CoInitializeEx call.
        unsafe { CoUninitialize() };
    }
}

fn enumerator() -> io::Result<IMMDeviceEnumerator> {
    // SAFETY: this class implements IMMDeviceEnumerator and COM is initialized by the caller.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(io::Error::other)
}

pub(super) fn snapshot() -> io::Result<Option<Restore>> {
    let _apartment = Apartment::new()?;
    let enumerator = enumerator()?;
    // SAFETY: COM interfaces are live on this initialized thread; GetId returns task memory.
    unsafe {
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(io::Error::other)?;
        let volume: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(io::Error::other)?;
        if volume.GetMute().map_err(io::Error::other)?.as_bool() {
            return Ok(None);
        }
        let id = device.GetId().map_err(io::Error::other)?;
        let name = id.to_string();
        CoTaskMemFree(Some(id.0.cast()));
        Ok(Some(Restore {
            kind: "windows-mute".into(),
            device: name.map_err(io::Error::other)?,
            channels: vec![(0, 0.0)],
        }))
    }
}

pub(super) fn apply(saved: &Restore, restore: bool) -> io::Result<()> {
    if saved.kind != "windows-mute" {
        return Err(io::Error::other(
            "the mute journal belongs to another backend",
        ));
    }
    let _apartment = Apartment::new()?;
    let name: Vec<u16> = saved.device.encode_utf16().chain(Some(0)).collect();
    // SAFETY: the terminated endpoint id is live, interfaces stay in this apartment, and null is an allowed context.
    unsafe {
        let device = enumerator()?
            .GetDevice(PCWSTR(name.as_ptr()))
            .map_err(io::Error::other)?;
        let volume: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(io::Error::other)?;
        volume
            .SetMute(!restore, std::ptr::null())
            .map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_different_backend_journal_is_refused_before_com() {
        let saved = Restore {
            kind: "pactl".into(),
            device: "default".into(),
            channels: vec![(0, 0.0)],
        };
        assert!(apply(&saved, true).is_err());
    }
}
