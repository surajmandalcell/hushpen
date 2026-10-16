# Linux test harness

Each slot is a Docker container (`hushpen-val-1` to `hushpen-val-6`, linux/arm64) with its own
Xvfb desktop, D-Bus, PulseAudio, and the debug app. Scripts here start slots, drive the app, and
check the result. Everything the harness writes lives under
`/Volumes/External1TB/data/_custom/Droid/hushpen/` (`$HUSHPEN_ROOT`), never in the repo.
`library/user-testing.md` in the mission folder points here.

## The image

`harness/Dockerfile` builds `hushpen-val:noble` from `ubuntu:24.04`:

```
. /Volumes/External1TB/data/_custom/Droid/hushpen/env.sh
docker build --platform linux/arm64 -t hushpen-val:noble -f harness/Dockerfile harness   # services.yaml: harness-image
```

It holds the GPUI build dependencies, Mesa lavapipe (software Vulkan, `llvmpipe`), Xvfb, openbox,
D-Bus, PulseAudio, `xdg-desktop-portal-gtk`, xdotool, xclip, ImageMagick, xinput, xterm,
python3 with GTK3 (the paste target), `iproute2` (`ss`), `strace`, `jq`, and Rust 1.99.0 with
rustfmt and clippy. The Rust toolchain is installed under the name in `rust-toolchain.toml`, so
cargo in a checkout starts with no download. Update both together.

Two choices to know:

- Inter is not installed (`fc-list | grep -ci inter` prints 0). The app embeds its own copy, so the
  font check needs no workaround.
- The openbox `rc.xml` binds Alt+F10 to `ToggleMaximize`, so the fixed-size check can press a
  maximize key. The build fails if the edit does not apply.

## Slot desktop

```
Xvfb :99 1280x800x24 -> dbus-launch -> openbox -> pulseaudio
   null sink "vmic" + virtual source "vmic_src" (default source): the virtual microphone
   null sink "cues" (default sink): cue sounds never reach the microphone
StatusNotifier watcher/host stub (slot/sni-host.py) -> xdg-desktop-portal on demand
hushpen (debug, test-automation)  HUSHPEN_DATA_DIR=/data  HUSHPEN_TESTHOOK_SOCKET=/run/hook.sock
```

Mounts: `harness/` at `/harness` (read-only), `$HUSHPEN_ROOT/harness-bin` at `/app` (the app,
`hookctl`, and `THIRD-PARTY-NOTICES.html` for the About licenses row, from `build-app.sh`), `slots/<n>/{data,out,logs}` at `/data`, `/out`, `/logs`, and the
test assets at `/assets` and `/fixtures` (read-only). A slot has 2 CPUs and 2 GB.

## Slot scripts

Run them from the repo root after sourcing `env.sh`. `<slot>` is 1 to 6.

| Script | Usage |
|---|---|
| `start.sh` | `harness/start.sh [--no-app] [--keep-data] <slot>...` starts the slots, waits until the desktop is ready, and starts the app |
| `stop.sh` | `harness/stop.sh <slot>...` removes `hushpen-val-<slot>` |
| `stop-all.sh` | `harness/stop-all.sh` removes `hushpen-val-1` to `-6` by exact name and fails if any is left |
| `shot.sh` | `harness/shot.sh <slot> [name]` saves a screenshot to `slots/<slot>/out/<run>/<name>.png` and prints the path |
| `xdo.sh` | `harness/xdo.sh <slot> <xdotool args...>` runs xdotool in the slot, for example `xdo.sh 1 key ctrl+v` |
| `hook.sh` | `harness/hook.sh <slot> <hookctl args...>` runs hookctl: `tree`, `state`, `click <id>`, `action`, `wait <path>=<value> [ms]`, `events`, `net`, `feed-wav`, `paths` |
| `play-wav.sh` | `harness/play-wav.sh <slot> <wav> [--lead-ms N] [--device NAME]` pads the WAV with 1 s of silence and plays it into the virtual mic |
| `clip.sh` | `harness/clip.sh <slot> set <text> \| get \| clear \| set-primary <text> \| get-primary` reads and writes the clipboard or PRIMARY |
| `logs.sh` | `harness/logs.sh <slot> [name [-f]]` prints or follows a slot log (`app`, `init`, `xvfb`, `openbox`, `sni-host`, ...); lists the logs without a name |

