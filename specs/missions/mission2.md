# Mission 2: public-release polish

Goal: make Hushpen look and behave like a finished, native app on macOS and Linux, add file text to speech, and leave a codebase a reviewer respects. The code audit lives at `/Volumes/External1TB/data/_custom/droid/hushpen/plan/audit.md` (agent scratch, file:line references).

## Owner requirements

1. Chrome: the traffic lights, the sidebar app name, and the view title share one vertical center line. Linux draws its lights at the same place as macOS.
2. Separators: a hairline between rows spans the full group width on every page. One rule in `ui`, no per-page insets.
3. Home: the dictation output is centered in its block.
4. Import becomes a native-looking "Files" workspace:
   - a large drop zone (drag and drop anywhere in the window opens it), a file picker, and a queue with progress, cancel, Copy, Export, and Show in History;
   - Transcribe (speech to text), with Translate to English where the model supports it;
   - Read aloud (text to speech) from typed text or a .txt/.md file, with an engine choice: System voice or Kokoro;
   - large files stream: decode runs in chunks, so RAM stays flat for a 5 h file.
5. Text to speech engines: System voice (macOS AVSpeechSynthesizer; Linux speech-dispatcher or espeak-ng when installed, hidden when absent) and Kokoro-82M (download from Models; hardware acceleration where the runtime offers it). The app remembers the engine, voice, speed, and format the user picked.
6. Models: an info button on every row opens details: what it is for, size, languages, speed and accuracy, RAM, license, download source, local path, checksum. More options: more Whisper variants, Kokoro, a recommended badge, a free-disk check, Show in Finder or Files.
7. Settings: one long scrolling list with grouped sections, like macOS System Settings, plus a search field. No tabs.
8. About: an app header (icon, name, version, one-line description), license and copyright, build commit, Check for Updates, and the links.
9. Linux: the same look as macOS: the same chrome geometry and positions, rounded window corners, and a translucent blurred sidebar when a compositor allows it (KDE blur property; plain transparency elsewhere; solid fill with no compositor).
10. Code: organized modules, shared UI builders, no duplicated builders, no stray literals outside `theme`, controller and big panels split along their functions.
11. Tests: unit and component tests for every change; Linux headless checks in Docker (pixel renders and the harness suites); Mac checks only through Codex in the background.
12. Docs: DESIGN.md, specs, README, and the docs site describe the result, with no stale text left.

## Tracks

- A (app UI, `crates/hushpen-app`): A1 shared rows + chrome line + separators + Home; A2 settings list + search; A3 models info + catalog; A4 About; A5 Files workspace UI; A6 controller and history splits.
- B (engines): B1 streaming decode + translate; B2 system voices + `Player::from_samples`; B3 `hushpen-tts` child with Kokoro.
- C (Linux look): C1 ARGB window, rounded corners, blur with compositor detection.

## Rules

- Same safety rules as mission 1 (`mission1.todo.md`, Hard rules): Codex-only Mac control, no grant-dropping checks, game rule, data on the external drive.
- Release builds stay Developer ID signed and notarized (`mission1.todo.md`).

## State

1. Chrome: done.
2. Separators: done.
3. Home: done.
4. Files: done. A real XDND drop (import suite, `slot/xdnd-drag.sh`) queues a WAV and opens a `.txt` in Read aloud with its text. A 5 h file transcribes with a flat engine memory (D-23). A Finder drag on macOS is not driven, because it would move the owner's pointer; the drop path is the same GPUI event.
5. Text to speech: done, within D-25 (no speech-dispatcher: it cannot hand back audio) and D-28 (Kokoro on the CPU only: the CoreML provider aborts on this model).
6. Models: done.
7. Settings: done. Cmd+F on macOS is proven by component tests only, because no synthetic keys are sent on the Mac.
8. About: done.
9. Linux look: done with picom (see-through window, rounded corners, solid without a compositor). KWin blur is set but not seen live: no KDE session is available.
10. Code: done.
11. Tests: done.
12. Docs: done.

Pop-up menus float above the page on every view (D-33), checked on the Mac through Codex in Dark and Light.

Release: v2.1.0 (CI by hand, D-03; macOS files signed and notarized locally). `/Applications/Hushpen.app` runs the release build.
