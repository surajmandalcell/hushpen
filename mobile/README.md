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