Also here: `build-app.sh` (builds the debug app and `hookctl` in the `hushpen-build` container and
copies them to `harness-bin`), `run-suite.sh` (below), and `lib.sh` (shared helpers). `slot/` holds
the scripts that run inside a slot.

Set `HUSHPEN_RUN_ID=<name>` to send `shot.sh` and `play-wav.sh` output to a named folder
(`slots/<n>/out/<name>/`); the default is `adhoc`.

## Rules

**Hold keys use the keycode form.** `xdo.sh 1 keydown 108` and `keyup 108` press Right Alt only.
Right Ctrl is 105 and Right Super is 134. The keysym form (`keydown Alt_R`) also presses the left
key (64, 37, 133), so a detector sees a chord. XI2 raw events see XTest input under Xvfb.

**Pad WAVs with 1 s of lead silence.** Capture loses the start of the audio otherwise (the first
word of `speech-short.wav` was lost). `play-wav.sh` does it for you. Compare transcripts with
normalized word recall.

**Concurrency.** At most 3 busy slots at once. A busy slot runs the engine, audio, or a build; the
`hushpen-build` container counts as 2. An LLM slot (4 CPUs, 4 GB) counts as 2, so run it with at
most one other busy slot. Up to 6 slots only for UI-only suites (`smoke`, `env`: no engine work,
about 1 % CPU each). `start.sh` refuses more than 3 slots unless `HUSHPEN_UI_ONLY=1`, and
`run-suite.sh` sets that only for UI-only suites. While slots run, use `-j 4` on the Mac. Never run
6 busy slots while a game is in front.

**Containers.** Touch only `hushpen-val-1` to `-6` and `hushpen-build`, by exact name.

## Suites

```
harness/run-suite.sh smoke --slots 6      # services.yaml: e2e (SUITE=smoke SLOTS=6)
```

`run-suite.sh <suite> --slots N` starts slots 1..N, runs the suite in all of them at once, prints
every check, keeps only the newest run per suite, and removes the slots (also when it fails).
Output per slot: `slots/<n>/out/<suite>-<timestamp>/` with `result.json`, `suite.log`,
screenshots, and the hook tree and state. It exits 0 only when every check in every slot passed.

The run happens in a detached session: `$HUSHPEN_ROOT/runs/<run id>.log` holds the output and
`<run id>.status` (written last) holds the exit code. The command only follows that log, so a
caller that is killed (a tool call that ends kills its process group) does not stop the run or
leave slots behind. `SUITE_TIMEOUT` (seconds, default 3600) bounds a run.

