# Shared phone core

`bash mobile/build-core.sh` builds the Rust phone library and generates Swift
and Kotlin bindings. It installs the four Rust targets and pinned `cargo-ndk`
when missing. Android requires NDK 28.2.13676358 under `ANDROID_HOME`.
iOS requires Xcode on the Mac. The `android`, `ios`, and `bindings` arguments
build one part. Use `check` to check the toolchains.

Outputs live under `$CARGO_TARGET_DIR/mobile`: Android libraries in
`android/jniLibs`, Swift and Kotlin in `bindings`, and the device/simulator
XCFramework in `ios/HushpenCore.xcframework`. iOS uses Metal. Android uses CPU,
with NEON on arm64. QNN and Core ML acceleration remain a later decision.

Call blocking Rust operations from a native worker thread. Both local and
served cleanup use one `SystemCleanup` over the native `SystemLlm` callback.
Unavailable, failed, or timed-out callbacks keep the rule-cleaned text.
Phone apps, keyboard extensions, and release packages are separate builds.

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
The `android` desktop harness suite checks remote insertion and local fallback;
see `harness/README.md` for its two stages.

ML Kit Prompt cleanup is off by default. It uses an already available
system model while the app is in front. A keyboard or foreground service
cannot remove [ML Kit's background restriction](https://developers.google.com/ml-kit/genai#background_usage).
Unavailable or failed native cleanup keeps Rust's rule-cleaned text.
Android blocks platform SDK HTTP and HTTPS traffic, including ML Kit metric
uploads. Rust owns approved downloads and paired connections. Android's
[network policy](https://developer.android.com/privacy-and-security/security-config)
applies in debug and release builds; neither build trusts a platform TLS CA.

`./gradlew assembleRelease` makes the release APK. Gradle can sign it when
`HUSHPEN_ANDROID_KEYSTORE`, `HUSHPEN_ANDROID_STORE_PASSWORD`, and
`HUSHPEN_ANDROID_KEY_PASSWORD` are set. The alias defaults to `hushpen`.
Alternatively, sign the unsigned output with Android's `apksigner`, passing
passwords through its `file:` options. The one local signing key and its
password file live in
`/Volumes/External1TB/data/_custom/droid/hushpen/signing/android/`, with mode
0600. Never commit or replace them. CI signing and release publication are
separate release steps.
