# Legacy Tauri Surface

Electron is the supported ASR Pro desktop runtime.

| Path | Status |
|---|---|
| `src-tauri/icons/` | Still used by Electron packaging for app and tray icons. |
| `src-tauri/src/`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` | Legacy Tauri code. Not part of the supported Windows, Linux, or macOS release path. |

Do not treat this directory as an active runtime until Tauri support is explicitly revived and `cargo check` passes.
