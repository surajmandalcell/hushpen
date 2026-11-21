# Hushpen 2.0 decisions

These are the binding decisions for Hushpen 2.0.0. When a feature settles a
new decision, add it here in the same change (D-21).

- **D-01 License.** `AGPL-3.0-or-later` in every crate, the full AGPL v3
  `LICENSE` file, and `THIRD-PARTY-NOTICES.html` inside every package. The
  About "Open-source licenses" row links to it. Every dependency license
  must be compatible with AGPL-3.0-or-later (`deny.toml` allow list).
- **D-02 Version.** 2.5.0. A GitHub release is published only after every
  validation passed and every release file was checked. `CHANGELOG.md` holds
  the notes of each version, and the release text is its section.
  Linux checks and test installs use oci2 (headless: Xvfb in
  `hushpen-val:noble` for UI checks, or name skipped checks). Each major or
  minor release runs `bash packaging/linux/install-on-test-host.sh` from
  `/mnt/sdb/data/hushpen/repo`; patch-only changes skip the build unless
  `--force`. Preserve the checkout, installed app, app data, and installers
  under `/mnt/sdb/data/hushpen/`, and keep the `hushpen-host` user service
  running.
- **D-03 Manual CI.** Every workflow runs by `workflow_dispatch` only.
  `release.yml` has a required `tag` input and checks out that tag. No push,
  tag-push, or pull_request trigger. The one exception is `deploy-docs.yml`.
  CI and Release are started by hand for a release, with no run cap: the
  repo is public, so standard GitHub-hosted runners cost nothing.
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
- **D-11 Window.** 940 px wide on every OS. The height can change from 600 px
  to 900 px, or to the screen height when that is smaller; the app keeps the
  last height in an internal setting and restores it inside the current
  screen. No zoom, no full screen. Close hides the window to the tray (or
  minimizes it with no tray). Wayland cannot hide or unminimize a window, so
  there close drops it and the tray's Show or Settings opens a fresh one on the
  same views. The idle app lists microphones only while a window is in use.
- **D-12 Appearance.** The app follows the system Light or Dark setting and
  changes live. Without a system setting it uses Dark. `DESIGN.md` is the
  binding design system.
- **D-13 Data folder.** macOS: `~/Library/Application Support/Hushpen`.
  Linux: `${XDG_DATA_HOME:-~/.local/share}/hushpen`. The user can move it in
  Settings. The UI shows `~/...` for paths under the home folder.
- **D-14 Privacy.** No telemetry. Network use is limited to model downloads
  the user starts, the update check the user turns on or starts from About,
  the opt-in OpenAI-compatible endpoint, and device discovery, pairing, and
  jobs the user enables in Devices (D-50). `xtask net-audit` scans for other calls.
  While the update check is on, each automatic or Check now check also sends
  the app name, platform, version, and a new-day or new-month flag to
  `apps.mandalsuraj.com`, which keeps only daily counters: no ID, no IP, no
  other data. Check for Updates in About with the check off sends nothing
  there. The host may appear only in `crates/hushpen-app/src/net/update.rs`;
  `xtask net-audit` fails elsewhere. The app stores the last counted UTC day
  and month and moves them only after a 2xx or 404 answer (the server counts before its 404). Privacy line for every
  privacy text: "Update checks send the app name, platform, and version. We
  count them per day and keep nothing that identifies you."
- **D-15 Cleanup.** Rule-based cleanup is on by default and runs offline. AI
  cleanup is opt-in. It uses a local LLM of at most 1.5 GB, or an endpoint
  the user sets. Every machine is offered S1-mini by Superwhisper first
  (English cleanup only) and Qwen3.5 2B second (other languages and Command
  Mode; Command Mode with S1-mini says it needs Qwen3.5 2B). An LLM timeout
  or a failed output check falls back to the rule-based text. The endpoint API
  key lives in a 0600 file, not in the Keychain. Cleanup profiles (General,
  Message, Email, Notes) set S1-mini's trained control values and the prompt
  text Qwen and endpoints get; the user's changes live in
  `cleanup.profileEdits`. `cleanup_prompt` builds both what the Prompt sheet
  shows and what is sent. Per-app profiles stay out (D-10).