| Suite | UI-only | Checks |
|---|---|---|
| `smoke` | yes | `window maps` (940x600, no frame extents, WM_NORMAL_HINTS min and max 940 by 600, allowing the lock up to 2 s to land after the map), `hint lock restores` (xprop rewrites WM_NORMAL_HINTS with a 16384 maximum and _MOTIF_WM_HINTS with a server frame; the lock writes both back within 1 s), `token colors` (Dark samples from DESIGN.md: content `#1E1E1E`, sidebar `#2B2B2D` to x 231, a neutral selected-row fill, no line in the toolbar band), `window controls and solid sidebar` (only the drawn close and minimize lights, 12 px at 20,20 and 40,20, no third circle, 5 sidebar samples `#2B2B2D`), `every main view keeps 940x600` (all 7 views: display size and hook state flags; the Settings suite does the same for its 7 sections and the onboarding suite for its 5 steps through `chrome_problems` in `slot/lib.sh`), `sidebar tree and hookctl click` (bounds in 0-232, fill and title follow each hookctl click), `sidebar keyboard` (Tab, Home, Down and Enter reach all 7 items, each with the accent focus ring `#0A84FF`), `light appearance` (hook `set-appearance` draws `#F5F5F5` and `#E8E8EA`, then back to the system), `click` (a real mouse click on a sidebar item opens its view), `maximize check` (wmctrl maximize and fullscreen, `xdotool windowsize`, Alt+F10, toolbar double click all keep 940x600) |
| `env` | yes | `tools`, `lavapipe`, `audio routing`, `play-wav padding`, `status notifier host`, `portal`, `xi2 keycode 108`, `keys released`, `paste targets` |
| `mic` | no | `VAL-MIC-001` to `008`: the picker, the level meter, the session WAV (16 kHz mono, speed, offline transcript), recovery after `kill -9`, the saved mic across a restart, fallback when it is gone, no mic, a mic removed while recording, and the hook WAV feed. It empties `/data`, restarts the app, and loads and unloads PulseAudio modules, so it needs its own slot |
| `models` | no | `VAL-MOD-002` to `005` and `008`: real downloads from Hugging Face (progress, cancel, `kill -9` resume, a changed file fails the hash check, delete, switching the active model). Needs outside network, empties `/data`, and downloads about 0.6 GB, so it needs its own slot |
| `engine` | no | `VAL-ENG-011`: starts a fresh app and kills the engine child three times in a row; the gaps must be about 1 s, 5 s, and 30 s, the UI pid must not change, and the hook must answer in every gap. Run it on a fresh app: any engine restart in the last 60 s moves the first gap to 5 s. Takes about 80 s |
| `cleanup` | no | `VAL-CLN-008`, harness half: the filler fixture `filler-dictation.wav` (from `HUSHPEN_TEST_FIXTURES`, mounted at `/fixtures`) is pushed to talk into the GTK entry and arrives with no um, uh, or er; with `cleanup.rules` off the same dictation arrives raw. Transcribes with base, empties `/data`, so it needs its own slot |
| `ai-cleanup` | no | `VAL-AIC-001` to `007`, `009` to `011`, `VAL-LLM-006`, `007`: AI cleanup off by default, the container's model offer, the real Hugging Face download of Qwen3.5 2B with a dictation made during it, self-correction, injected instruction, spoken question, forced timeout, Esc while cleaning, Spanish and 10 runs, kill -9 and kill -STOP of the worker with the restart backoff, idle unload, deleted model. Start it as `VAL_CPUS=4 VAL_MEM=4g harness/run-suite.sh ai-cleanup --slots 1` (an LLM slot counts as 2 busy slots); needs network and takes about 25 minutes. Fixtures `ai-*.wav` in `HUSHPEN_TEST_FIXTURES` |
| `endpoint` | no | `VAL-EP-001` to `009`: the opt-in OpenAI-compatible endpoint against `slot/openai-stub.py` on `127.0.0.1:11434` (its mode is the first line of `/tmp/stub-mode`, its request log is `stub-requests.jsonl` in the run folder). Provider off by default and the Ollama and LM Studio presets, Test connection success, refusal, and a silent endpoint (6 s timeout), cleanup through the stub with the key as a Bearer header, the key only in `secrets.json` with mode 0600 under `umask 000` (changed and removed, never in logs or the hook dumps), endpoint down or 30 s slow falls back to the rule text, the output guards, the strace connect list for a session (only `127.0.0.1:11434`), and plain http to `10.0.0.2` refused before any connect. No model download. Transcribes with base, empties `/data`, and attaches strace to the app, so it needs its own slot. Fixture `filler-dictation.wav` |
| `undo` | no | `VAL-UNDO-001` to `007` and `VAL-CROSS-002`: Undo cleanup after an AI-cleaned insertion. Part one runs against `slot/openai-stub.py` (its answer is the transcript plus ` [stub]`): a real click on the flow bar's Undo cleanup puts the rule text back in the GTK entry (`target-gtk.py` steps back on Ctrl+Z, keeps the live text in `/out/gtk-live.txt` and logs each Ctrl+Z in `/out/gtk-undo.log`) and sets the History row to `undone`, the offer ends after 5 s, a focus change withdraws it, a blocked grab, a timeout, and AI cleanup off never offer it, the History detail undoes without sending a key, and a stock xterm running `cat` never sees a 0x1a byte while History offers Copy original. Part two uses the local Qwen3.5 2B model, a dictionary entry, and `ai-self-correction.wav` to check that raw, rule, AI, and final text stay in History. Start it as `VAL_CPUS=4 VAL_MEM=4g harness/run-suite.sh undo --slots 1` (an LLM slot counts as 2 busy slots). Empties `/data`, so it needs its own slot. Fixtures `filler-dictation.wav`, `ai-self-correction.wav`, and the `llm` folder in `HUSHPEN_TEST_FIXTURES` |
| `command` | no | `VAL-CMD-001` to `006`: Command Mode. The chord (Right Alt, then Right Shift: keycodes 108 and 62) is pressed with xdotool; the instruction is `command-formal.wav` ("make this formal"); the selection is made in the GTK target with Shift+Right, so the entry owns PRIMARY. Against the stub (`slot/openai-stub.py`: default answer is the selection plus ` [edited]`, mode `edited` answers `EDITED TEXT`) the suite checks the replaced middle with prefix and suffix intact and the sentinel restored, the "Select text first" notice with no request and no row, the `command` row (instruction kept, the marker word `zebrafinch` only in `final_text`, not in any log), the "needs AI cleanup or the endpoint" notice with AI off, the request the stub received, and Esc while listening and while cleaning. With the local Qwen3.5 2B model it checks a real edit, then `kill -9` and `kill -STOP` of the worker during cleaning (selection unchanged, error on the bar, sentinel back, next command works). Start it as `VAL_CPUS=4 VAL_MEM=4g harness/run-suite.sh command --slots 1`. Empties `/data`, so it needs its own slot. Fixtures `command-formal.wav` and the `llm` folder in `HUSHPEN_TEST_FIXTURES` |
| `updates` | no | `VAL-UPD-001` to `007` and `009`, and the Linux `xdg-open` half of `VAL-LIC-004`: against a local signed feed (`slot/update-feed-server.py` serves the fixed manifests in `slot/update-feed`, signed with a test minisign key; the app is the debug build, which has the `update-test-feed` feature) the check and automatic install are off by default, no connect and no request while the check is off (60 s under strace), a newer version shows available with the release notes link opened through a stub `xdg-open` and the banner "Download and install", the same version shows up to date, a tampered manifest and one signed by another key end in `UPDATE_SIGNATURE_INVALID`, three starts make one manifest request and Check now one more, no query, cookie, or identifier in a request, the About rows call `xdg-open`, and the release build (`harness-bin/hushpen-release`, built with `HUSHPEN_BUILD_RELEASE=1 harness/build-app.sh`) ignores `HUSHPEN_UPDATE_FEED`. No model, empties `/data`, so it needs its own slot |
| `update-session` | no | `VAL-CROSS-005` and `006`: one scripted session (start, push-to-talk into xterm, a dictation with local AI cleanup, History search, Reprocess, TXT export, import of `speech-short.flac`, every Settings section, About, Quit from the tray) runs twice under `strace -f` from the first instruction. With update checks off it makes zero IP connects; with them on and the same version offered it connects only to the local feed and asks only for the manifest and its signature. Start it as `VAL_CPUS=4 VAL_MEM=4g harness/run-suite.sh update-session --slots 1` (an LLM slot counts as 2 busy slots). Needs the base model, `filler-dictation.wav`, and the `llm` folder; empties `/data`, so it needs its own slot |
| `self-update` | no | `VAL-AUP-001` to `005` (deb and tar.gz) and `VAL-CROSS-008`: release AppImages built with `xtask package --test-feed` (rc.1 and rc.2) and signed manifests from `harness/update-fixtures.sh <rc.1 dist> <rc.2 dist> <out>`, mounted as `/update-test` (the runner sets `HUSHPEN_UPDATE_TEST` to `$HUSHPEN_ROOT/update-test`; the feed server then also serves the rc.2 file). A release build has no hook, so the proof is `hushpen.log`, the files, the feed log, sqlite3, and screenshots; the banner button is pressed with `slot/click-accent.py`. rc.1 updates itself to rc.2 with no click until Restart to update, a wrong sha256 ends in `UPDATE_HASH_MISMATCH` with the old file intact, SIGKILL of `update-apply` during the swap leaves a runnable rc.1, a swap that fails (the staged file removed) logs `UPDATE_SWAP_FAILED`, autoInstall off downloads nothing for 60 s until Download and install, the deb and tar.gz installs show the notice and no download, and 3 History rows, 1 dictionary entry, and a setting survive the update and push-to-talk still works. The rpm (fedora:40) and CI deb (ubuntu:24.04, whose Mesa renders the window; the Mesa of 22.04 draws a blank frame) halves run in `packaging/smoke/update-notice-in-container.sh <update-test dir> <image> <out dir>`. Start it as `VAL_MEM=3g harness/run-suite.sh self-update --slots 1`; empties `/data` and `/opt/hushpen-test`, so it needs its own slot |
| `dictionary` | no | `VAL-DICT-001` to `004`, the hook half of `006`, and `VAL-INS-014`: entries added with the keyboard only survive a restart, empty and repeated phrases are refused, a replacement changes the text in the GTK entry, a stored word reaches the whisper prompt and leaves it when deleted, edit and delete apply to the next dictation, and `dog -> café-ß-🎤` arrives byte for byte while the clipboard sentinel comes back. Transcribes with base, empties `/data`, so it needs its own slot |
| `history` | no | `VAL-HIST-001` to `018`: one complete SQLite row per outcome (pasted, copied only, blocked, cancelled, failed) that survives a restart, the detail view, Copy, Re-paste, Reprocess with another model and the current dictionary (it recovers a failed row), delete with undo and a delete that is not undone, Clear all with confirm, substring and accent-blind search, a 10,000-row paged list (`slot/seed-history.sh`) with search time and UI gaps from the hook, and a read-only database that never loses the text. Transcribes with tiny.en and base.en, empties `/data`, so it needs its own slot |
| `import` | no | `VAL-IMP-001` to `008`, `010`, `VAL-CROSS-003`, `VAL-CROSS-004`, and the X11 flow bar watcher thread count: a WAV picked in the portal file dialog, 6 more formats (`speech-short.aiff` and `speech-short-alac.m4a` come from `/fixtures`), the History row (type Import, segments with times, Reprocess off, search by file name), SRT and JSON export of an import, queue order with one job at a time, cancel of a 10 minute file in under 1 s with the queue going on, Opus, WebM, and MP4 refused with `IMPORT_UNSUPPORTED_CODEC`, corrupt and empty files with `IMPORT_DECODE_FAILED`, `/dev/zero` and a FIFO refused without an `openat` (strace of the app and the engine child), the 10 minute file across 6 windows with base.en, a Spanish file with base, and 20 flow bar hide and show cycles with a flat thread count. Files go in through the hook action `import-add`. Transcribes with base.en and base, empties `/data`, so it needs its own slot; takes about 12 minutes |
| `flowbar` | no | `VAL-BAR-001` to `009`, `VAL-CROSS-010`, `VAL-CROSS-011`: the flow bar never takes the focus (`xdotool getwindowfocus` at map, listening, transcribing, and after the insert), shows each of the six states, moves its waveform with the level, starts and stops a dictation on a real click, sets the language from its picker, sits at the bottom or top center, drags with the real pointer and keeps its place across a restart, stays above an xterm, offers Open history after a no-speech run, and keeps a blocked or Wayland insert for History and Paste last. Transcribes with base and tiny.en, empties `/data`, so it needs its own slot |
| `tray` | no | `VAL-TRAY-001` to `008`, `VAL-CROSS-009`, and the History Re-paste hide: the StatusNotifier item and its dbusmenu (read and clicked with `slot/tray-menu.py`, like a panel does), the live Start/Stop label, tray start and stop with the text in the GTK entry and Paste last transcript, a refused start with no mic, close hides to the tray and Show Hushpen and Settings bring the window back, a second start shows the hidden window, Quit exits 0 with its children, Re-paste hides the window and pastes into the app behind it, and with the watcher stub stopped close minimizes the window. Transcribes with base and tiny.en, empties `/data`, stops the watcher stub in its last check, so it needs its own slot |
| `shortcuts` | no | `VAL-KEY-001` to `006` and `008`: the Settings recorder (opened with a hook click, fed with xdotool keycodes) for the hold key (Right Ctrl), hands-free and paste last chords, and the Command Mode pair; `SHORTCUT_IN_USE` from a chord another X client holds (`slot/grab-chord.py`), `SHORTCUT_RESERVED` for a chord Hushpen uses and for Ctrl+Q; Esc cancels and the live key is blocked while recording; Linux key names with no Mac symbols; Reset to default; and a chord held at start shows the error while the hold key still works. Transcribes with base, empties `/data`, restarts the app, so it needs its own slot |
| `tail` | no | the capture tail: a clip that ends on speech is stopped at once and keeps its last word |
| `core`, `full` | no | `smoke`, `env`, `mic`, `models`, `home`, `tail`, `engine`, and `history`; later features add scripts under `slot/suites/` |

