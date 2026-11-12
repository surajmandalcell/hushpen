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

## Linux checks and release test install

Use the closer of `ssh oci1` and `ssh oci2`. Oci2 is selected: 0.46 s versus
3.25 s. Run Linux clippy and changed-crate tests in `hushpen-val:noble` there.
The permanent checkout is `/mnt/sdb/data/hushpen/repo`; reuse
`target/linux-arm64` and `cache/{cargo,rustup,tmp}` under the same data root.

Before a major or minor release is complete, run:

```sh
ssh oci2 'cd /mnt/sdb/data/hushpen/repo && bash packaging/linux/install-on-test-host.sh'
```

The script builds and verifies the arm64 AppImage and host archive in that
container. It installs the AppImage under `install/`, the host under
`install/host`, and the user service file. SSH setup uses the host through
`~/.local/share/hushpen-host`; its data stays under `host-data`. Installers
remain under `downloads/<version>`, linked from `~/Downloads/Hushpen`.
First installation is required. Patch-only changes skip the build; use
`--force` for a requested patch install. `--skip-build` installs already built
packages from that version's download folder after verification.

Oci2 has no desktop. The installer checks package contents, checksums,
versions, and the desktop entry. It skips desktop startup, tray, global
keys, microphone, and paste checks. Run UI-only checks in the Xvfb container
with the existing harness. The native `hushpen-host` user service is the
Office server; verify it through SSH pairing and a real speech job.
Its local service drop-in sets `--name "Office server"`. SSH pairing tries
the host's Tailscale IPv4 address, then OpenSSH's configured HostName and the
offered address. It saves the first address that passes Noise authentication.
One of those addresses must accept the host's TCP port, normally 47321.
`specs/decisions.md` D-02 names the retained setup.

## Phone and host release files

The manually started `release.yml` builds these files in addition to desktop
packages. Its required tag must name the version being released.

- `Hushpen-<version>-host-linux-x64.tar.gz`
- `Hushpen-<version>-host-linux-arm64.tar.gz`
- `Hushpen-<version>-android.apk`

`cargo xtask release-manifest` includes all three in `SHA256SUMS`, and
`release-verify` requires them. The signed desktop update manifest keeps its
desktop targets only. iOS archives are not GitHub release assets.

Build a host archive on matching Linux hardware:

```sh
cargo xtask package-host --target linux-x64
```

Use `linux-arm64` on arm64. `--out <folder>` changes the destination;
`--skip-build` packages existing Release binaries. The archive includes
`hushpen-host`, `hushpen-asr`, `hushpen-llm`, the speech detector, the user
service file, and license notices. Desktop SSH setup downloads the matching
archive, checks its SHA-256, and installs it without sudo.

Android release CI builds both arm64-v8a and x86_64, runs unit tests, and
signs the APK with the existing `hushpen` key. It reads these repository secrets:

- `ANDROID_KEYSTORE_B64`: the base64-encoded local keystore.
- `ANDROID_KEYSTORE_PASSWORD`: the keystore password.
- `ANDROID_KEY_PASSWORD`: the signing-key password.

The local key and mode-0600 password file remain under
`/Volumes/External1TB/data/_custom/droid/hushpen/signing/android/`.
Never replace the key: Android updates must use the same signing identity.
The workflow removes the decoded key after use and verifies the APK signature
before uploading the build artifact. See [the phone guide](../mobile/README.md)
for native build and emulator checks.

## iOS archive and owner upload

`mobile/ios/project.yml` owns the Release archive configuration for team
`GF57JXJF5A`. The app embeds `HushpenKeyboard.appex`; the extension links no
Rust engine. Both targets share `group.com.mandalsuraj.hushpen`.

Build the core and generate the project, then check an unsigned device archive:

```sh
bash mobile/build-core.sh ios
bash mobile/ios/generate.sh
xcodebuild archive -project mobile/ios/Hushpen.xcodeproj \
  -scheme Hushpen -configuration Release -destination 'generic/platform=iOS' \
  -derivedDataPath "$AGENT_DATA_DIR/hushpen/ios-build/DerivedData" \
  -archivePath "$AGENT_DATA_DIR/hushpen/ios-build/Hushpen.xcarchive" \
  -jobs 2 CODE_SIGNING_ALLOWED=NO
```

This checks archive structure and builds the extension. An unsigned archive
cannot be uploaded. No archive command here uploads a build or creates an app record.

The owner must complete these steps before App Store or TestFlight distribution:

1. Sign in to [Apple Developer](https://developer.apple.com/account/) with team `GF57JXJF5A`.
2. Register explicit App IDs `com.mandalsuraj.hushpen` and `com.mandalsuraj.hushpen.keyboard` if absent.
3. Register `group.com.mandalsuraj.hushpen` and enable it for both App IDs.
4. In [App Store Connect](https://appstoreconnect.apple.com/), accept any required agreements as the Account Holder.
5. Open Apps, click `+`, and select New App.
6. Select iOS, name `Hushpen`, primary language English (U.S.), and bundle ID `com.mandalsuraj.hushpen`.
7. Set SKU `hushpen-ios`, choose the required User Access, and click Create.
8. Set a new `CURRENT_PROJECT_VERSION` in `project.yml` for each upload, then regenerate the project.
9. Confirm signing profiles for both targets include the App Group and use the selected team.
10. Repeat the archive command without `CODE_SIGNING_ALLOWED=NO`, using authorized distribution signing.
11. In Xcode's Archives organizer, select Hushpen, then Distribute App, App Store Connect, and Upload.
12. Review signing, profiles, and entitlements before the final Upload action.
13. Wait for build processing, then open Hushpen's TestFlight tab and select the processed build.
14. Complete export-compliance questions and provide any required encryption documents before testing or submission.
15. Add internal testers, or complete beta review for external testers. App Store release needs a separate review submission.

`mobile/ios/AppStore/metadata.md` holds every App Store Connect field: name,
subtitle, keywords, description, age rating, privacy answers, and review notes.
`caption.html` and `feature.html` there add the caption bands and the feature
image to the real captures from `StoreShotsUITests`.

Hushpen bundles Noise encryption. The Info.plist leaves out
`ITSAppUsesNonExemptEncryption`, so App Store Connect asks the export
compliance questions for each build; the owner answers them.
An app record is required before upload. These account, signing, compliance,
and publication actions are separate from building an archive.

Apple's instructions: [create an app record](https://developer.apple.com/help/app-store-connect/create-an-app-record/add-a-new-app/),
[upload builds](https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds),
[upload from Xcode](https://help.apple.com/xcode/mac/current/en.lproj/dev442d7f2ca.html),
and [export compliance](https://developer.apple.com/help/app-store-connect/manage-app-information/overview-of-export-compliance).

## More

- Design rules: `DESIGN.md`
- Project decisions and names: `specs/decisions.md` and `specs/ddd.md`
