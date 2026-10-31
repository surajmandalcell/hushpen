# Hushpen

<p align="center">
  <img src="./assets/hushpen-app-icon.png" alt="Hushpen app icon" width="128" height="128" />
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
  on Linux by default). Tap, double-tap, and Esc to cancel. Any key or chord
  the system reports can be a shortcut, media keys included; a shortcut that
  another app uses shows how to free it. Each shortcut has a switch to turn
  it off and a button to reset it.
- Local transcription with Whisper models from Tiny to Large v3, with
  English-only and compact variants. Metal on macOS, CPU on Linux.
- Models view in three sections: speech recognition, text cleanup, and
  voices. It marks the recommended model of each kind, and a details sheet
  shows what a model is for, its languages, speed, accuracy, memory, license,
  source page, folder, and checksum. Models download only when you ask,
  after a free-disk check, and Hushpen checks the hash of every file.
- Paste into the focused app. Hushpen saves your clipboard and restores it,
  and never types into a secure field.
- Rule-based cleanup, on by default: fillers, repeats, spoken punctuation,
  "scratch that", and "X, no wait, Y".
- Optional AI cleanup with a local model (S1-mini by Superwhisper for
  English, or Qwen3.5 2B), or with an OpenAI-compatible endpoint such as
  Ollama or LM Studio. Off until you turn it on.
- Command Mode: hold a key, speak an instruction, and the model edits the
  selected text. It needs Qwen3.5 2B or the endpoint.
- Personal dictionary of words and replacements.
- History with search, copy, reprocess, delete with undo, and export to TXT,
  SRT, VTT, and JSON.
- Manual: drop audio files anywhere on the window to transcribe or translate
  them in a queue (WAV, MP3, FLAC, Ogg Vorbis, M4A, and AIFF, up to 12 hours
  each), with the time left, Copy, Export, Show in History, and Retry.
- Read aloud: typed text or a `.txt` or `.md` file, spoken by the system
  voice or by Kokoro, with Play and Export Audio to WAV. A shortcut reads the
  text selected in any app.
- Settings in one list with a search field (Cmd+F on macOS, Ctrl+F on
  Linux).
- Flow bar that shows the dictation state without taking focus, a tray menu,
  and a fixed 940x600 window that follows the system Light or Dark setting.
- Opt-in update check with signed updates. No telemetry.

Platforms: macOS 13 or later on Apple silicon, and Linux x64 and arm64 on X11.

## Install

Download a file from the [releases page](https://github.com/surajmandalcell/hushpen/releases).
Each name follows `Hushpen-<version>-<target>.<ext>`.

### macOS (Apple silicon)

1. Download `Hushpen-2.2.0-macos-arm64.dmg` and drag Hushpen to Applications,
   or unzip `Hushpen-2.2.0-macos-arm64.zip` and move `Hushpen.app` to
   Applications.
2. Open Hushpen. The app is signed with a Developer ID certificate and
   notarized by Apple, so macOS opens it after one standard confirmation.

### Linux

Replace `x64` with `arm64` on an Arm machine.

- AppImage: `chmod +x Hushpen-2.2.0-linux-x64.AppImage`, then run it.
- Debian and Ubuntu: `sudo apt install ./Hushpen-2.2.0-linux-x64.deb`
- Fedora and openSUSE: `sudo dnf install ./Hushpen-2.2.0-linux-x64.rpm`
- tar.gz: `tar xzf Hushpen-2.2.0-linux-x64.tar.gz`, then run `hushpen` from
  the new folder.

Every package holds three programs: `hushpen`, the `hushpen-llm` helper that
runs the local AI cleanup model, and the `hushpen-tts` helper that runs the
Kokoro voice for Read aloud.

### Linux requirements

- An X11 session. On Wayland the global key and paste show "not available".
- A Vulkan or OpenGL driver. On Debian and Ubuntu, `mesa-vulkan-drivers` or
  `libgl1-mesa-dri` is enough. GPUI draws the window with it. A machine with
  no driver cannot open the window.
- A system tray host if you want the tray icon (StatusNotifier, which most
  desktops include).

## macOS permissions

Hushpen asks for three permissions in its first-run setup. Each row has
Allow and Open Settings buttons, and the status updates live.

| Permission       | Used for                               |
| ---------------- | -------------------------------------- |
| Microphone       | Recording your voice                   |
| Input Monitoring | Seeing the hold key while you use other apps |
| Accessibility    | Pasting the text into the focused app  |

Hushpen adds itself to the matching lists in System Settings when it starts,
and the stable signature keeps your grants across updates and reinstalls.
On macOS 27, the Accessibility switch also covers Input Monitoring. macOS
forgets the grants only when the signing certificate changes, or when you
remove them yourself. Hushpen notices at start and opens the setup step
again. To grant them again:

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

Clone and build the three programs. The whisper.cpp source is vendored, so
there are no submodules:

```sh
git clone https://github.com/surajmandalcell/hushpen.git
cd hushpen
cargo build --release --locked -p hushpen-app -p hushpen-llm -p hushpen-tts
cp assets/ggml-silero-v5.1.2.bin "${CARGO_TARGET_DIR:-target}/release/"
```

The programs are `target/release/hushpen`, `target/release/hushpen-llm`, and
`target/release/hushpen-tts`. Keep them in the same folder; `hushpen` starts
the two workers from there. Keep the small Silero speech detector beside
them too; it skips silence without a download. The first build compiles
whisper.cpp and llama.cpp and takes several minutes.

Read aloud with the Kokoro voice also needs the sherpa-onnx libraries next to
`hushpen-tts` (in `Contents/Frameworks` of an `.app`). Fetch the pinned,
checksummed copies with `cargo xtask tts-runtime --dir target/release`.

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