Paste targets (`slot/targets.sh`): an xterm with `cat > /out/xterm.txt` and a translation override
for Ctrl+Shift+V, a stock xterm (`--stock`) that only takes Shift+Insert from PRIMARY, and a GTK
Entry whose Enter writes `/out/gtk.txt`.

Click math: use the bounds from `hookctl tree` plus the absolute origin from `xwininfo`.
`xdotool getwindowgeometry` is 20 px off under openbox.

## Pixel baselines (container only)

`services.yaml` `test-pixel` runs `cargo test -p hushpen-app --features pixel-tests` in
`hushpen-build` with `DISPLAY` unset. GPUI's wgpu headless renderer draws the window offscreen on
lavapipe at scale 2 (1560x1040) and compares it with
`crates/hushpen-app/tests/pixel/baselines/*.png`. On a failure it writes `<view>-actual.png` and
`<view>-diff.png` to `HUSHPEN_PIXEL_OUT` (the container's `TMPDIR`, on the data drive). Updating
baselines is an explicit step: run the test with `HUSHPEN_PIXEL_UPDATE=1` and
`HUSHPEN_PIXEL_BASELINES` pointing at a writable folder, then review and commit the PNGs.

## Mac

Pixel tests and anything that opens a window never run on the Mac. GPUI component tests
(`TestAppContext`, `cargo test -p hushpen-app --lib`) open no window and activate no app there;
run them only when no game is in front (`lsappinfo info -only name "$(lsappinfo front)"`).

`harness/mac-nowindow-proof.sh` is the proof (VAL-FND-033). It refuses to start while a game is in
front (exit 2), runs the lib tests while it reads the frontmost app every 100 ms, and fails if the
frontmost app changes or a new app registers. It only reads; it never activates anything. The log
is `$HUSHPEN_ROOT/evidence/m0-harness/mac-nowindow-proof.log`.

## Cleanup

`harness/stop-all.sh`. A leftover slot is removed by exact name only: `docker rm -f hushpen-val-1`.