- **D-16 Insertion.** The app saves the clipboard, pastes, and restores the
  clipboard. It never types into a secure field. Terminals get their own
  paste key.
- **D-17 Updates.** Off by default. One check per 24 hours plus "Check now".
  About's Check for Updates runs one check even while the check is off, and
  leaves it off.
  The manifest `latest.json` is signed with minisign and the public key is in
  the app. The macOS zip and the AppImage replace themselves; deb, rpm, and
  tar.gz show a notice with the release link. No install id.
- **D-18 Signing.** Shipped macOS files are signed with the owner's
  Developer ID Application certificate (team GF57JXJF5A), with the hardened
  runtime and the microphone entitlement, then notarized and stapled, on the
  owner's Mac with rcodesign and a key file, never the Keychain. CI builds
  stay ad-hoc and never ship (a new signature resets the Microphone,
  Accessibility, and Input Monitoring grants). The app detects a lost grant at
  start and reopens the onboarding permission step. The local Mac test build
  (`harness/mac-test-app.sh`) is signed with the owner's Apple Development
  identity, so its grants survive rebuilds.
- **D-19 Engine build.** Metal on macOS and CPU on Linux. Release builds use
  `GGML_NATIVE=OFF` so one build runs on older CPUs. Cancel finishes in under
  1 s, and an engine crash keeps the audio.
  Dictation and file transcription skip silence with the bundled Silero VAD
  v5.1.2 model, offline and always on. Speech keeps 250 ms of padding on each
  side; a 600 ms pause splits regions, with a speech threshold of 0.5 and a
  minimum speech length of 100 ms. Subtitle times use the original audio
  clock, including the gaps. Empty windows do not fix the detected language.
  Decoding uses no past transcript context, suppresses non-speech tokens,
  and keeps the no-speech threshold at 0.6. The 885,098-byte MIT model ships
  in every package, pinned by SHA-256 in the package checks, not downloaded.
- **D-20 Packages.** macOS zip and dmg; Linux AppImage, deb, rpm, and tar.gz
  for x64 and arm64. File names are `Hushpen-<version>-<target>.<ext>`. The
  dmg is built on CI only. Releases also require Linux x64/arm64 host
  tarballs and a signed Android APK in `SHA256SUMS`. Desktop update targets
  stay unchanged. iOS has a Release archive configuration; the owner creates
  its App Store Connect record before any upload.
- **D-21 Specs.** `specs/ddd.md` (glossary with the naming tree) and
  `specs/decisions.md` (this file) live at the repo root. Features update
  them when they settle naming or behavior.
- **D-22 Linux test harness.** Linux end-to-end tests run the debug app in
  containers with Xvfb, openbox, PulseAudio, and Mesa lavapipe. At most 3
  slots run engine work at once.
- **D-23 Import limits.** The engine child decodes an imported file one
  packet at a time in 120 s windows with 10 s overlap, so memory does not
  grow with file length. Files up to 12 hours are accepted. Translate writes
  English from any spoken language with a multilingual model; an English-only
  (`.en`) model refuses it with `ENGINE_TRANSLATE_UNSUPPORTED`. An
  `import_file` frame without `translate` does not translate.
- **D-24 Chrome and rows.** The window controls, the sidebar app name, and the
  view title center on one line 26 px from the window top (`size::CHROME_CENTER`).
  `DESIGN.md` owns their geometry and the full-width row separators.
  Views use the shared builders in `ui/` (group, rows, controls).
- **D-25 Read aloud engines.** The crate `hushpen-speech` holds the
  `SpeechEngine` seam, the text preparation, and the system voice. An engine
  lists its voices and speaks one chunk of plain text; `synthesize` runs any
  engine on its own thread, resamples each chunk to 24 kHz mono, streams it
  with progress, and cancels between chunks. Text is plain or Markdown (syntax
  removed), at most 1 MiB, in chunks of whole sentences up to 300 bytes. The
  system voice is AVSpeechSynthesizer in process on macOS (never spoken on the
  main thread). On Linux it is espeak-ng, started once per chunk, found at run
  time and never linked; without it the engine reports `SPEECH_UNAVAILABLE`
  with an install hint. speech-dispatcher is not used: `spd-say` cannot hand
  back audio, so pause, seek, and export would not work.
