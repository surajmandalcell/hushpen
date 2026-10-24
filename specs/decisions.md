# Hushpen 2.0 decisions

These are the binding decisions for Hushpen 2.0.0. When a feature settles a
new decision, add it here in the same change (D-21).

- **D-01 License.** `AGPL-3.0-or-later` in every crate, the full AGPL v3
  `LICENSE` file, and `THIRD-PARTY-NOTICES.html` inside every package. The
  About "Open-source licenses" row links to it. Every dependency license
  must be compatible with AGPL-3.0-or-later (`deny.toml` allow list).
- **D-02 Version.** 2.0.0. The GitHub release v2.0.0 is published only after
  every validation passed and every release file was checked.
- **D-03 CI budget.** Every workflow runs by `workflow_dispatch` only.
  `release.yml` has a required `tag` input and checks out that tag. No push,
  tag-push, or pull_request trigger. The one exception is `deploy-docs.yml`.
  GitHub Actions gets at most 35 runs; the hard limit is 50.
- **D-04 Docs site.** `docs/**` deploys to GitHub Pages on push and changes
  once, in the publish step after the release.
- **D-05 GPUI.** The UI is GPUI (`gpui-pre =0.3.7`, GPUI Kit `=0.7.0`) with
  the engine in Rust. No web view and no Python sidecar. Crates use exact
  `=` pins. Component tests use `TestAppContext`; pixel tests render offscreen
  on Linux.
- **D-06 Two binaries.** `hushpen` (UI and the `engine` subcommand) links
  whisper.cpp. `hushpen-llm` links llama.cpp. Each brings its own ggml, and
  in one binary one copy silently replaces the other. `hushpen-llm` never
  depends on `hushpen-engine`, and the reverse. An xtask check fails if one
  binary has both sys crates. Both binaries ship in every package.
- **D-07 No data migration.** Hushpen reads nothing from the 1.x apps or from
  the Electron builds: no settings, history, or models are imported. It
  starts with a new data folder.
- **D-08 Test hook.** A debug-only hook (cargo feature `test-automation`)
  reports the element tree and the state and runs actions for the Linux and
  Mac checks. A release build never contains it, and `cargo build --release
  --features test-automation` is refused with a `compile_error!`. `xtask
  release-check` scans the binaries and the packages for it. The same rule
  covers the `update-test-feed` feature.
- **D-09 Gate.** Merging needs `cargo fmt --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`, `cargo deny
  check`, `xtask two-binary`, and `xtask net-audit`. `cargo xtask gate` runs
  them in one step.
  Warnings are errors. The toolchain is Rust 1.99.0 (`rust-toolchain.toml`).
- **D-10 Platforms.** macOS 13 or later on arm64, and Linux x64 and arm64 on
  X11. On Wayland the global key and paste show "not available". Linux needs
  a Vulkan or GL driver. Not in 2.0: Windows, Intel Mac, UI languages other
  than English, speaker labels, live captions, bundles, notes, stats,
  snippets, per-app styles, video import, Opus or WebM import, code signing,
  and app stores.
- **D-11 Window.** Fixed at 940x600 on every OS: not resizable, no zoom, no
  full screen. On X11 the size hints set min equal to max. One app instance
  runs at a time.
- **D-12 Appearance.** The app follows the system Light or Dark setting and
  changes live. Without a system setting it uses Dark. `DESIGN.md` is the
  binding design system.
- **D-13 Data folder.** macOS: `~/Library/Application Support/Hushpen`.
  Linux: `${XDG_DATA_HOME:-~/.local/share}/hushpen`. The user can move it in
  Settings. The UI shows `~/...` for paths under the home folder.
- **D-14 Privacy.** No telemetry. Network use is limited to model downloads
  the user starts, the update check the user turns on, and the opt-in
  OpenAI-compatible endpoint. `xtask net-audit` scans for other calls.
- **D-15 Cleanup.** Rule-based cleanup is on by default and runs offline. AI
  cleanup is opt-in. It uses a local LLM chosen by RAM, or an endpoint the
  user sets. An LLM timeout or a failed output check falls back to the
  rule-based text. The endpoint API key lives in a 0600 file, not in the
  Keychain.
- **D-16 Insertion.** The app saves the clipboard, pastes, and restores the
  clipboard. It never types into a secure field. Terminals get their own
  paste key.
- **D-17 Updates.** Off by default. One check per 24 hours plus "Check now".
  The manifest `latest.json` is signed with minisign and the public key is in
  the app. The macOS zip and the AppImage replace themselves; deb, rpm, and
  tar.gz show a notice with the release link. No install id.
- **D-18 Signing.** Shipped macOS files are signed with the owner's
  Developer ID Application certificate (team GF57JXJF5A), with the hardened
  runtime and the microphone entitlement, then notarized and stapled. The
  signing runs on the owner's Mac with rcodesign and a key file, never the
  Keychain. CI builds stay ad-hoc and never ship: a new signature resets the
  Microphone, Accessibility, and Input Monitoring grants. The app detects a
  lost grant at start and reopens the onboarding permission step. The local
  Mac test build (`harness/mac-test-app.sh`) is signed with the owner's
  Apple Development identity, so its grants survive rebuilds.
- **D-19 Engine build.** Metal on macOS and CPU on Linux. Release builds use
  `GGML_NATIVE=OFF` so one build runs on older CPUs. Cancel finishes in under
  1 s, and an engine crash keeps the audio.
- **D-20 Packages.** macOS zip and dmg; Linux AppImage, deb, rpm, and tar.gz
  for x64 and arm64. File names are `Hushpen-<version>-<target>.<ext>`. The
  dmg is built on CI only.
- **D-21 Specs.** `specs/ddd.md` (glossary with the naming tree) and
  `specs/decisions.md` (this file) live at the repo root. Features update
  them when they settle naming or behavior.
- **D-22 Linux test harness.** Linux end-to-end tests run the debug app in
  containers with Xvfb, openbox, PulseAudio, and Mesa lavapipe. At most 3
  slots run engine work at once.

## History notes

ASR Pro was the name of the Electron app (1.x, and a 2.0 mission that the
Rust rewrite replaced). Its decisions D-01 to D-40 covered Electron, IPC,
the renderer, Windows, and the old data folders. They no longer apply and
stay in git history.
