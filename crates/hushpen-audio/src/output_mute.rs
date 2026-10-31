//! Output-only mute with a write-ahead recovery marker. All calls run on the cue thread.

use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MARKER: &str = "output-mute.json";

/// Whether this platform has a way to control the default output.
pub fn available() -> bool {
    #[cfg(target_os = "macos")]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        linux::has_tool("wpctl") || linux::has_tool("pactl")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Restore {
    kind: String,
    device: String,
    channels: Vec<(u32, f32)>,
}

impl Restore {
    fn encode(&self) -> Value {
        json!({"kind": self.kind, "device": self.device, "channels": self.channels})
    }

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid output mute marker");
        let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        let kind = value["kind"].as_str().ok_or_else(invalid)?;
        if !["mac-mute", "mac-volume", "wpctl", "pactl"].contains(&kind) {
            return Err(invalid());
        }
        let device = value["device"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(invalid)?;
        let channels = value["channels"]
            .as_array()
            .ok_or_else(invalid)?
            .iter()
            .map(|pair| {
                let channel =
                    u32::try_from(pair[0].as_u64().ok_or_else(invalid)?).map_err(|_| invalid())?;
                let volume = pair[1]
                    .as_f64()
                    .filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
                    .ok_or_else(invalid)?;
                Ok((channel, volume as f32))
            })
            .collect::<io::Result<Vec<_>>>()?;
        if channels.is_empty() || channels.len() > 64 {
            return Err(invalid());
        }
        Ok(Self {
            kind: kind.into(),
            device: device.into(),
            channels,
        })
    }
}

pub(crate) trait Backend {
    /// `None` when the user already silenced the device.
    fn snapshot(&mut self) -> io::Result<Option<Restore>>;
    fn apply(&mut self, saved: &Restore, restore: bool) -> io::Result<()>;
}

pub(crate) struct System;

impl Backend for System {
    fn snapshot(&mut self) -> io::Result<Option<Restore>> {
        #[cfg(target_os = "macos")]
        {
            macos::snapshot()
        }
        #[cfg(target_os = "linux")]
        {
            linux::snapshot()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Err(io::Error::other("output mute is not available"))
        }
    }

    fn apply(&mut self, saved: &Restore, restore: bool) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            macos::apply(saved, restore)
        }
        #[cfg(target_os = "linux")]
        {
            linux::apply(saved, restore)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (saved, restore);
            Err(io::Error::other("output mute is not available"))
        }
    }
}

pub(crate) struct OutputMute<B: Backend = System> {
    backend: B,
    marker: PathBuf,
    saved: Option<Restore>,
}

impl<B: Backend> OutputMute<B> {
    pub(crate) fn new(root: &Path, backend: B) -> Self {
        let mut me = Self {
            backend,
            marker: root.join(MARKER),
            saved: None,
        };
        if let Err(error) = me.recover() {
            log::warn!("could not restore the output after an interrupted dictation: {error}");
        }
        me
    }

