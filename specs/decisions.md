# Hushpen 2.0 decisions

These are the binding decisions for Hushpen 2.0.0. When a feature settles a
new decision, add it here in the same change (D-21).

- **D-01 License.** `AGPL-3.0-or-later` in every crate, the full AGPL v3
  `LICENSE` file, and `THIRD-PARTY-NOTICES.html` inside every package. The
  About "Open-source licenses" row links to it. Every dependency license
  must be compatible with AGPL-3.0-or-later (`deny.toml` allow list).
- **D-02 Version.** 2.4.0. A GitHub release is published only after every
  validation passed and every release file was checked. `CHANGELOG.md` holds
  the notes of each version, and the release text is its section.
  Linux checks and test installs use the closer of `ssh oci1` and `ssh oci2`:
  oci2 (0.46 s versus 3.25 s). Each major or minor release requires
  `bash packaging/linux/install-on-test-host.sh` from
  `/mnt/sdb/data/hushpen/repo`. First installation is required; patch-only
  changes skip the build unless `--force` is requested. Preserve the checkout,
  toolchain, target, installed app, app data, and installers under
  `/mnt/sdb/data/hushpen/`. Home links expose `~/dev/personal/hushpen` and
  `~/Downloads/Hushpen`. Keep the native `hushpen-host` user service running.
  Oci2 is headless: use Xvfb in `hushpen-val:noble` for UI checks, or name
  skipped checks. This local package does not replace public Ubuntu 22.04
  packages or enable automatic in-app patch updates.
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
  screen. No zoom, no full screen.
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
- **D-15 Cleanup.** Rule-based cleanup is on by default and runs offline. AI
  cleanup is opt-in. It uses a local LLM of at most 1.5 GB, or an endpoint
  the user sets. Every machine is offered S1-mini by Superwhisper first
  (English cleanup only) and Qwen3.5 2B second (other languages and Command
  Mode). Command Mode with S1-mini says it needs Qwen3.5 2B. An LLM timeout or a failed output check falls back to the
  rule-based text. The endpoint API key lives in a 0600 file, not in the
  Keychain. Cleanup profiles (General, Message, Email, Notes) set S1-mini's
  trained control values (General semi-formal/prose/general, Message
  semi-casual/prose/general, Email semi-formal/prose/email, Notes
  semi-formal/lists/general) and the prompt text Qwen and endpoints get; the
  user's changes live in `cleanup.profileEdits`. One function,
  `cleanup_prompt`, builds both what the Prompt sheet shows and what is sent.
  Per-app profiles stay out (D-10).
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
  packet at a time while it transcribes it in 120 s windows with 10 s of
  overlap, so memory does not grow with the length of the file (tiny.en:
  about 470 MB peak for 2 min, 10 min, and 1 h on the Mac; 344 MB peak for
  a 5 h file in a Linux slot, reached in the first 30 s). Files up to 12 hours are
  accepted; the limit only bounds how long one job runs. Translate writes
  English from any spoken language with a multilingual model; an
  English-only (`.en`) model refuses it with
  `ENGINE_TRANSLATE_UNSUPPORTED`. An `import_file` frame without `translate`
  does not translate.
- **D-24 Chrome and rows.** The window controls, the sidebar app name, and the
  view title center on one line 26 px from the window top (`size::CHROME_CENTER`).
  Linux draws its lights at (20, 20) like macOS; on macOS the lights' button
  frame is read after the window opens and placed so its center is on the line.
  A line between rows spans the full group width on every page. Views build
  their rows from the shared builders in `ui/` (group, rows, controls).
