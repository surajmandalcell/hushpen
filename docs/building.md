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
Permission-check tests supply each OS and its live facts. They cover blocking
microphone checks, macOS grants, Windows hook retry, and Linux session limits.

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
notice. A green runner proves builds and unit tests, not interactive
behavior on Windows hardware.

Onboarding and Settings show Microphone, Admin apps, and Global keys.
Microphone consent reads the current user's privacy settings, including
desktop-app access. Open Settings opens `ms-settings:privacy-microphone`.
The hook row offers Retry when global keys are unavailable. Only microphone
readiness blocks Continue on Windows.

## Windows packages

On x64 Windows, install the pinned packaging tool and build both packages:

```sh
cargo install --locked cargo-packager@0.11.8
cargo xtask package --target windows-x64
cargo xtask package-verify --target windows-x64
```

The output folder contains `Hushpen-<version>-windows-x64-setup.exe`,
`Hushpen-<version>-windows-x64.zip`, and `SHA256SUMS`. Setup uses NSIS and
installs for the current user in `%LOCALAPPDATA%\Programs\Hushpen` without
administrator rights. Extract the ZIP to use a portable copy. Both carry
the four programs, speech DLLs, speech detector, and license notices.
The build makes the ICO from the existing PNGs. The app embeds its version
and uses the GPUI manifest's PerMonitorV2 DPI mode and asInvoker privilege.

Windows packages are unsigned: there is no Authenticode certificate.
SmartScreen will warn. Update metadata still uses minisign and SHA-256.
Setup installs download the verified setup, wait for Hushpen to exit,
install silently, and relaunch. Portable copies show the release link.

The repository also provides a PowerShell installer for the latest release:

```powershell
irm https://hushpen.mandalsuraj.com/install.ps1 | iex
```

It checks SHA256SUMS before silent setup. `HUSHPEN_NO_OPEN=1` skips relaunch.
The script must be deployed separately before this website command is available.

Dispatch `ci.yml` with `targets=windows-x64` and `suite=package` to build,
check, and upload both packages. Use `suite=gate` for format, clippy,
workspace tests, dependency checks, and all four program version checks.
`release.yml` includes Windows when a release is explicitly requested.

## Linux notes

- Hushpen needs X11 for global keys and paste. On Wayland, onboarding and Settings explain this limit; microphone setup can continue.
- It needs a Vulkan or OpenGL driver to draw the window. On Debian and Ubuntu, `mesa-vulkan-drivers` or `libgl1-mesa-dri` is enough.
- The tray needs a StatusNotifier host. Without one, onboarding and Settings show an information row.

## Release test install on lin1

Before a major or minor release is complete, run `bash packaging/linux/install-on-lin1.sh` on lin1, from `/mnt/nvme1/data/hushpen/repo`. It builds on lin1, verifies the AppImage, installs the app, and keeps the installers in `~/Downloads/Hushpen`. Patch-only changes skip the build. Use `--force` for a requested patch install. The release decision in `specs/decisions.md` names the retained source and data paths.

## Mobile core

`bash mobile/build-core.sh` builds the shared phone library and generates
Swift and Kotlin bindings. It installs the four Rust targets and pinned
`cargo-ndk` when missing. Android requires NDK 28.2.13676358 under
`ANDROID_HOME`; iOS requires Xcode on the Mac. The `android`, `ios`, and
`bindings` arguments build one part. Use `check` to check the toolchains.

Outputs live under `$CARGO_TARGET_DIR/mobile`: Android libraries in
`android/jniLibs`, Swift and Kotlin in `bindings`, and the device/simulator
XCFramework in `ios/HushpenCore.xcframework`. iOS uses Metal. Android uses
CPU, with NEON on arm64. QNN and Core ML acceleration remain a later decision.

Call blocking Rust operations from a native worker thread. Both local cleanup
and served cleanup use one `SystemCleanup` over the native `SystemLlm`
callback. An unavailable, failed, or timed-out callback keeps rule-cleaned text.
Phone apps, keyboard extensions, and release packages are separate builds.

`bash mobile/tests/build-core.sh` checks the build commands without compiling.
The headless Swift smoke test is `mobile/tests/ffi.swift`. Compile it with the
generated `HushpenCore.swift`, the generated header directory, and
`-L "$CARGO_TARGET_DIR/debug" -lhushpen_mobile -lc++`. Pass a scratch folder
to the executable. It checks catalog, dictation, callbacks, and detector setup.

## More

- Design rules: `DESIGN.md`
- Project decisions and names: `specs/decisions.md` and `specs/ddd.md`
