# ASR Pro Portable Data

> TLDR: packaged Windows and Linux builds keep ASR Pro data beside the executable in `asrpro-data/`. macOS does the same when the app is outside a normal Applications folder.

## Intent

ASR Pro is local-first. The app should be movable without scattering private dictation state across hidden system folders, especially on Windows and Linux where users often keep tools in a custom folder.

| Goal | Result |
|---|---|
| Keep models local | Whisper model downloads stay under the app-owned data folder. |
| Keep history local | Transcript history, text exports, session state, and Chromium local storage stay under app-owned data paths. |
| Keep config local | App settings, overlay settings, launch preference, and editor preference stay in `config/`. |
| Make moves predictable | Move the executable folder with `asrpro-data/` and the app carries its useful state with it. |

## Data Folder Rules

| Platform and location | Data folder |
|---|---|
| Windows packaged app | Folder containing `ASR Pro.exe` plus `asrpro-data/` |
| Windows electron-builder portable `.exe` | Folder from `PORTABLE_EXECUTABLE_DIR` plus `asrpro-data/` |
| Linux packaged app | Folder containing the executable plus `asrpro-data/` |
| macOS in `/Applications` | Standard app support path with ASR Pro data under `data/` |
| macOS in `/System/Applications` | Standard app support path with ASR Pro data under `data/` |
| macOS in `~/Applications` | Standard app support path with ASR Pro data under `data/` |
| macOS in `/Volumes/<drive>/Applications` | Standard app support path with ASR Pro data under `data/` |
| macOS outside those folders | Folder containing `ASR Pro.app` plus `asrpro-data/` |
| Development | `tmp/app-data/` in the repo |

## What Lives Inside

| Folder | Contents |
|---|---|
| `asrpro-data/models/whisper/` | Downloaded Whisper model files. |
| `asrpro-data/transcripts/` | Text files opened from transcript history. |
| `asrpro-data/config/` | App settings, overlay settings, launch preference, and editor preference. |
| `asrpro-data/user-data/` | Electron user data, including renderer local storage used by history. |
| `asrpro-data/session/` | Chromium session data. |
| `asrpro-data/logs/` | Electron logs. |
| `asrpro-data/cache/` | Runtime cache paths exposed to native dependencies. |

## Move Checklist

| Step | Action |
|---|---|
| 1 | Quit ASR Pro from the tray or app menu. |
| 2 | Move the executable folder and the sibling `asrpro-data/` folder together. |
| 3 | Start ASR Pro from the new location. |
| 4 | If `Launch at startup` is enabled, toggle it off and on, or simply turn it on again, so the saved sign-in target is replaced with the new executable path. |

## Windows Portable Notes

Electron-builder portable `.exe` files run through an extracted runtime. ASR Pro uses electron-builder's `PORTABLE_EXECUTABLE_DIR` and `PORTABLE_EXECUTABLE_FILE` values when they are present, so data and sign-in launch target the real `.exe` the user moved, not the temporary extracted executable.

## Automation Override

Set `ASRPRO_DATA_DIR` when a build, test, or packaging smoke needs an isolated data directory.

| Variable | Behavior |
|---|---|
| `ASRPRO_DATA_DIR=/tmp/asrpro-smoke` | Uses the absolute path directly. |
| `ASRPRO_DATA_DIR=tmp/smoke-data` | Uses the path relative to the app path. |

This override is for automation and diagnostics. Normal packaged users should not need it.