- **D-25 Read aloud engines.** The crate `hushpen-speech` holds the speech
  engine seam, the text preparation, and the system voice. It is its own
  crate, not part of `hushpen-platform`, because it needs `hushpen-audio` to
  resample and because the Kokoro host implements the same `SpeechEngine`
  trait. An engine lists its voices and speaks one chunk of plain text;
  `synthesize` runs any engine on its own thread, resamples each chunk to
  24 kHz mono (Kokoro's rate), streams it with progress, and cancels between
  chunks. Text is plain or Markdown (syntax removed), at most 1 MiB, in
  chunks of whole sentences up to 300 bytes. The system voice is
  AVSpeechSynthesizer in process on macOS: its buffers arrive on the main
  queue, so a chunk is never spoken on the main thread. On Linux it is
  espeak-ng, started once per chunk, found at run time and never linked;
  without it the engine reports `SPEECH_UNAVAILABLE` with an install hint,
  and the UI hides or explains the engine. speech-dispatcher is not used:
  `spd-say` cannot hand back audio, so pause, seek, and export would not
  work.
- **D-26 Settings list and search.** Settings is one scrolling list of
  sections (a heading above each group), not tabs. The row registry in
  `settings/search.rs` names every row with its section, settings key,
  label, and aliases; the search matches each word of the query against
  them and keeps the rows a match depends on (the AI model and endpoint rows
  under AI source). The query lives only in the view and is never saved.
  `dictation.maxMinutes`, `audio.cueVolume`, `engine.useGpu`, `engine.threads`,
  and `cleanup.spokenPunctuation` are shown; the pickers offer only values
  inside the registry range. Use the GPU shows on macOS only (Linux runs the
  engine on the CPU).
- **D-27 About.** About shows the version with the short commit and the
  commit date. `crates/hushpen-app/build.rs` stamps them from
  `HUSHPEN_BUILD_COMMIT` and `HUSHPEN_BUILD_DATE`, else from git; a source
  tree without git shows the version alone. The website address lives only
  in `hushpen_core::links::SITE_URL`. The Engine fact names whisper.cpp;
  About names no framework.

- **D-28 Kokoro voice worker.** Read aloud with the Kokoro voice runs in a
  third child process, `hushpen-tts`, a crate with no engine or HTTP
  dependency (net-audit and two-binary cover it). It speaks the same frame
  protocol style as `hushpen-llm` (ListVoices, Speak, audio in binary frames,
  Done, Cancel, idle unload), and the app drives it through the one shared
  supervisor and a host that implements `SpeechEngine` (D-25). The runtime is
  the sherpa-onnx 2.0.0 C API, opened with dlopen from pinned, checksummed
  release libraries (`cargo xtask tts-runtime`; `Contents/Frameworks` on
  macOS, beside the worker on Linux); nothing downloads at build time and a
  version mismatch is refused. The worker also runs Chatterbox (D-35). The
  `sherpa-onnx-sys` crate was rejected because its build script downloads
  the library. sherpa-onnx 2.0.0 contains
  no espeak-ng or piper-phonemize, so the package ships no GPL data; the
  worker turns text into phonemes itself (the model's lexicon plus number
  and abbreviation rules) and passes phoneme codepoints, so words outside
  the lexicon are still spoken instead of silently dropped. The model is
  Kokoro-82M int8 (four pinned files, catalog kind `voice`, stored under
  `models/tts/`), v1.0 int8 pinned to revision
  `57adb37fc0e8d4bd430cfc1b8f8c2294945b9af2`, an export that keeps the STFT
  convolutions in float32 (the older export gave invalid samples). Synthesis
  keeps the model's pauses (`silence_scale` 1.0). A voice model counts as
  installed only when its stamp matches the current catalog size and hash.
  Mixed voices are D-39. It runs on the CPU only: the CoreML provider aborts the
  process on this model's dynamic shapes (an uncatchable C++ exception from
  ONNX Runtime). Measured real-time factors: about 0.5 on an Apple M4 Pro,
  0.44 to 0.46 on four vCPUs of a Linux arm64 container. The library is
  never dlclosed, because ONNX Runtime's exit-time destructors raced the
  unload on Linux (a SIGSEGV after an otherwise clean shutdown).

