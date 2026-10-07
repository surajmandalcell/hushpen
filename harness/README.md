# Linux test harness

Each slot is a Docker container (`hushpen-val-1` to `hushpen-val-6`, linux/arm64) with its own
Xvfb desktop, D-Bus, PulseAudio, and the debug app. Scripts here start slots, drive the app, and
check the result. Everything the harness writes lives under
`/Volumes/External1TB/data/_custom/hushpen/` (`$HUSHPEN_ROOT`), never in the repo.
`library/user-testing.md` in the mission folder points here.

## The image

`harness/Dockerfile` builds `hushpen-val:noble` from `ubuntu:24.04`:

```
. /Volumes/External1TB/data/_custom/hushpen/env.sh
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

Mounts: `harness/` at `/harness` (read-only), `$HUSHPEN_ROOT/harness-bin` at `/app` (the app and
`hookctl`, from `build-app.sh`), `slots/<n>/{data,out,logs}` at `/data`, `/out`, `/logs`, and the
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

| Suite | UI-only | Checks |
|---|---|---|
| `smoke` | yes | `window maps` (780x520), `token colors` (DESIGN.md samples), `click` (a real mouse click on a sidebar item opens its view), `maximize check` (wmctrl maximize and fullscreen, `xdotool windowsize`, Alt+F10, toolbar double click all keep 780x520) |
| `env` | yes | `tools`, `lavapipe`, `audio routing`, `play-wav padding`, `status notifier host`, `portal`, `xi2 keycode 108`, `keys released`, `paste targets` |
| `mic` | no | `VAL-MIC-001` to `008`: the picker, the level meter, the session WAV (16 kHz mono, speed, offline transcript), recovery after `kill -9`, the saved mic across a restart, fallback when it is gone, no mic, a mic removed while recording, and the hook WAV feed. It empties `/data`, restarts the app, and loads and unloads PulseAudio modules, so it needs its own slot |
| `models` | no | `VAL-MOD-002` to `005` and `008`: real downloads from Hugging Face (progress, cancel, `kill -9` resume, a changed file fails the hash check, delete, switching the active model). Needs outside network, empties `/data`, and downloads about 0.6 GB, so it needs its own slot |
| `core`, `full` | no | `smoke`, `env`, `mic`, and `models`; later features add scripts under `slot/suites/` |

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