    fn recover(&mut self) -> io::Result<()> {
        match fs::read(&self.marker) {
            Ok(bytes) => {
                self.saved = Some(Restore::decode(&bytes)?);
                self.restore()
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn mute(&mut self, enabled: bool) -> io::Result<()> {
        if !enabled || self.saved.is_some() {
            return Ok(());
        }
        let Some(saved) = self.backend.snapshot()? else {
            return Ok(());
        };
        // Never change the output unless a crash can recover the exact device and volume.
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut marker = options.open(&self.marker)?;
        let bytes = serde_json::to_vec(&saved.encode())?;
        if let Err(error) = marker.write_all(&bytes).and_then(|()| marker.sync_all()) {
            let _ = fs::remove_file(&self.marker);
            return Err(error);
        }
        self.saved = Some(saved.clone());
        self.backend.apply(&saved, false)
    }

    pub(crate) fn restore(&mut self) -> io::Result<()> {
        let Some(saved) = &self.saved else {
            return Ok(());
        };
        self.backend.apply(saved, true)?;
        match fs::remove_file(&self.marker) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.saved = None;
        Ok(())
    }
}

impl<B: Backend> Drop for OutputMute<B> {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            log::warn!("could not restore the system output: {error}");
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos {
    use super::*;
    use std::ffi::c_void;

    #[repr(C)]
    struct Address {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyData(
            object: u32,
            address: *const Address,
            qualifier_size: u32,
            qualifier: *const c_void,
            size: *mut u32,
            data: *mut c_void,
        ) -> i32;
        fn AudioObjectIsPropertySettable(
            object: u32,
            address: *const Address,
            settable: *mut u8,
        ) -> i32;
        fn AudioObjectSetPropertyData(
            object: u32,
            address: *const Address,
            qualifier_size: u32,
            qualifier: *const c_void,
            size: u32,
            data: *const c_void,
        ) -> i32;
    }

    const OUTPUT: u32 = u32::from_be_bytes(*b"outp");
    const MUTE: u32 = u32::from_be_bytes(*b"mute");
    const VOLUME: u32 = u32::from_be_bytes(*b"volm");

    fn check(status: i32) -> io::Result<()> {
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!("CoreAudio status {status}")))
        }
    }

    fn read<T: Default>(device: u32, address: &Address) -> io::Result<T> {
        let mut value = T::default();
        let mut size = size_of::<T>() as u32;
        // SAFETY: CoreAudio receives a live, correctly sized scalar and property address.
        check(unsafe {
            AudioObjectGetPropertyData(
                device,
                address,
                0,
                std::ptr::null(),
                &mut size,
                (&mut value as *mut T).cast(),
            )
        })?;
        if size != size_of::<T>() as u32 {
            return Err(io::Error::other("unexpected CoreAudio property size"));
        }
        Ok(value)
    }

    fn settable(device: u32, address: &Address) -> bool {
        let mut value = 0;
        // SAFETY: The output is a CoreAudio Boolean (one byte).
        unsafe { AudioObjectIsPropertySettable(device, address, &mut value) == 0 && value != 0 }
    }

    fn write<T>(device: u32, address: &Address, value: T) -> io::Result<()> {
        // SAFETY: The property's scalar type, size, and lifetime match this call.
        check(unsafe {
            AudioObjectSetPropertyData(
                device,
                address,
                0,
                std::ptr::null(),
                size_of::<T>() as u32,
                (&value as *const T).cast(),
            )
        })
    }

    pub(super) fn snapshot() -> io::Result<Option<Restore>> {
        let device: u32 = read(
            1,
            &Address {
                selector: u32::from_be_bytes(*b"dOut"),
                scope: u32::from_be_bytes(*b"glob"),
                element: 0,
            },
        )?;
        if device == 0 {
            return Err(io::Error::other("no default output device"));
        }
        let mute = Address {
            selector: MUTE,
            scope: OUTPUT,
            element: 0,
        };
        if read::<u32>(device, &mute).is_ok_and(|value| value != 0) {
            return Ok(None);
        }
        let (kind, channels) = if settable(device, &mute) {
            // Do not mute if the original state cannot be read.
            let _ = read::<u32>(device, &mute)?;
            ("mac-mute", vec![(0, 0.0)])
        } else {
            let main = Address {
                selector: VOLUME,
                scope: OUTPUT,
                element: 0,
            };
            let channels = if settable(device, &main) {
                vec![(0, read::<f32>(device, &main)?)]
            } else {
                let mut channels = Vec::new();
                for element in 1..=64 {
                    let address = Address {
                        selector: VOLUME,
                        scope: OUTPUT,
                        element,
                    };
                    if !settable(device, &address) {
                        break;
                    }
                    channels.push((element, read::<f32>(device, &address)?));
                }
                channels
            };
            if channels.is_empty() {
                return Err(io::Error::other(
                    "this output has no settable mute or volume",
                ));
            }
            if channels.iter().all(|(_, value)| *value == 0.0) {
                return Ok(None);
            }
            ("mac-volume", channels)
        };
        Ok(Some(Restore {
            kind: kind.into(),
            device: device.to_string(),
            channels,
        }))
    }

    pub(super) fn apply(saved: &Restore, restore: bool) -> io::Result<()> {
        let device = saved.device.parse().map_err(io::Error::other)?;
        // Try every channel even if one was removed or rejected a write.
        let mut failure = None;
        for &(element, volume) in &saved.channels {
            let result = match saved.kind.as_str() {
                "mac-mute" => write(
                    device,
                    &Address {
                        selector: MUTE,
                        scope: OUTPUT,
                        element,
                    },
                    u32::from(!restore),
                ),
                "mac-volume" => write(
                    device,
                    &Address {
                        selector: VOLUME,
                        scope: OUTPUT,
                        element,
                    },
                    if restore { volume } else { 0.0 },
                ),
                _ => Err(io::Error::other(
                    "output mute marker belongs to another platform",
                )),
            };
            if let Err(error) = result {
                failure = Some(error);
            }
        }
        failure.map_or(Ok(()), Err)
    }

    #[cfg(test)]
    #[test]
    fn the_real_output_getter_never_changes_any_property() {
        // This may be unavailable on a headless Mac. No setter is called.
        let _ = snapshot();
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    pub(super) fn has_tool(name: &str) -> bool {
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| {
                use std::os::unix::fs::PermissionsExt as _;
                fs::metadata(dir.join(name))
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
    }

    fn run(program: &str, args: &[&str]) -> io::Result<String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait()? {
                let output = child.wait_with_output()?;
                return if status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
                } else {
                    Err(io::Error::other(format!(
                        "{program} could not control the output"
                    )))
                };
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "output control timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wpctl() -> io::Result<Option<Restore>> {
        let inspect = run("wpctl", &["inspect", "@DEFAULT_AUDIO_SINK@"])?;
        let device = inspect
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("id ")
                    .and_then(|s| s.split(',').next())
                    .filter(|s| s.parse::<u32>().is_ok())
            })
            .ok_or_else(|| io::Error::other("wpctl did not return the default sink id"))?
            .to_owned();
        let state = run("wpctl", &["get-volume", &device])?;
        if state.contains("[MUTED]") {
            return Ok(None);
        }
        if !state.starts_with("Volume:") {
            return Err(io::Error::other("wpctl did not return the output state"));
        }
        Ok(Some(Restore {
            kind: "wpctl".into(),
            device,
            channels: vec![(0, 0.0)],
        }))
    }

