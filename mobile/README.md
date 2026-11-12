# Shared phone core

`bash mobile/build-core.sh` builds the Rust phone library and generates Swift
and Kotlin bindings. It installs the four Rust targets and pinned `cargo-ndk`
when missing. Android requires NDK 28.2.13676358 under `ANDROID_HOME`.
iOS requires Xcode on the Mac. The `android`, `ios`, and `bindings` arguments
build one part. Use `check` to check the toolchains.

Outputs live under `$CARGO_TARGET_DIR/mobile`: Android libraries in
`android/jniLibs`, Swift and Kotlin in `bindings`, and the device/simulator
XCFramework in `ios/HushpenCore.xcframework`. iOS devices use Metal; the iOS
simulator uses CPU. Android uses CPU, with NEON on arm64. QNN and Core ML
acceleration remain a later decision.

Call blocking Rust operations from a native worker thread. Both local and
served cleanup use one `SystemCleanup`. It tries the native `SystemLlm`
callback first, then the selected downloaded model when the system model is
unavailable. Local cleanup runs the desktop llama.cpp worker on a thread,
with the same core profiles, prompts, and output checks. Missing models,
low memory, failures, and timeouts keep the rule-cleaned text.
Phone apps, keyboard extensions, and release packages are separate builds.

Both apps pass reported physical RAM to the core phone catalog. Parakeet,
Whisper Base Q5, and Whisper Small Q5 are always offered. Larger speech and
cleanup models use the shared RAM gates in `hushpen-core/src/catalog.rs`.
Downloads reuse `assets/models.json` files and checksums. Local cleanup
checks current free app memory before loading and unloads after each job.
The iOS keyboard still loads no model.

Phone RAM gates still need phone measurements. Desktop memory figures supply
the current free-memory checks; they do not prove a phone's peak usage.
No increased-memory-limit entitlement is enabled without that evidence.

`bash mobile/tests/build-core.sh` checks the build commands without compiling.
The headless Swift smoke test is `mobile/tests/ffi.swift`. Compile it with the
generated `HushpenCore.swift`, the generated header directory, and
`-L "$CARGO_TARGET_DIR/debug" -lhushpen_mobile -lc++`. Pass a scratch folder
to the executable. It checks catalog, dictation, callbacks, and detector setup.

## Android app

`mobile/android` builds the Android app and input method. Install JDK 17 and
Android SDK 36. The minimum is Android 10 (API 29). Build the core first, then:

```sh
export ANDROID_HOME=/Volumes/External1TB/Data/Library/Android/sdk
export HUSHPEN_MOBILE_CORE=/Volumes/External1TB/data/_custom/droid/hushpen/target/macos-arm64/mobile
export HUSHPEN_ANDROID_BUILD_DIR="${AGENT_DATA_DIR:-/Volumes/External1TB/data/_custom/droid}/hushpen/android-build"
cd mobile/android
./gradlew testDebugUnitTest assembleDebug
./gradlew connectedDebugAndroidTest
```

Gradle output and the project cache use `HUSHPEN_ANDROID_BUILD_DIR`. Keep
`GRADLE_USER_HOME` and Android AVD storage on the data drive. The app stores
models, settings, pairing keys, and session audio in its private Android
sandbox. Backup is off. Screens use default placeholder controls; their
state and actions are in `SCREENS.md`.
One desktop speech session or local dictation owns the speech engine at a
time. Stop Server mode to release a desktop session before dictating locally.

For instrumentation, install the debug APK before staging fixtures with
`adb shell run-as com.mandalsuraj.hushpen`. Put the spoken WAV at
`files/ime-test.wav`, the pinned Whisper Base Q5 file at
`files/hushpen/models/whisper/ggml-base-q5_1.bin`, and a desktop pairing QR
image at `files/qr-test.png`. Tests verify the model through Rust. They
enable and select the IME, drive default keyboard controls, run real speech,
check undo and input changes, and exercise camera and service lifetimes.
The WAV hook and server pairing activity exist only in debug builds.

