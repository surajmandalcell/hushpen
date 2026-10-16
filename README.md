# Hushpen

<p align="center">
  <img src="./assets/asrpro-app-icon.png" alt="Hushpen app icon" width="128" height="128" />
</p>

## What Hushpen is

Hushpen is a private, local dictation app. Hold a key and speak. Hushpen
transcribes your speech on your own computer, cleans up the text, and pastes
it into the app you are using. It is an open-source, local-first take on
tools like Wispr Flow. Your audio and text never leave the machine unless you
choose an endpoint yourself.

Hushpen 2.0 is written in Rust. The window uses GPUI, and the speech engine
runs whisper.cpp in a child process.

## Features

- Hold-to-talk dictation with a global key (Right Option on macOS, Right Alt
  on Linux by default; you can record your own). Tap, double-tap, and Esc to
  cancel.
- Local transcription with Whisper models (tiny to large-v3-turbo). Models
  download only when you ask, with a checked hash. Metal on macOS, CPU on
  Linux.
- Paste into the focused app. Hushpen saves your clipboard and restores it,
  and never types into a secure field.
- Rule-based cleanup, on by default: fillers, repeats, spoken punctuation,
  "scratch that", and "X, no wait, Y".
- Optional AI cleanup with a local model, or with an OpenAI-compatible
  endpoint such as Ollama or LM Studio. Off until you turn it on.
- Command Mode: hold a key, speak an instruction, and the model edits the
  selected text.
- Personal dictionary of words and replacements.
- History with search, re-paste, reprocess, delete with undo, and export to
  TXT, SRT, VTT, and JSON.
- Audio file import (formats that symphonia decodes) with a queue and cancel.
- Flow bar that shows the dictation state without taking focus, a tray menu,
  and a fixed 940x600 window that follows the system Light or Dark setting.
- Opt-in update check with signed updates. No telemetry.

Platforms: macOS 13 or later on Apple silicon, and Linux x64 and arm64 on X11.

## Install

Download a file from the [releases page](https://github.com/surajmandalcell/asrpro/releases).
Each name follows `Hushpen-<version>-<target>.<ext>`.

### macOS (Apple silicon)

1. Download `Hushpen-2.0.0-macos-arm64.dmg` and drag Hushpen to Applications,
   or unzip `Hushpen-2.0.0-macos-arm64.zip` and move `Hushpen.app` to
   Applications.
2. The app is ad-hoc signed, not notarized, so macOS blocks the first start.
   Control-click Hushpen, choose Open, and confirm. On macOS 15, open System
   Settings > Privacy & Security and choose Open Anyway. You can also run
   `xattr -dr com.apple.quarantine /Applications/Hushpen.app`.

### Linux

Replace `x64` with `arm64` on an Arm machine.

- AppImage: `chmod +x Hushpen-2.0.0-linux-x64.AppImage`, then run it.
- Debian and Ubuntu: `sudo apt install ./Hushpen-2.0.0-linux-x64.deb`
- Fedora and openSUSE: `sudo dnf install ./Hushpen-2.0.0-linux-x64.rpm`
- tar.gz: `tar xzf Hushpen-2.0.0-linux-x64.tar.gz`, then run `hushpen` from
  the new folder.

Every package holds two programs: `hushpen` and `hushpen-llm`, the helper that
runs the local AI cleanup model.

### Linux requirements

- An X11 session. On Wayland the global key and paste show "not available".
- A Vulkan or OpenGL driver. On Debian and Ubuntu, `mesa-vulkan-drivers` or
  `libgl1-mesa-dri` is enough. GPUI draws the window with it. A machine with
  no driver cannot open the window.
- A system tray host if you want the tray icon (StatusNotifier, which most
  desktops include).

## macOS permissions

Hushpen asks for three permissions in its first-run setup. Each row has an
Allow button, and the status updates live.

| Permission       | Used for                               |
| ---------------- | -------------------------------------- |
| Microphone       | Recording your voice                   |
| Input Monitoring | Seeing the hold key while you use other apps |
| Accessibility    | Pasting the text into the focused app  |

Because the app is signed ad hoc, macOS forgets these grants every time you
install a new build or update. Hushpen notices at start and opens the setup
step again. To grant them again:

1. Open the setup step (it opens by itself), or System Settings > Privacy &
   Security.
2. For each of the three permissions, select Hushpen and remove it with the
   minus button if it is listed, then press Allow in Hushpen.
3. If a row stays stuck, reset it in Terminal and press Allow again:

   ```sh
   tccutil reset Microphone io.github.surajmandalcell.hushpen
   tccutil reset ListenEvent io.github.surajmandalcell.hushpen
   tccutil reset Accessibility io.github.surajmandalcell.hushpen
   ```

## Build from source

You need Rust 1.99 (rustup reads `rust-toolchain.toml` and installs it), a C
and C++ toolchain, CMake, and Clang with libclang.

- macOS: `xcode-select --install`, then `brew install cmake`.
- Debian and Ubuntu 24.04:

  ```sh
  sudo apt install build-essential clang libclang-dev cmake pkg-config \
    libfontconfig-dev libfreetype-dev libwayland-dev libxkbcommon-dev \
    libxkbcommon-x11-dev libx11-dev libx11-xcb-dev libxcb1-dev libxcb-xkb-dev \
    libxi-dev libxrandr-dev libssl-dev libzstd-dev libasound2-dev \
    libvulkan-dev libdbus-1-dev libgtk-3-dev libxdo-dev \
    libayatana-appindicator3-dev libxtst-dev libxcb-xinput-dev \
    libxcb-xtest0-dev libxcb-randr0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libpulse-dev
  ```

Clone and build both programs. The whisper.cpp source is vendored, so there are
no submodules:

```sh
git clone https://github.com/surajmandalcell/asrpro.git hushpen
cd hushpen
cargo build --release --locked -p hushpen-app -p hushpen-llm
```

The programs are `target/release/hushpen` and `target/release/hushpen-llm`.
Keep them in the same folder; `hushpen` starts `hushpen-llm` from there. The
first build compiles whisper.cpp and llama.cpp and takes several minutes.

To keep build output out of the clone, set `CARGO_TARGET_DIR` to a folder
outside it, for example `export CARGO_TARGET_DIR="$HOME/.cache/hushpen-target"`.

## Test

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Warnings are errors. The gate runs every check in one step: format, the
two-binary check, clippy, tests, `cargo deny`, and the network audit. It needs
`cargo-deny` (`cargo install cargo-deny --locked`) and a `CARGO_TARGET_DIR`
outside the clone:

```sh
export CARGO_TARGET_DIR="$HOME/.cache/hushpen-target"
cargo xtask gate
```

The unit and component tests open no window and need no models. The tests
that run the real speech engine and the pixel tests are opt-in, and the Linux
end-to-end suite runs in the containers under `harness/` (see
`harness/README.md`).

Design rules live in `DESIGN.md`, and project decisions and names in
`specs/decisions.md` and `specs/ddd.md`.

## License

Hushpen is free software under the GNU Affero General Public License, version
3 or later (AGPL-3.0-or-later). See `LICENSE`. Third-party notices are in
`assets/THIRD-PARTY-NOTICES.html`, inside every package, and in the About
view.