    fn pactl() -> io::Result<Option<Restore>> {
        let device = run("pactl", &["get-default-sink"])?;
        if device.is_empty() || device.starts_with('-') {
            return Err(io::Error::other("no default output sink"));
        }
        let state = run("pactl", &["get-sink-mute", &device])?;
        match state.as_str() {
            "Mute: yes" => Ok(None),
            "Mute: no" => Ok(Some(Restore {
                kind: "pactl".into(),
                device,
                channels: vec![(0, 0.0)],
            })),
            _ => Err(io::Error::other("pactl did not return the output state")),
        }
    }

    pub(super) fn snapshot() -> io::Result<Option<Restore>> {
        if has_tool("wpctl")
            && let Ok(state) = wpctl()
        {
            return Ok(state);
        }
        pactl()
    }

    pub(super) fn apply(saved: &Restore, restore: bool) -> io::Result<()> {
        let muted = if restore { "0" } else { "1" };
        match saved.kind.as_str() {
            "wpctl" => run("wpctl", &["set-mute", &saved.device, muted]),
            "pactl" => run("pactl", &["set-sink-mute", &saved.device, muted]),
            _ => Err(io::Error::other(
                "output mute marker belongs to another platform",
            )),
        }
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct State {
        muted: bool,
        fail_restore: bool,
        changes: Vec<bool>,
    }
    struct Fake(Arc<Mutex<State>>);
    impl Backend for Fake {
        fn snapshot(&mut self) -> io::Result<Option<Restore>> {
            Ok((!self.0.lock().unwrap().muted).then(|| Restore {
                kind: "mac-mute".into(),
                device: "42".into(),
                channels: vec![(0, 0.0)],
            }))
        }
        fn apply(&mut self, _: &Restore, restore: bool) -> io::Result<()> {
            let mut state = self.0.lock().unwrap();
            if restore && state.fail_restore {
                return Err(io::Error::other("device unavailable"));
            }
            state.muted = !restore;
            state.changes.push(!restore);
            Ok(())
        }
    }

    #[test]
    fn restores_only_an_output_we_muted() {
        for initially_muted in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let state = Arc::new(Mutex::new(State {
                muted: initially_muted,
                ..State::default()
            }));
            let mut mute = OutputMute::new(root.path(), Fake(state.clone()));
            mute.mute(true).unwrap();
            assert!(state.lock().unwrap().muted);
            assert_eq!(mute.marker.exists(), !initially_muted);
            mute.restore().unwrap();
            let state = state.lock().unwrap();
            assert_eq!(state.muted, initially_muted);
            assert_eq!(
                state.changes,
                if initially_muted {
                    vec![]
                } else {
                    vec![true, false]
                }
            );
        }
    }

