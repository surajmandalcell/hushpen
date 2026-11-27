//! The Settings guide drawn offscreen: no window reaches the screen and no app takes focus.
//! AppKit views need the main thread, so this target has its own `main`. With
//! `GUIDE_SNAPSHOT_DIR` set it also writes the 2x renders for the look check.

#[cfg(target_os = "macos")]
fn main() {
    use hushpen_core::permission::Permission;
    use hushpen_platform::settings_guide::snapshot_png;
    use std::path::PathBuf;

    let app = std::env::var_os("GUIDE_APP")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Applications/Hushpen.app"));
    let out = std::env::var_os("GUIDE_SNAPSHOT_DIR").map(PathBuf::from);
    // The panel width under a 702 pt Settings window, as in the reference shots.
    let width = 472.0;
    for (pane, name) in [
        (Permission::Accessibility, "accessibility"),
        (Permission::InputMonitoring, "input-monitoring"),
        (Permission::Microphone, "microphone"),
    ] {
        for dark in [false, true] {
            let png = snapshot_png(pane, dark, width, &app).expect("the guide renders");
            // A PNG header, then the IHDR width and height at 2x.
            assert_eq!(&png[1..4], b"PNG");
            let wide = u32::from_be_bytes(png[16..20].try_into().unwrap());
            let high = u32::from_be_bytes(png[20..24].try_into().unwrap());
            assert_eq!(wide, 944, "{name}");
            assert!((150..=260).contains(&high), "{name} is {high} px high");
            if let Some(dir) = &out {
                let theme = if dark { "dark" } else { "light" };
                std::fs::write(dir.join(format!("{name}-{theme}.png")), &png).unwrap();
            }
        }
    }
    println!("settings guide: 6 renders ok");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