- **D-29 Linux window look.** The Linux window looks like the macOS one when
  a compositor draws the screen: an owner of `_NET_WM_CM_S<screen>` on X11,
  or any Wayland session. Then the window is transparent (GPUI always takes
  the ARGB visual on X11 and switches transparency at run time), with 10 px
  corners, a hairline edge, the translucent sidebar (`sidebar-blur`, the
  sidebar color at 80%), and `_KDE_NET_WM_BLUR_BEHIND_REGION` on the sidebar
  rectangle so KWin blurs it; picom blurs from its own configuration. Without
  a compositor a transparent pixel renders black, so the window stays square
  and opaque with the solid sidebar, and the flow bar is a rectangle. The app
  reads the selection owner at start and once a second, and switches live
  (`window_chrome::follow_compositor`).

- **D-30 Model catalog and details.** The catalog is schema 2: every model
  has a kind (`speech`, `cleanup`, `voice`), a description, speed and
  accuracy from 1 to 5, an approximate RAM need, what it is good for, and a
  `https://huggingface.co/` source page. A schema 1 catalog (no kind, no
  details) still parses, and its models take the kind of their list.
  Only models whose exact size and SHA-256 are verified against their pinned
  Hugging Face files are listed. The speech list is fixed by D-40. The Models
  page groups the models by kind and opens a details sheet from each row.
  Before a download the app reads the free space of the downloads folder
  (statvfs) and refuses
  when it is less than the bytes still to fetch plus 10%. At start it deletes
  partial downloads older than 7 days. A file entry may carry
  `replaces: [{bytes, sha256}]`, earlier pins of the same file; files that
  match one are updated in the background instead of failing verification.
- **D-31 Manual view.** Manual, third in the sidebar after History, is one
  place for audio files and text that the user starts by hand, with a
  segmented control for Transcribe (the import queue) and Read aloud. "Files"
  stays its code name (the `files` module, the `files.*` settings, the
  `sidebar.files` hook id), and "Import" the code name of one transcribed
  file (the history kind, the `import.*` hook ids); its History line says
  "File" where a dictation names the app. A drop anywhere on the
  main window opens Manual: when every dropped file is `.txt` or `.md` the
  first opens in Read aloud, else all go to the queue, which refuses what is
  not a regular file without opening it. The engine child reads the length
  from the headers and sends it as `audio_ms` on the first `progress` event
  of the import, so the UI process never parses an outside file (D-23) and
  adding a 12 hour file never blocks it; the time left is the pace so far,
  shown from 5% on. The import language (`files.language`) is separate from
  the dictation language. Translate is refused for Parakeet, English-only
  Whisper models, and Turbo models, which have no translation task. Read
  aloud streams: each chunk is appended to the player as it arrives and the
  first plays while the rest is spoken. Export Audio writes the samples the
  player holds as a 16-bit WAV, the only format for now. The settings keys
  are `files.mode`, `files.language`, `files.translate`, `readAloud.engine`,
  `readAloud.voice.system`, `readAloud.voice.kokoro`,
  `readAloud.voice.chatterbox` (both Chatterbox models),
  `readAloud.defaultVoice` (a cloned voice id, empty for the voice of the
  model), `readAloud.speed`
  (0.5 to 2, in steps of 0.05), and `readAloud.format`.