- **D-26 Settings list and search.** Settings is one scrolling list of
  sections, not tabs. The row registry in `settings/search.rs` names every row
  with its section, settings key, label, and aliases; the search matches each
  query word against them and keeps the rows a match depends on (the AI model
  and endpoint rows under AI source). The query is never saved.
  `dictation.maxMinutes`, `audio.cueVolume`, `engine.useGpu`, `engine.threads`,
  and `cleanup.spokenPunctuation` are shown; pickers offer only values inside
  the registry range. Use the GPU shows on macOS only.
- **D-27 About.** About shows the version with the short commit and the
  commit date. `crates/hushpen-app/build.rs` stamps them from
  `HUSHPEN_BUILD_COMMIT` and `HUSHPEN_BUILD_DATE`, else from git; a source
  tree without git shows the version alone. The website address lives only
  in `hushpen_core::links::SITE_URL`. The Engine fact names whisper.cpp;
  About names no framework.

- **D-28 Kokoro voice worker.** Read aloud with Kokoro runs in a third child
  process, `hushpen-tts`, with no engine or HTTP dependency (net-audit and
  two-binary cover it). It uses the `hushpen-llm` frame protocol style through
  the shared supervisor and a host implementing `SpeechEngine` (D-25). The
  runtime is the sherpa-onnx 2.0.0 C API, opened with dlopen from pinned,
  checksummed release libraries (`cargo xtask tts-runtime`); nothing downloads
  at build time and a version mismatch is refused. The package ships no
  espeak-ng, piper-phonemize, or other GPL data: the worker makes phonemes
  itself (the model's lexicon plus number and abbreviation rules). The model
  is Kokoro-82M v1.0 int8 (four pinned files, catalog kind `voice`, under
  `models/tts/`, revision `57adb37fc0e8d4bd430cfc1b8f8c2294945b9af2`),
  `silence_scale` 1.0. A voice model counts as installed only when its stamp
  matches the catalog size and hash. Mixed voices are D-39; Chatterbox runs in
  the same worker (D-35). CPU only (CoreML aborts on this model), and the
  library is never dlclosed (SIGSEGV on Linux).

- **D-29 Linux window look.** `DESIGN.md` owns the compositor, blur, and
  opaque fallback rules. GPUI always takes the ARGB visual on X11;
  `window_chrome::follow_compositor` checks once a second and switches
  transparency live.

- **D-30 Model catalog and details.** Schema 2 records kind (`speech`, `cleanup`, `voice`),
  description, speed and accuracy (1–5), approximate RAM, use case and Hugging Face source.
  Each download pins a verified revision, size and SHA-256. Models groups rows
  by kind and opens their details. Downloads require free space (statvfs) for
  remaining bytes plus 10%. Startup removes partials older than seven days.
  Pins in `replaces: [{bytes, sha256}]` qualify matching older files for
  background updates.
- **D-31 Manual view.** Manual is third in the sidebar, after History, with
  Transcribe (import queue) and Read aloud (tabs per D-58). The code keeps
  `files`, `files.*`, `sidebar.files`, the import History kind and `import.*`
  hooks. History labels imports "File" and dictations with the app name.
  Dropping files anywhere on the main window opens Manual. For an all-`.txt`/`.md` drop,
  Read aloud opens the first file. Otherwise the queue takes all regular files
  and refuses other paths before opening them. Only the engine parses imported
  audio (D-23); it reports header duration as `audio_ms` on first progress
  without blocking the UI, and from 5% elapsed pace estimates time left.
  `files.language` is independent of dictation. English-only Whisper, Turbo
  and both NVIDIA models refuse translation. Read aloud plays chunks as they
  arrive; Export Audio writes the player samples as 16-bit WAV. Settings keys:
  `files.mode`, `files.language`, `files.translate`, `readAloud.engine`,
  `readAloud.voice.system|kokoro|chatterbox`, `readAloud.defaultVoice`,
  `readAloud.speed` (0.5 to 2, steps of 0.05), and `readAloud.format`.
- **D-32 Focus and control edges.** `DESIGN.md` owns focus borders, rings,
  control alignment, shortcut widths, and default labels. The kit theme's
  `focus_ring` is off. Home and Settings share `Mic::choices`; Home shows one
  microphone pop-up and a level meter only during a test.
- **D-33 Pop-up menus.** Every choice of fixed values is a pop-up button
  (`ui::pop_up`). Its menu (`ui::Menu`) is drawn deferred by the kit `Popup`
  under the right end of the button, so a group's `overflow_hidden` never cuts
  it. The list shows at most `MENU_OPTIONS_SHOWN` (8) whole 24px options,
  scrolls past them, and opens with the chosen one in view. The shared builder
  owns the keys: Up and Down move, Enter picks, Esc closes, a press outside
  closes and goes no further, and focus going elsewhere closes it. Each way
  gives the focus back to the button. The menu fill and shadow are the `menu`
  and `shadow` tokens; the tooltip color is the menu color.
- **D-34 Shortcut keys and conflicts.** Settings > Shortcuts has five global slots
  (hold, hands-free, paste last, Command Mode, read aloud); the footer says how
  to change one: "Click a shortcut, then press the keys. Esc cancels." Each
  row has a reset button (only while the slot is not its default), the field,
  and a switch. Hands-free defaults to Option+Tab on macOS (Left Option: Right
  Option is the hold key). Linux keeps the built-in gestures (a double tap of
  the hold key, or the hold key with Space), because Alt+Tab switches windows
  on every Linux desktop. Settings schema 4 replaces an older stored gestures
  text with the new default once, unless another shortcut holds Option+Tab;
  the gestures stay a choice. A slot that is off keeps its keys but is not live
  (`shortcut.enabled` maps a slot key to `false`); with hold off, the hold key
  still starts the built-in gestures when hands-free uses them. Read aloud
  (Ctrl+Cmd+S, Ctrl+Alt+S on Linux) reads the selection like Command Mode and
  plays it with the Read aloud voice; pressed while it plays, it stops, and
  with nothing selected the flow bar says "Select text first".
  Any keys the system reports, held together, are a shortcut (bare, media,
  keypad, and unnamed keys saved as `Key<number>`, plain modifiers, both sides
  of one modifier, up to 6 keys). Esc alone closes the recorder; with other
  keys down it is part of the shortcut. Hushpen refuses, and keeps the old
  one, only a chord another slot has (the message names the slot) or setting
  text that names no key. A chord something else runs now (an X11 passive
  grab, or a macOS system shortcut that is on) is saved but not live: the
  field shows who uses it, with Open Settings, and the slot is retried every
  2 s. While a live shortcut is Cmd+Tab or Cmd+Shift+Tab, Hushpen turns the
  macOS app switcher hotkey off for the login session and restores it when the
  shortcut changes or the app quits; a child `hushpen switcher-guard` restores
  it after a kill or crash. The key that completes a live shortcut, and every
  non-modifier key while the recorder is open, stay away from the front app
  (macOS: a filtering event tap with Accessibility, listen-only with Input
  Monitoring; X11: grabs). Caps Lock on macOS reports only its press, so it
  works for a tap shortcut, not a hold.
- **D-35 Chatterbox and the voice library.** Read aloud also speaks with
  Chatterbox Turbo and Chatterbox Nano (catalog kind `voice`, under
  `models/tts/`) in the same `hushpen-tts` worker, one voice model in RAM at
  a time. They run on ONNX Runtime through `ort` with `load-dynamic`, opened
  from the library file sherpa-onnx already links, so the package ships no new
  runtime. Both read English only: other text fails with `SPEECH_NOT_ENGLISH`,
  and the UI offers the system voice. A voice is cloned from a clip of 5 to
  20 s of speech (a guided microphone take or a WAV, MP3, or M4A file,
  resampled to 24 kHz) kept in `voices/<id>/reference.wav` with `voice.json`.
  The worker caches the voice conditioning as `reference.<model>.cond`; a clip
  it cannot use fails with `SPEECH_VOICE_CLIP`. The default voice (`readAloud.defaultVoice`) is what
  Chatterbox reads with while the Voice menu says Default; empty means the
  model's own voice (`default_voice.wav`).
- **D-36 Read aloud in History.** Each text that Read aloud speaks to the
  end, for Play or Export, saves one history row of kind `read`: the spoken
  text as the final text, the engine key as `model_id`, the voice name as
  `source_name` (so search finds it), the voice language, the length, and
  `audio/<id>.wav` while the audio retention keeps dictation audio. Playing or
  exporting the same speech again adds no row; a stopped or failed read adds
  none. Schema version 2 allows the kind. Reprocess is refused for a read;
  Read aloud again speaks the text once more with the voice chosen now.

- **D-37 Mute other audio while dictating.** On by default. After the start
  cue has played, Hushpen mutes the default system output (CoreAudio mute,
  or volume 0 when the device has no mute; `wpctl` or `pactl` on Linux) and
  restores it before the finish or cancel cue. It restores only output it
  changed. A marker in the data folder records a mute in progress; the next
  start restores the output, so a crash does not leave the system muted. The
  microphone is never muted.

- **D-38 Idle models.** Settings > Engine > When idle applies to the speech,
  cleanup, and voice workers. Keep in memory leaves the model loaded (the
  macOS default: macOS compresses and swaps idle memory itself). Page out
  (Linux only, the Linux default) calls `madvise(MADV_PAGEOUT)` on the weight
  mappings after the idle time; the model stays loaded. Unload frees the model
  after the idle time. `MADV_FREE_REUSABLE` is never used, because it discards
  the weights. A stored value the system cannot do reads as its default.

- **D-39 Kokoro voice mix.** The sherpa C API takes only a speaker id, so
  the worker writes a copy of `voices.bin` (cached in `models/tts/mixes`, at
  most three) whose last speaker holds the weighted average of the two
  voices' [510, 1, 256] style blocks, and loads Kokoro with it. The pinned
  `voices.bin` is only read. A mix travels as the voice id
  `<base>:<100-p>+<with>:<p>`.

- **D-40 Speech models.** Desktop catalog order: Nemotron Labs Unified ASR 0.6B Q8_0
  (default for new installs), Parakeet v3 Q4_K (kept by existing installs,
  phone default), Whisper Large v3 Turbo Q5_0 (languages outside Parakeet's
  25), Small Q5_1 (translate to English) and Base Q5_1 (older computers).
  Startup resets retired selections to Parakeet and removes only known retired
  speech files and their verification stamps.
- **D-41 Speech engine.** The speech child uses catalog filenames and `AsrEngine`.
  Whisper and Parakeet v3 use vendored whisper.cpp 1.9.4. Unified uses parakeet.cpp with
  the same ggml library (Mac Metal, Linux CPU). Only Whisper takes a dictionary prompt.
  Home and Manual suggest Turbo for languages outside the two NVIDIA models' 25.
  Both NVIDIA models and Turbo refuse translation. Manual disables Translate with a tooltip.
  Parakeet v3 detects language automatically. It decodes at most 20 seconds per span,
  cutting near pauses with one second of context on each side, and checks cancel between
  spans and during decoding. Timed subwords form words on the recording clock. An
  unknown detected language is `und`. Packaged notices name NVIDIA, the models,
  converters and licenses (v3 CC-BY-4.0, Unified OpenMDW-1.1).

- **D-42 Word packs.** The Dictionary offers hand-written word packs
  (Indian names, Indian places, Hinglish words, Tech terms) under
  `assets/word-packs/`, all off by default. Pack words never go into the
  user's entries. In the Whisper prompt the user's words come first inside
  the 150-token cap; Parakeet takes no prompt. Only pack words found in the
  text join the cleanup keep-list, and heard-as replacements run after
  cleanup.

- **D-43 Fast mode.** Unified streams locally or from a paired host. On macOS, stable
  words go into the focused field. The flow bar shows the rest. Corrections, cleanup
  and Undo paste edit only a verified span. After interruption, paste the remaining
  words. Unsupported fields, Windows and X11 use final paste. Stop drains audio. Older
  hosts get the recording at Stop. Cleanup and History use the final transcript.
  Other models and imports decode in batch.

- **D-44 Flow bar and Recover.** `DESIGN.md` owns the flow bar geometry and
  motion rules. Esc or the cross keeps the recording 4 s (`RECOVER_MS`) for
  Recover, which transcribes and pastes it again. The audio goes, or the
  cancelled row is saved, when the 4 s end or a new dictation starts. Command
  Mode has no Recover.

- **D-45 Models rows and the default model.** Sections: Speech to Text, Text
  cleanup, Text to Speech (search also matches the old names Speech
  recognition and Voices). A speech row shows the catalog's accuracy and speed
  as two lines of five 6px dots. One quiet word after the name says the
  download percent, Updating N%, Checking, or Failed verification (the only
  one in a status color). Every action is a 22px icon button in a fixed slot:
  Details, Download or Cancel, Set as default (a star, speech only), and
  Delete. The first speech download that finishes becomes the default while no
  other default is downloaded; deleting the default makes the first other
  downloaded speech model the default, or leaves none chosen. The onboarding
  model step shows the same four speech rows, and Continue opens once one
  speech model is downloaded.

- **D-46 Platform policy and state.** `hushpen_core::os::Os` is the shared
  OS identity. Pure capabilities combine it with supplied display and
  compositor facts. Windows defaults to Right Alt gestures, CPU, and Keep
  in memory; Page out is Linux only. Core owns model download/default,
  updater, flow-bar result/refusal, and microphone purpose/generation rules.
  GPUI shells feed facts and apply effects through the existing service
  traits. macOS and Linux behavior stays unchanged.

- **D-47 Windows backends.** Windows x64 builds and unit tests run on the
  `windows-2025` CI runner. Native adapters use the pinned `windows` crate
  behind the existing service traits. Low-level keyboard hooks feed the core
  shortcut engine. Paste checks the foreground window, process, secure field,
  and integrity level; a target with higher privileges gets a copy and a
  notice. Clipboard restore preserves copyable formats and respects newer
  copies. Windows 11 build 22621+ uses Mica; older builds use an opaque
  sidebar. Data uses roaming AppData, private keys use a current-user-only
  DACL, and a private named pipe carries second-launch requests. A data move
  clears old files only after process exit. System voices use WinRT. Model
  workers default to CPU with an AVX2 baseline; Vulkan is an optional build
  feature, off by default. Page out is unavailable. Speech runtime DLLs are
  pinned by size and SHA-256.

- **D-48 Permission and capability checks.** Core builds one check list from
  the OS, capabilities, and live facts. Onboarding and Settings share its
  rows, statuses, and fixed actions. macOS keeps Microphone, Accessibility,
  and Input Monitoring. Windows shows microphone consent, administrator-app
  limits, and keyboard-hook status with Retry. Linux explains Wayland and a
  missing tray watcher. Continue requires a microphone on every OS and the
  two key permissions on macOS. Unavailable keys show no shortcut promise.
  Paste refusal keeps Copied with a notice.

- **D-49 Windows packages and updates.** Windows x64 builds an unsigned
  per-user NSIS setup and a portable ZIP on `windows-2025`. Setup installs
  under `%LOCALAPPDATA%\Programs\Hushpen` without administrator rights; the
  app manifest uses asInvoker and PerMonitorV2. The signed update manifest
  selects setup. After SHA-256 verification, a detached helper waits for
  Hushpen to exit, runs setup with `/S`, and restarts the app. Portable
  installs show a release notice. `install.ps1` checks SHA256SUMS before
  silent setup. Windows files have no Authenticode signature; SmartScreen
  warns. CI packages do not publish a release or the website.

- **D-50 Devices and remote setup.** Devices follows Models. Flow D has
  separate speech and cleanup orders. Remote starts off. This computer stays
  last unless local models are disabled; unavailable remote routes then
  refuse the job. Core owns routing, health, pairing state, fallback notices,
  and unload rules. Route rows name models; devices report none. Phones use
  system cleanup. D-57 owns Add a Device and pairing.
  SSH setup uses concrete Host aliases and BatchMode, verifies SHA-256
  before installation, starts the user service, and receives its offer over
  authenticated SSH. It tries Tailscale IPv4 before configured and offered
  addresses, saving only one that passes Noise authentication. When no
  address answers, it pairs through an SSH tunnel (D-55) and saves that route.
  `DESIGN.md` owns the desktop page and sheets; `mobile/SCREENS.md` records
  phone screen state and intents.

- **D-51 Shared phone core.** Native phone apps call `hushpen-mobile` through
  pinned UniFFI bindings. Rust owns dictation, model rules, verified downloads,
  pairing, and serving. Parakeet v3 Q4, Whisper Base Q5, and Small Q5 are
  available on every phone; Turbo Q5 follows core's physical RAM policy.
  Cleanup uses the system model first, then S1-mini by Superwhisper or Qwen3.5 2B
  when core's RAM policy and the app's current free memory allow it. Files,
  checksums, prompts, profiles, and output checks match desktop. Failure keeps
  rule-cleaned text. Speech and llama.cpp run on threads over the existing
  bounded worker protocol. iOS uses Metal; Android uses CPU and NEON. QNN and
  Core ML acceleration need a later decision. The iOS keyboard never loads
  engines.

- **D-52 Android app and keyboard.** `mobile/android` uses Kotlin and Compose
  over the generated Rust phone API, minimum Android 10. The IME records into
  a session WAV, feeds the Rust dictation machine, and inserts through
  `InputConnection`. Password fields refuse dictation. Undo requires the same
  field, cursor, and inserted text. A desktop speech session reserves the
  shared engine until it disconnects. Speech and cleanup selections are
  separate and persist; cleanup capability stays paired when off or
  unavailable. Server mode uses a `specialUse` foreground service (declared in
  the manifest) for paired desktops' requests, with a quiet notification while
  serving. A Rust job callback holds a wake lock only during model work.
  Memory callbacks and Android's low-memory reading feed Rust's unload rule;
  `MemoryInfo.totalMem` supplies core's RAM policy. Cleanup is opt-in. ML Kit
  needs a ready system model and the app in front; a keyboard or foreground
  service does not remove Google's
  [background limit](https://developers.google.com/ml-kit/genai#background_usage).
  Local cleanup can run in the background within the memory budget; failure
  keeps rule text. The network security configuration denies cleartext and
  trusts no TLS authorities, and platform HTTP(S) is rejected before SDK
  initialization, because ML Kit has no supported metrics opt-out
  ([data disclosure](https://developers.google.com/ml-kit/android-data-disclosure)).
  Rust owns the permitted model downloads and encrypted device links.

- **D-53 iOS app and keyboard.** `mobile/ios` uses SwiftUI and observable
  screen models over the shared Rust phone API, with iOS 26 as its minimum.
  XcodeGen owns the project. The app and keyboard share
  `group.com.mandalsuraj.hushpen`; only the app links the Rust model engine.
  A keyboard flow session opens `hushpen://dictate?session=...` in the app,
  which records, transcribes, and cleans the text. The keyboard inserts the
  result by itself at the cursor the field has then; typing or moving
  meanwhile does not cancel it. Results expire after ten minutes and are used
  once, consumed before the insert. iOS reports no insert result, so the
  keyboard checks that the text before the cursor changed; on a miss it copies
  the text and says so, or offers Copy. Typing or moving the cursor withdraws
  undo. Shared files are protected and excluded from backups. The keyboard
  writes its Full Access heartbeat to the App Group; the app checks setup on
  foreground. Server mode serves only in
  front, uses core's fixed unload rule, dims the screen, and restores
  brightness and the idle timer on exit. Cleanup is off by default and follows
  D-51. `ProcessInfo.physicalMemory` supplies core's RAM policy; the process
  budget also limits local cleanup. iOS advertises through native Bonjour
  (other platforms keep `mdns-sd`), which avoids the restricted multicast
  entitlement ([Apple's rules](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)).

- **D-54 Device link.** Dictation and job traffic go from device to device
  over Noise_XX_25519_ChaChaPoly_BLAKE2s on TCP, with the Noise keys pinned
  at pairing (D-50). No Hushpen relay, STUN, signalling, or hole-punching
  service exists, and no Hushpen server is in the path.

- **D-55 Off-LAN.** Devices on different networks connect at an address the
  user routes: their own Tailscale, a VPN, or SSH (D-50). Hushpen does not
  embed Tailscale (no tsnet or libtailscale) and runs no relay. A new off-LAN
  transport needs a new decision, with its hosts added to `xtask net-audit`
  (D-14). A server added over SSH whose port 47321 does not answer directly is
  reached through `ssh -L` to its loopback port, run by the user's own ssh and
  `~/.ssh/config`; Noise still encrypts and authenticates, SSH is only the pipe.

- **D-56 One anonymous usage number a day.** Desktop only, inside the D-14
  update check (so only while updates are on). The first check of a new day
  adds `u`: the previous local day's uses (completed dictations, file
  transcriptions, and read-alouds, capped at 1023) in bits 0-9, a bitmask of
  the models used in bits 10-25 (slot list `hushpen_core::usage::MODEL_SLOTS`,
  same order in the API), and version 1 in bits 26-27. Settings "Share
  anonymous usage" is on by default and turns it off; "See what's sent" shows
  the exact request. No install ids, no other analytics; the server stores no
  IP or id.

- **D-57 Add a Device pairing.** `DESIGN.md` owns the Nearby | Remote sheet.
  mDNS is discovery only. Both peers commit before revealing fresh randomness.
  Verify commitments, then derive the same 6-digit **pairing number**; require
  Pair on both screens before saving trust. Cancel or mismatch discards the
  session; retries use fresh commitments. Under the list, always show a QR
  pinning the computer's key and an 8-digit **pairing code** for a PAKE
  (password-authenticated key exchange). Offers work once for two minutes,
  with three shared tries. Never send the typed secret as a password. Bind
  commitments and mutual PAKE confirmation to both keys, roles, computer name,
  and Noise transcript before enrollment. Send the validated name in the
  protected handshake. Show no copyable URI or paste field. The sheet alone
  opens pairing; closing cancels sessions and offers. Desktop: at most four
  connections; unauthenticated work has five seconds, then number comparison
  has 60 seconds. Nearby failures do not spend QR/code tries. Show the peer's
  protected name; match selected discovery ids to static keys. Remote merges
  SSH/Tailscale hosts and a host field; SSH carries offers (D-50).
  The PAKE is RustCrypto [`spake2 = "=0.4.0"`](https://docs.rs/crate/spake2/0.4.0),
  MIT OR Apache-2.0, and passes all four `deny.toml` checks.
  [Upstream](https://docs.rs/spake2/0.4.0/spake2/#security) notes no
  independent audit and no timing-attack protection; review the protocol
  before release. Tests cover mismatch, replay, expiry, three tries, cancel,
  names, and approvals.

- **D-58 Home screen.** The main window body has a title-bar tab row on the
  right: Home | Transcribe | Read aloud (Manual is split into the last two).
  Home shows a stats block (number tiles plus a words-per-day map), then the 5
  newest transcriptions, each with a copy button on hover and a "Show more"
  link that opens History. Stats are computed on this Mac from History rows
  plus a daily totals table that "Clear all" does not wipe. They are never sent
  anywhere (D-56 still holds). Transcribe uses a big drop card with a file
  stack and supports: several files at once, paste a file or text, drop on the
  Dock icon, Open With and Services, drag the result out, a notification when
  done, and Quick Look with Space. Look details live in `DESIGN.md`.
