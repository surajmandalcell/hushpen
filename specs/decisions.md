# Hushpen 2.0 decisions

These are the binding decisions for Hushpen 2.0.0. When a feature settles a
new decision, add it here in the same change (D-21).

- **D-01 License.** `AGPL-3.0-or-later` in every crate, the full AGPL v3
  `LICENSE` file, and `THIRD-PARTY-NOTICES.html` inside every package. The
  About "Open-source licenses" row links to it. Every dependency license
  must be compatible with AGPL-3.0-or-later (`deny.toml` allow list).
- **D-02 Version.** 2.2.0. A GitHub release is published only after every
  validation passed and every release file was checked. `CHANGELOG.md` holds
  the notes of each version, and the release text is its section.
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
  and the opt-in OpenAI-compatible endpoint. `xtask net-audit` scans for other calls.
- **D-15 Cleanup.** Rule-based cleanup is on by default and runs offline. AI
  cleanup is opt-in. It uses a local LLM of at most 1.5 GB, or an endpoint
  the user sets. Every machine is offered S1-mini by Superwhisper first
  (English cleanup only) and Qwen3.5 2B second (other languages and Command
  Mode). Command Mode with S1-mini says it needs Qwen3.5 2B. An LLM timeout or a failed output check falls back to the
  rule-based text. The endpoint API key lives in a 0600 file, not in the
  Keychain.
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
  dmg is built on CI only.
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
  partial downloads older than 7 days.
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
  fields have one fixed width; the default hands-free reads `Right Alt ×2 or
  Right Alt+Space` (`Right ⌥ ×2 or Right ⌥+Space` on macOS). Home picks the
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
  and a switch. A slot that is off keeps its keys but is not live
  (`shortcut.enabled` maps a slot key to `false`); with hold off, the hold key
  still starts the built-in hands-free (double tap, Hold+Space). Read aloud
  (Ctrl+Cmd+S, Ctrl+Alt+S on Linux) reads the selection like Command Mode and
  plays it with the Read aloud voice; pressed while it plays, it stops, and
  with nothing selected the flow bar says "Select text first". Hushpen keeps no
  list of allowed chords: any keys the system reports, held together, are a
  shortcut. That covers a bare key (a letter, F13, Caps Lock), a media key
  (Play/Pause, Volume Up), a keypad key, a key with no name (saved as
  `Key<number>`: the macOS key code, or the X11 keysym, named by X11), a
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

## History notes

ASR Pro was the name of the Electron app (1.x, and a 2.0 mission that the
Rust rewrite replaced). Its decisions D-01 to D-40 covered Electron, IPC,
the renderer, Windows, and the old data folders. They no longer apply and
stay in git history.