- **D-32 Focus and control edges.** A focused text field has a 1px accent
  border and no glow (the kit theme's `focus_ring` is off). Buttons, pop-up
  buttons, and rows keep the 2px accent ring inside their bounds. A switch or
  a slider draws its ring outside its track and is offset by that ring, so
  its track ends on the same line as the buttons above and below. Shortcut
  fields have one fixed width; the default hands-free reads `⌥⇥` (Option+Tab)
  on macOS and `Right Alt ×2 or Right Alt+Space` on Linux. Home picks the
  microphone with one pop-up row like Language, shared with Settings
  through `Mic::choices`, and shows the level meter only while a test runs.
- **D-33 Pop-up menus.** Every choice of fixed values is a pop-up button
  (`ui::pop_up`). Its menu (`ui::Menu`) is drawn deferred by the kit
  `Popup`, under the right end of the button, so the `overflow_hidden` of a
  group never cuts it, and the kit moves it inside the window. The list shows
  at most `MENU_OPTIONS_SHOWN` (8) whole 24px options, scrolls past them, and
  opens with the chosen one in view. The shared builder owns the keys and the
  closing: Up and Down move the focus and scroll with it, Enter picks, Esc
  closes, a press outside closes and goes no further, and the menu closes when
  the focus goes anywhere else (Tab runs as an action before any key
  listener). Each way gives the focus back to the button. A view keeps only
  the open state and the close callback. The menu fill and its shadow are the
  `menu` and `shadow` tokens; the kit popover color, which tooltips use, is
  the menu color.
- **D-34 Shortcut keys and conflicts.** Settings > Shortcuts has five global slots
  (hold, hands-free, paste last, Command Mode, read aloud); the footer says how
  to change one: "Click a shortcut, then press the keys. Esc cancels." Each
  row has a reset button (only while the slot is not its default), the field,
  and a switch. Hands-free defaults to Option+Tab on macOS (Left Option: Right
  Option is the hold key). Linux keeps the built-in gestures (a double tap of
  the hold key, or the hold key with Space), because Alt+Tab switches windows
  on every Linux desktop. Settings schema 4 replaces the stored gestures text
  of an older file with the new default once, unless another shortcut holds
  Option+Tab; the gestures stay a choice. A slot that is off keeps its keys
  but is not live (`shortcut.enabled` maps a slot key to `false`); with hold
  off, the hold key still starts the built-in gestures when hands-free uses
  them. Read aloud (Ctrl+Cmd+S, Ctrl+Alt+S on Linux) reads the selection like
  Command Mode and plays it with the Read aloud voice; pressed while it plays,
  it stops, and with nothing selected the flow bar says "Select text first".
  Hushpen keeps no list of allowed chords: any keys the system reports, held
  together, are a shortcut. That covers a bare key (a letter, F13, Caps Lock),
  a media key (Play/Pause, Volume Up), a keypad key, a key with no name (saved
  as `Key<number>`: the macOS key code, or the X11 keysym, named by X11), a
  plain modifier, both sides of one modifier (Left Ctrl+Right Ctrl), and up
  to 6 keys. Esc alone closes the recorder; with other keys down it is part
  of the shortcut. Hushpen refuses, and keeps the old one, only a chord that
  another slot has (the message names the slot) or setting text that names no
  key. A chord that something else runs now (an X11 passive grab by another
  client, or a macOS system shortcut that is on, read with
  `CopySymbolicHotKeys`) is saved but not live: the field shows who uses it
  (on macOS the System Settings name, found with `CGSGetSymbolicHotKeyValue`)
  and one line of steps, with Open Settings (System Settings > Keyboard on
  macOS, the desktop's shortcut settings on Linux). The slot is tried again
  every 2 s and goes live once it is free. The macOS app switcher has no
  switch in System Settings and runs before any tap, so while a live shortcut
  is Cmd+Tab or Cmd+Shift+Tab, Hushpen turns that hotkey off for the login
  session (`CGSSetSymbolicHotKeyEnabled`, as AltTab does) and turns it on
  again when the shortcut changes or the app quits. While it is off, a child
  `hushpen switcher-guard` in its own process group turns it on when the app
  is killed or crashes (it reads the hotkeys on a pipe and acts when the pipe
  closes); a note in the temporary folder lets the next start undo it if the
  guard died too. macOS keeps no list of
  other apps' shortcuts, so those cannot be checked. The key that completes a
  live shortcut, and every key but a modifier while the recorder is open,
  stay away from the front app: on macOS a filtering event tap drops them,
  media keys included (with Accessibility; with only Input Monitoring the
  tap listens), on X11 each live chord with one key is grabbed and the
  recorder grabs the keyboard. Modifier events always pass. Caps Lock on
  macOS reports only its press, so it works for a tap shortcut, not a hold.
- **D-35 Chatterbox and the voice library.** Read aloud also speaks with
  Chatterbox Turbo and Chatterbox Nano (catalog kind `voice`, under
  `models/tts/`) in the same `hushpen-tts` worker, one voice model in RAM at
  a time. They run on ONNX Runtime through `ort` with `load-dynamic`,
  opened from the library file sherpa-onnx already links, so the package
  ships no new runtime. Turbo comes from `ResembleAI/chatterbox-turbo-ONNX`,
  whose repo has no default voice clip, so its clip comes from
  `onnx-community/chatterbox-ONNX`. No official Nano ONNX export exists;
  Nano is the community export `Masterx/chatterbox-nano-ONNX`. Both read
  English only: other text fails with `SPEECH_NOT_ENGLISH`, and the UI
  offers the system voice. A voice is cloned from a clip of 5 to 20 s of
  speech (a guided microphone take or a WAV, MP3, or M4A file, the silence
  cut and the level raised, resampled to 24 kHz) kept in
  `voices/<id>/reference.wav` with `voice.json`. The worker computes the
  voice conditioning once and keeps it beside the clip as
  `reference.<model>.cond`; a clip it cannot use fails with
  `SPEECH_VOICE_CLIP`. The default voice (`readAloud.defaultVoice`) is what
  Chatterbox reads with while the Voice menu says Default; empty means the
  voice of the model (`default_voice.wav`). Measured on an Apple M4 Pro with
  8 threads: Turbo has its first audio after about 1.2 s at a real-time
  factor of 0.4 to 0.55 and uses about 650 MB idle, up to 850 MB; Nano about
  0.75 s, 0.27 to 0.37, 470 MB, up to 680 MB.
- **D-36 Read aloud in History.** Each text that Read aloud speaks to the
  end, for Play or for Export, saves one history row of kind `read`: the
  spoken text as the final text, the engine key as `model_id`, the voice
  name as `source_name` (so search finds it), the voice language, the length,
  and `audio/<id>.wav` while the audio retention keeps the audio of a
  finished dictation. Playing or exporting the same speech again adds no
  row; a stopped or failed read adds none. Schema version 2 allows the kind:
  SQLite cannot change a CHECK, so the migration builds the table again with
  the same rowids, with foreign keys off and `foreign_key_check` before the
  commit. Reprocess is refused for a read; Read aloud again speaks the text
  once more with the voice chosen now.

- **D-37 Mute other audio while dictating.** On by default. After the start
  cue has played, Hushpen mutes the default system output (CoreAudio mute,
  or volume 0 when the device has no mute; `wpctl` or `pactl` on Linux) and
  restores it before the finish or cancel cue. It restores only output it
  changed, so output the user had muted stays muted. A marker in the data
  folder records a mute in progress; the next start restores the output and
  removes it, so a crash does not leave the system muted. The microphone is
  never muted.

- **D-38 Idle models.** Settings > Engine > When idle applies to the speech,
  cleanup, and voice workers. Keep in memory leaves the model loaded; on
  macOS it is the default, because macOS compresses and swaps idle memory by
  itself (a kept Whisper base fell from 323 to 178 MiB resident while idle)
  and has no public page-out call. Page out (Linux only, the Linux default)
  calls `madvise(MADV_PAGEOUT)` on the weight mappings after the idle time;
  the model stays loaded and its pages come back from swap. Unload frees the
  model after the idle time. `MADV_FREE_REUSABLE` is never used, because it
  discards the weights. A stored value the system cannot do reads as its
  default.

- **D-39 Kokoro voice mix.** The sherpa C API takes only a speaker id, so
  the worker writes a copy of `voices.bin` (cached in `models/tts/mixes`, at
  most three) whose last speaker holds the weighted average of the two
  voices' [510, 1, 256] style blocks, and loads Kokoro with it. The pinned
  `voices.bin` is only read. A mix travels as the voice id
  `<base>:<100-p>+<with>:<p>`.

- **D-40 Speech models.** Offer only Parakeet v3 Q4_K, Whisper Large v3
  Turbo Q5_0, Whisper Small Q5_1, and Whisper Base Q5_1, in that order.
  Parakeet is the default and the sole Recommended speech model. The owner's
  77 Indian/British English dictations measured 4.14%, 8.44%, 11.01%, and
  11.97% word error rate, respectively. Turbo is for languages outside
  Parakeet's 25 European languages, Small for translation to English, and
  Base for older or slower computers. Downloads use pinned Hugging Face
  revisions, exact sizes, and SHA-256 checks. A retired selection moves to
  Parakeet at startup. Only known retired speech files and their verification
  stamps are deleted; History and other files stay.
- **D-41 Parakeet engine.** The speech child selects Parakeet or Whisper
  behind the existing AsrEngine interface. Both come from the vendored
  whisper.cpp 1.9.4 tree, with Metal on macOS and CPU on Linux. Parakeet
  always detects among its 25 supported languages. It takes no initial
  prompt and cannot translate, so dictionary bias goes to Whisper only.
  Parakeet and Turbo refuse translation in the engine and show a disabled
  Translate to English switch with a tooltip in Manual. Home and Manual
  suggest Whisper Large v3 Turbo Q5 when the selected language is outside
  Parakeet's set. Speech filenames come from the catalog, not from the model
  ID. Speech is decoded in spans of at most 20 seconds, cut near pauses with
  one second of context on each side. The cancel flag is checked between
  spans and during native decoding. The Parakeet C API gives timed subword
  tokens and word starts, but no detected-language code. Hushpen groups those
  tokens into word segments for History, SRT, and VTT, on the original audio
  clock. It reports `und` for the unavailable language code, without
  inventing a detected-language label. NVIDIA's model uses CC-BY-4.0; the
  packaged notices identify NVIDIA, the original model, the quantized
  conversion, and the license.

- **D-42 Word packs.** The Dictionary offers hand-written word packs
  (Indian names, Indian places, Hinglish words, Tech terms) under
  `assets/word-packs/`, all off by default. Pack words never go into the
  user's entries. In the Whisper prompt the user's words come first inside
  the 150-token cap; Parakeet takes no prompt. Only pack words found in the
  text join the cleanup keep-list, and heard-as replacements run after
  cleanup.

- **D-43 Whole-clip dictation.** Dictation transcribes the whole recording
  after stop. Cutting at pauses and transcribing while the user speaks was
  measured on 77 owner dictations and rejected against a 0.3-point WER
  limit: Parakeet v3 Q4 lost 0.29 points (95% CI up to 0.85) to save
  329 ms, and Whisper Turbo Q5 lost 0.36 points to save 937 ms.

- **D-44 Flow bar balance and Recover.** Every flow bar state has a 20 pt
  circle at both ends or at neither, 4 pt in from the edge, and centers its
  text between them. The pill is 28 pt high with radius h/2; on macOS its
  panel is borderless, because a titled panel clips it to 17 pt corners.
  Listening, Transcribing, and Cleaning are 118 wide and idle 67 by 22.
  Every other pill fits its measured line: 4 + 20 + 8 + line + 8 + 20 + 4
  with circles, 12 + line + 12 without, at most 366. The line is the widest
  the pill can show (Undo or Recover on hover too), so hover never resizes
  it. Motion: the size follows a critically damped spring (about 280 ms, no
  overshoot); old content fades out in 100 ms before new content fades in
  over 160 ms; the listening bars become the processing wave in place
  (same 11 bars, 280 ms); marks and circles fade and grow from 0.9 once the
  size settles. Show and hide fade the pill in place over 160 ms with scale
  0.96 to 1, never a shrink to a circle. On macOS the panel has a clear
  background and no shadow, and it is ordered out only after it drew a
  clear frame, so a kept window never flashes old content. Reduce Motion
  crossfades, resizes while the pill is empty, and hides at once. Esc or
  the cross keeps the recording 4 s (`RECOVER_MS`) for Recover, which
  transcribes it again and pastes it as if it were finished. The audio
  goes, or the cancelled row is saved, when the 4 s end or a new dictation
  starts. Command Mode has no Recover.

- **D-45 Models rows and the default model.** The sections are Speech to
  Text, Text cleanup, and Text to Speech; the search also matches the old
  names Speech recognition and Voices. A speech row shows the catalog's
  accuracy and speed as two lines of five 6px dots. There is no status
  column: one quiet word after the name says Default, the download percent,
  Updating N%, Checking, or Failed verification, the only one in a status
  color. Every action is a 22px icon button in a fixed slot: Details,
  Download or Cancel, Set as default (a star, speech only, which replaced
  Use), and Delete. The first speech download that finishes becomes the
  default while no other default is downloaded, and deleting the default
  makes the first other downloaded speech model the default, or leaves none
  chosen. So the default can be deleted (`MODEL_IN_USE` is gone). The
  onboarding model step shows the same four speech rows, and Continue opens
  once one speech model is downloaded.

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
  and integrity level; a target with higher privileges gets a copy and a notice.
  Clipboard restore preserves copyable formats and respects newer copies.
  The flow bar stays topmost without activation. Windows 11 build 22621+
  uses Mica; older builds use an opaque sidebar. Data uses roaming AppData,
  private keys use a current-user-only DACL, and a private named pipe carries
  second-launch requests. A data move clears old files only after process
  exit. System voices use WinRT. Model workers default to CPU with an AVX2
  baseline; Vulkan is an optional build feature, off by default. Page out is
  unavailable. Speech runtime DLLs are pinned by size and SHA-256. Native
  runtime behavior needs Windows hardware checks; packaging is separate.

- **D-48 Permission and capability checks.** Core builds one check list from
  the OS, capabilities, and live facts. Onboarding and Settings share its
  rows, statuses, and fixed actions. macOS keeps Microphone, Accessibility,
  and Input Monitoring. Windows shows microphone consent, administrator-app
  limits, and keyboard-hook status with Retry. Linux explains Wayland and a
  missing tray watcher. Continue requires a microphone on every OS and the
  two key permissions on macOS; other checks do not block it. Unavailable
  keys show no shortcut promise. Paste refusal keeps Copied with a notice.

- **D-49 Windows packages and updates.** Windows x64 builds an unsigned
  per-user NSIS setup and a portable ZIP on `windows-2025`. Setup installs
  under `%LOCALAPPDATA%\Programs\Hushpen` without administrator rights.
  Build-generated icons and version resources use the existing assets;
  the app manifest uses asInvoker and PerMonitorV2. The signed update
  manifest selects setup. After SHA-256 verification, a detached helper
  waits for Hushpen to exit, runs setup with `/S`, and restarts the app.
  Portable installs show a release notice. `install.ps1` checks SHA256SUMS
  before silent setup. Windows files have no Authenticode signature;
  SmartScreen warns. CI packages do not publish a release or the website.

- **D-50 Devices and remote setup.** Devices follows Models in the sidebar.
  Flow D gives speech and cleanup separate device orders. Remote use stays
  off by default. This computer is last unless "Do not load models on this
  computer" is on; then unavailable remote routes refuse the job. Core owns
  routing, health, pairing, fallback notices, and allowed unload rules.
  A route row names the model this computer asks that device to load, since
  devices report none; phones clean up with their system model. Nearby
  phones use mDNS and a one-use, expiring QR offer with Noise key pinning.
  Tailscale uses the same protocol at the supplied address. SSH
  setup reads concrete Host aliases from `~/.ssh/config`, uses BatchMode
  only, verifies SHA-256 before installation, starts the user service, and
  pairs over the authenticated SSH channel. Views send intents and show
  service progress; they perform no network or process work. The desktop
  screen follows the Devices rules of `DESIGN.md`, with Light and Dark
  pixel baselines for the page and both sheets. `mobile/SCREENS.md`
  records the desktop screen state and intent contract.

- **D-51 Shared phone core.** Native phone apps call `hushpen-mobile` through
  pinned UniFFI bindings. Rust owns dictation, model rules, verified downloads,
  pairing, and serving. Parakeet v3 Q4, Whisper Base Q5, and Small Q5 are
  available on every phone; Turbo Q5 follows core's physical RAM policy.
  Cleanup uses the system model first, then S1-mini by Superwhisper or Qwen3.5 2B
  when core's RAM policy and the app's current free memory allow it. Files,
  checksums, prompts, profiles, and output checks match desktop. Failure keeps
  rule-cleaned text. Speech and llama.cpp run on threads over the existing
  bounded worker protocol; the host serves those workers. iOS uses Metal;
  Android uses CPU and NEON. QNN and Core ML acceleration need a later decision.
  The iOS keyboard never loads engines. Build steps live in `mobile/README.md`.

- **D-52 Android app and keyboard.** `mobile/android` uses Kotlin and Compose
  over the generated Rust phone API, with Android 10 as its minimum. Each
  screen has a ViewModel; the repository calls Rust on worker threads.
  The IME records into a session WAV, feeds
  the Rust dictation machine, and inserts through `InputConnection`.
  Password fields refuse dictation. Undo requires the same field, cursor,
  and inserted text. A desktop speech session reserves the shared engine
  until it disconnects. Downloads keep ready choices; speech and cleanup
  selections are separate and persist. Cleanup capability remains paired
  when off or unavailable. Server mode uses `specialUse` for paired desktops'
  speech and cleanup requests, declared in the manifest. A quiet notification
  remains while serving. A Rust job callback
  holds a wake lock only during model work. Memory callbacks and Android's
  low-memory reading feed Rust's unload rule. `MemoryInfo.totalMem` supplies
  core's RAM policy. Cleanup is opt-in. ML Kit needs a ready system model
  and the app in front; a keyboard or foreground service does not remove
  Google's background limit. Local cleanup can run in the background within
  the memory budget; failure keeps rule text. See the
  [ML Kit background limit](https://developers.google.com/ml-kit/genai#background_usage)
  and [specialUse service type](https://developer.android.com/develop/background-work/services/fgs/service-types#special-use).
  Android rejects platform HTTP(S) connections before SDK initialization.
  Its network security configuration also denies cleartext and trusts no
  TLS authorities, blocking SDK metric uploads. Rust owns the
  permitted model downloads and encrypted device links. ML Kit has no
  supported metrics opt-out; see its [data disclosure](https://developers.google.com/ml-kit/android-data-disclosure).

- **D-53 iOS app and keyboard.** `mobile/ios` uses SwiftUI and observable
  screen models over the shared Rust phone API, with iOS 26 as its minimum.
  XcodeGen owns the project. The app and keyboard share
  `group.com.mandalsuraj.hushpen`; only the app links the Rust model engine.
  A keyboard flow session opens `hushpen://dictate?session=...` in the app.
  The app records, transcribes, and cleans text. Insert dictation confirms
  the active field on return because iOS can renew input session IDs.
  Results expire after ten minutes and are used once. Field or cursor changes
  cancel insertion and undo. Shared files are protected and excluded from
  backups. The keyboard writes its Full Access heartbeat to the App Group;
  the app refreshes setup on foreground and checks active input modes.
  Server mode serves only in front, uses core's fixed unload rule, dims the
  screen, and restores brightness and the idle timer on exit. Cleanup is off
  by default and follows D-51. `ProcessInfo.physicalMemory` supplies core's RAM
  policy; the current process budget also limits local cleanup.
  `mobile/SCREENS.md` holds the screen contract.
  iOS advertises through native Bonjour. Other platforms keep `mdns-sd`.
  Native Bonjour avoids the restricted entitlement required for raw multicast;
  see [Apple's local network rules](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy).
