# Build Hushpen from source

Hushpen 2.0 is written in Rust. The window uses GPUI, and the speech engine runs whisper.cpp in a child process. The whisper.cpp source is vendored, so there are no submodules.

## What you need

- Rust 1.99. The file `rust-toolchain.toml` picks it, and rustup installs it for you.
- CMake, a C and C++ compiler, and Clang with libclang.

On a Mac, run `xcode-select --install`, then `brew install cmake`.

On Debian and Ubuntu 24.04:

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

## Build

```sh
git clone https://github.com/surajmandalcell/hushpen.git
cd hushpen
cargo build --release --locked -p hushpen-app -p hushpen-engine -p hushpen-llm -p hushpen-tts
cp assets/ggml-silero-v5.1.2.bin "${CARGO_TARGET_DIR:-target}/release/"
```

The first build compiles whisper.cpp and llama.cpp, and takes several minutes.

You get four programs in `target/release`: `hushpen`, `hushpen-asr` (the speech engine), `hushpen-llm` (the local AI cleanup model), and `hushpen-tts` (the voices for Read aloud). Keep them in one folder, with the small `ggml-silero-v5.1.2.bin` speech detector beside them. `hushpen` starts the other three from there.

Read aloud with the Kokoro voice also needs the sherpa-onnx libraries next to `hushpen-tts` (in `Contents/Frameworks` of an `.app`). Fetch the pinned, checksummed copies with:

```sh
cargo xtask tts-runtime --dir target/release
```

To keep build output out of the clone, set `CARGO_TARGET_DIR` to a folder outside it:

```sh
export CARGO_TARGET_DIR="$HOME/.cache/hushpen-target"
```

## Run the tests

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Warnings count as errors. To run every check in one step (format, the two-binary check, clippy, tests, `cargo deny`, and the network audit), install `cargo-deny` (`cargo install cargo-deny --locked`), set `CARGO_TARGET_DIR` outside the clone, and run:

```sh
cargo xtask gate
```

The unit and component tests open no window and need no models. The tests that run the real speech engine, and the pixel tests, are opt-in. The Linux end-to-end tests run in the containers under `harness/`. See `harness/README.md`.

Core tests exercise macOS, Linux, and Windows policy on every host. They cover
platform defaults, model downloads and defaults, updater states, flow-bar
results, and microphone session rules without native services.

## Windows build checks

Windows builds and unit tests run on GitHub Actions with the `windows-2025`
runner. Select `targets=windows-x64` and `suite=gate` in `ci.yml`. The runner
uses MSVC, CMake, and LLVM/libclang. Native C dependencies prevent checking
this target from macOS. The four programs end in `.exe`.

The MSVC linker embeds the Common Controls 6 dependency for the native window
subclasses, including test programs. See [manifest dependencies](https://learn.microsoft.com/en-us/cpp/build/reference/manifestdependency-specify-manifest-dependencies).

Inference defaults to CPU, with `GGML_NATIVE=OFF` and an AVX2/FMA/F16C/BMI2
baseline. The `vulkan` feature in the engine and LLM crates is optional and
off by default. Windows supports Unload; it does not support Page out.

Fetch the pinned Windows speech DLLs with:

```sh
cargo xtask tts-runtime --target windows-x64 --dir target/debug
```

Keep all three DLLs beside `hushpen-tts.exe`: `sherpa-onnx-c-api.dll`,
`onnxruntime.dll`, and `onnxruntime_providers_shared.dll`. Downloads verify
the archive and each DLL by size and SHA-256.

Windows stores data in the roaming AppData known folder under `Hushpen`.
`HUSHPEN_DATA_DIR` and the saved folder choice still override it. Endpoint
keys and the single-instance pipe allow only the current user. Text for an
app with higher privileges stays on the clipboard with an administrator
notice. Windows packages are not available yet. A green runner proves a
build and unit tests, not interactive behavior on Windows hardware.

## Linux notes

- Hushpen needs an X11 session. On Wayland, the global key and the paste show "not available".
- It needs a Vulkan or OpenGL driver to draw the window. On Debian and Ubuntu, `mesa-vulkan-drivers` or `libgl1-mesa-dri` is enough.
- For the tray icon, you need a system tray host (StatusNotifier). Most desktops have one.

## Release test install on lin1

Before a major or minor release is complete, run `bash packaging/linux/install-on-lin1.sh` on lin1, from `/mnt/nvme1/data/hushpen/repo`. It builds on lin1, verifies the AppImage, installs the app, and keeps the installers in `~/Downloads/Hushpen`. Patch-only changes skip the build. Use `--force` for a requested patch install. The release decision in `specs/decisions.md` names the retained source and data paths.

## More

- Design rules: `DESIGN.md`
- Project decisions and names: `specs/decisions.md` and `specs/ddd.md`
