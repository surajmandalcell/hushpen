# Hushpen domain glossary

This glossary is the shared vocabulary for the code, the tests, and the
specs. Use these terms in names and in UI copy. When a feature settles a new
term or changes one, update this file in the same change (D-21).

## Naming tree

```
Hushpen
├── hushpen (UI process, GPUI)
│   ├── main window                 940x600, sidebar with 7 views
│   ├── flow bar                    the dark pill that shows dictation states
│   ├── tray                        menu with the same actions
│   ├── dictation                   one hold-to-talk run of the state machine
│   │   └── recording               microphone audio in a session WAV
│   ├── updater                     opt-in check and install
│   └── test hook                   debug builds only
├── hushpen engine (child)          whisper.cpp, started as `hushpen engine`
├── hushpen-llm (child binary)      llama.cpp, one model in RAM
└── data folder                     all app data on disk (D-13)
    ├── settings                    config/settings.json
    ├── model                       models/whisper/*.bin, models/llm/*.gguf
    ├── history row                 one entry in history/history.db
    │   ├── audio file              audio/<id>.wav
    │   └── transcript              raw text and cleaned text
    ├── dictionary entry            a word or a replacement
    └── session WAV                 cache/sessions/<id>.wav
```

## Terms

- **dictation**: one run of the dictation state machine: idle, listening,
  transcribing, cleaning, inserting, then done, cancelled, or failed. A
  dictation starts from the hold key, the flow bar, the tray, or Home.
- **hold key**: the key the user holds to talk (Right Option on macOS, Right
  Alt on Linux by default). Tap and double-tap rules decide hold and
  hands-free mode. Esc cancels.
- **recording**: the microphone audio of one dictation. It streams to a
  session WAV, so an engine crash keeps the audio.
- **transcript**: the text of a history row. The raw engine text and the
  cleaned text are both stored.
- **cleanup**: the step after transcription. Rule-based cleanup (fillers,
  repeats, spoken punctuation, "scratch that") is on by default. AI cleanup
  uses a local LLM or an opt-in endpoint and is off by default.
- **Command Mode**: hold a key, speak an instruction, and the LLM edits the
  selected text.
- **insert**: paste the final text into the focused app. The clipboard is
  saved first and restored after the target app reads it.
- **history row**: one entry in the SQLite history database. Search answers
  in under 200 ms. Audio is deleted after 30 days by default; the text stays.
- **dictionary entry**: a word that goes into the Whisper prompt, or a
  replacement applied after cleanup.
- **model**: a Whisper model (ggml `.bin`) or an LLM model (GGUF). Every
  model has a pinned sha256 and downloads only when the user asks.
- **flow bar**: the always-on-top pill that never takes focus. It stays dark
  in Light and Dark appearance. It shows only while a dictation is active
  (or with Show when idle on), with a cross to cancel and a tick to finish.
- **appearance**: the system Light or Dark setting. Views follow it live.
- **settings registry**: the owner of `config/settings.json`. It validates
  values, writes atomically, and keeps unknown keys.
- **error log**: `logs/hushpen.log`, a local rotating file. It never holds
  transcript text or audio.
- **data folder**: the root of all app data (D-13). The UI shows it as `~/...`
  when it is under the user home.
- **test hook**: the debug-only socket that reports the element tree and the
  state and runs actions. Release builds do not contain it (D-08).

## Process names

- **hushpen**: the UI process. It owns the windows, the tray, the hold key,
  capture, insertion, the store, and the updater.
- **engine**: the child process (`hushpen engine`) that runs whisper.cpp.
- **hushpen-llm**: the child binary that runs llama.cpp (D-06).
- Children talk to `hushpen` over framed stdin and stdout only.

## History notes

ASR Pro was the name of the Electron app (1.x and the 2.0 Electron mission).
Hushpen replaced it in 2.0.0. The terms bundle, speaker, segment, session,
open target, and error code belonged to that app and are not part of Hushpen.