    #[test]
    fn disabled_never_touches_the_output_or_writes_a_marker() {
        let root = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let mut mute = OutputMute::new(root.path(), Fake(state.clone()));
        mute.mute(false).unwrap();
        mute.restore().unwrap();
        assert!(state.lock().unwrap().changes.is_empty());
        assert!(!mute.marker.exists());
    }

    #[test]
    fn repeated_start_does_not_replace_the_original_state() {
        let root = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let mut mute = OutputMute::new(root.path(), Fake(state.clone()));
        mute.mute(true).unwrap();
        mute.mute(true).unwrap();
        mute.restore().unwrap();
        mute.restore().unwrap();
        assert_eq!(state.lock().unwrap().changes, [true, false]);
    }

    #[test]
    fn startup_recovers_an_interrupted_mute() {
        let root = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let mut mute = OutputMute::new(root.path(), Fake(state.clone()));
        mute.mute(true).unwrap();
        // Leave the journal as a crash does, without Drop restoring the fake output.
        mute.saved = None;
        drop(mute);
        let recovered = OutputMute::new(root.path(), Fake(state.clone()));
        assert!(!state.lock().unwrap().muted);
        assert!(!recovered.marker.exists());
    }

    #[test]
    fn failed_restore_keeps_the_marker_and_retries() {
        let root = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let mut mute = OutputMute::new(root.path(), Fake(state.clone()));
        mute.mute(true).unwrap();
        state.lock().unwrap().fail_restore = true;
        assert!(mute.restore().is_err());
        assert!(mute.marker.exists());
        state.lock().unwrap().fail_restore = false;
        mute.restore().unwrap();
        assert!(!mute.marker.exists());
    }

    #[test]
    fn a_failed_journal_write_never_mutes() {
        let root = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let mut mute = OutputMute::new(&root.path().join("missing"), Fake(state.clone()));
        assert!(mute.mute(true).is_err());
        assert!(state.lock().unwrap().changes.is_empty());
    }

    #[test]
    fn a_volume_marker_keeps_every_channel_for_recovery() {
        let saved = Restore {
            kind: "mac-volume".into(),
            device: "42".into(),
            channels: vec![(1, 0.5), (2, 0.75)],
        };
        assert_eq!(
            Restore::decode(&serde_json::to_vec(&saved.encode()).unwrap()).unwrap(),
            saved
        );
        assert!(
            Restore::decode(br#"{"kind":"mac-volume","device":"42","channels":[[0,2]]}"#).is_err()
        );
    }
}