`PhoneModelsTest` checks the physical-RAM catalog without extra model fixtures.
Its cleanup, Turbo load, and selection-restart tests require
`-Pandroid.testInstrumentationRunnerArguments.phoneModelFixtures=true` and the
pinned files under `files/hushpen/models/llm` or `files/hushpen/models/whisper`.
Run each measurement separately to keep the process memory peak independent.
The test reports latency, sampled peak RSS, process high-water RSS, and PSS.
Qwen measurements require at least 8 GB of reported physical RAM.
The `android` desktop harness suite checks remote insertion and local fallback;
see `harness/README.md` for its two stages.

ML Kit Prompt cleanup is off by default. It uses an already available
system model while the app is in front. A keyboard or foreground service
cannot remove [ML Kit's background restriction](https://developers.google.com/ml-kit/genai#background_usage).
When native cleanup is unavailable, the selected downloaded cleanup model
can run in the keyboard or foreground service. Otherwise, Rust rules clean the text.
Android blocks platform SDK HTTP and HTTPS traffic, including ML Kit metric
uploads. Rust owns approved downloads and paired connections. Android's
[network policy](https://developer.android.com/privacy-and-security/security-config)
applies in debug and release builds; neither build trusts a platform TLS CA.

`./gradlew assembleRelease` makes the release APK. Gradle can sign it when
`HUSHPEN_ANDROID_KEYSTORE`, `HUSHPEN_ANDROID_STORE_PASSWORD`, and
`HUSHPEN_ANDROID_KEY_PASSWORD` are set. The alias defaults to `hushpen`.
Alternatively, sign the unsigned output with Android's `apksigner`, using
`--ks-pass file:<password-file>`. Omit `--key-pass` when both passwords match.
The signing key and password file live in
`/Volumes/External1TB/data/_custom/droid/hushpen/signing/android/`, with mode
0600. Never commit or replace them. Release CI reads the existing key through
the three Android secrets described in [the release guide](../docs/building.md#phone-and-host-release-files).

## iOS app

`mobile/ios` requires Xcode, XcodeGen 2.44 or later, and iOS 26 or later.
Generate the project from `project.yml`; do not commit generated projects or plists.

```sh
bash mobile/build-core.sh ios
bash mobile/ios/generate.sh
xcodebuild build-for-testing -project mobile/ios/Hushpen.xcodeproj \
  -scheme Hushpen -destination 'platform=iOS Simulator,name=iPhone 18 Pro,OS=27.0' \
  -derivedDataPath "$AGENT_DATA_DIR/hushpen/ios-build/DerivedData" \
  -jobs 2 CODE_SIGN_IDENTITY=-
```

On this Mac, Askama's binding generator cannot resolve its templates through
the symlinked default Cargo registry. For core generation, use the existing
real-path Cargo home with
`CARGO_HOME=/Volumes/External1TB/data/_custom/droid/hushpen/cargo-home`.
Do not change the shared cache configuration.

Keep simulator data on the data drive, on an APFS volume with ownership enabled.
An APFS sparse disk image stored there can provide ownership without changing
the main drive's settings. A device set on a volume with ownership disabled
can fail creation with Cocoa error 513 or EPERM. The app needs microphone
and Local Network permission; the scanner needs camera permission.

For flow UI tests, install the debug app and `HushpenKeyboardTestHost.app`
from the build products on the same simulator. Copy `speech-short.wav` to
the app's `Documents/flow-test.wav` and the pinned Base Q5 model to
`Documents/ggml-base-q5_1.bin`. Find the app container with
`xcrun simctl get_app_container <simulator-id> com.mandalsuraj.hushpen data`.
Grant the isolated simulator's microphone permission with
`xcrun simctl privacy <simulator-id> grant microphone com.mandalsuraj.hushpen`.
The tests verify and select the model through Rust, enable the keyboard with
Full Access through Settings, and drive cold-launch dictation, insertion,
undo, cancellation, field changes, and foreground Server mode.

```sh
xcodebuild test-without-building -project mobile/ios/Hushpen.xcodeproj \
  -scheme Hushpen -destination 'platform=iOS Simulator,name=iPhone 18 Pro,OS=27.0' \
  -derivedDataPath "$AGENT_DATA_DIR/hushpen/ios-build/DerivedData" \
  -parallel-testing-enabled NO -test-timeouts-enabled YES \
  -collect-test-diagnostics never \
  -default-test-execution-time-allowance 240 CODE_SIGN_IDENTITY=-
```

For prepared products, replace `-project`, `-scheme`, and `-derivedDataPath`
with `-xctestrun <products.xctestrun>`. Boot the owned simulator with
`xcrun simctl boot <simulator-id>`; no Simulator window is needed.
Disabling verbose diagnostics avoids a stalled system diagnostic collection
after a failed test. Test assertions and result bundles remain enabled.

`PhoneMeasurements` skips when its pinned weights are absent. Stage them in
the app's `Documents/PhoneMeasurements/models/llm` or `models/whisper` folder.
Run each method alone with `-only-testing:HushpenTests/PhoneMeasurements/<method>`
in a fresh test process. Each method reports elapsed time and process peak RSS;
running several methods together mixes their memory peaks. Simulator RAM
overrides test catalog selection, not a physical iPhone's memory limit.

`StoreShotsUITests` takes the App Store screenshots from a real run: stage the
simulator as above (a 16 kHz mono `flow-test.wav` of the sentence to dictate),
boot it with `xcrun simctl status_bar <id> override --time 9:41 --batteryState
charged --batteryLevel 100`, light appearance, and no Simulator window. Run
`test1EnableKeyboard`, `test2MessagesDictation`, `test3Screens`, and
`test4LiveActivity` with `-only-testing` and
`TEST_RUNNER_HUSHPEN_SHOTS_DIR=<folder>`. The tests skip without that variable.
Use an iPhone 17 Pro Max (1320 by 2868) and an iPad Pro 13-inch M5 (2064 by 2752).

The WAV hook exists only in debug builds. The extension links no Rust model
engine and never records. It starts a flow through the app's session URL.
Recording can continue after the user returns to the keyboard. The keyboard
sends session-scoped stop and cancel commands through the App Group and
Darwin notifications. It receives changes only while visible, without polling.
Insert dictation confirms the active field because iOS can replace the
input-session identifier on return.
Results expire after ten minutes and require unchanged cursor context.
Changing the field or cursor withdraws the offer and undo. iOS exposes no
public enabled-keyboard identifier property. The app reads the extension's
App Group heartbeat on return and checks active input modes when their
descriptions identify Hushpen. Full Access is last observed state.

Server mode serves only while the app is in front. It restores brightness and
the idle timer when it stops. iOS uses native Bonjour registration to avoid
the restricted entitlement for raw multicast; other platforms keep `mdns-sd`.
See [Apple's local network rules](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy).
Cleanup is off by default and uses the available on-device
[Foundation Models system model](https://developer.apple.com/documentation/foundationmodels/systemlanguagemodel).
Without Foundation Models, the selected downloaded cleanup model runs in
the app. Missing models, low memory, or failed cleanup keep Rust's rule-cleaned text.
The scheme archives the app and embedded keyboard in Release, with debug
symbols. See [the archive and upload guide](../docs/building.md#ios-archive-and-owner-upload)
for the unsigned archive check and the owner's App Store Connect steps.

## Idle work

Both keyboards use the Rust dictation deadline instead of repeated ticks.
Android cancels its deadline and releases local capture and speech on hide.
Memory callbacks feed its server unload rule without a background polling task.
Local flow cleanup preserves models owned by Server mode.

iOS opens its audio session only for recording. ActivityKit lifecycle logic
tracks the active flow and ends at completion, cancellation, failure, or expiry.
The embedded ActivityConfiguration widget defines its display. Display on a
physical phone remains unverified.
A [Live Activity](https://developer.apple.com/documentation/ActivityKit/displaying-live-data-with-live-activities)
does not grant background execution. Real background recording uses the audio
mode; processing uses a finite background task. No silent audio
keeps the app alive between flows, so a new flow uses the app URL again.

In-process and selected remote workers send no idle pings. Active jobs retain
their watchdog. Phone listeners wait for socket events and wake once on stop.
Unix encrypted streams use blocking socket waits; Windows retains cancellation
polling. Authentication, worker selection, uploads, and partial encrypted
packets keep their time limits. Bonjour waits for events. Other
platforms check changed network interfaces every five minutes, which can delay
advertising on a new network. Query responses and protocol announcements remain.
Linux memory checks run only for the low-memory unload rule and back off from
30 seconds to five minutes. Selected model unload deadlines remain unchanged.
