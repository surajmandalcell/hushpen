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
│   ├── Manual view                 the sidebar view for audio files and text
│   │   ├── drop zone               where audio files are dropped or chosen
│   │   ├── queue                   the imports, one at a time
│   │   │   └── import              one audio file transcribed into a history row
│   │   └── read aloud              text to speech with a speech engine
│   │       ├── voice               one voice of the speech engine
│   │       └── voice library       the cloned voices for Chatterbox
│   ├── updater                     opt-in check and install
│   ├── about                       build stamp, facts, and links
│   └── test hook                   debug builds only
├── hushpen engine (child)          whisper.cpp, started as `hushpen engine`
├── hushpen-llm (child binary)      llama.cpp, one model in RAM
├── hushpen-tts (child binary)      Kokoro and Chatterbox, one voice model in RAM
└── data folder                     all app data on disk (D-13)
    ├── settings                    config/settings.json
    ├── model                       models/whisper/*.bin, models/llm/*.gguf, models/tts/<id>/
    ├── history row                 one entry in history/history.db
    │   ├── audio file              audio/<id>.wav
    │   └── transcript              raw text and cleaned text, or the text read aloud
    ├── cloned voice                voices/<id>/reference.wav and voice.json
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
  Its kind is `dictation`, `import`, `command`, or `read` (a text read
  aloud to the end, D-36).
- **dictionary entry**: a word that goes into the Whisper prompt, or a
  replacement applied after cleanup.
- **model**: a Whisper model (ggml `.bin`), an LLM model (GGUF), or a voice
  model (a folder of pinned files under `models/tts/`, such as
  `kokoro-82m-int8`). Every model file has a pinned sha256 and downloads only
  when the user asks. The catalog (`assets/models.json`) lists every model.
- **model kind**: what a model does: speech (Whisper, turns speech into
  text), cleanup (an LLM, fixes the text), or voice (Kokoro, Chatterbox
  Turbo, or Chatterbox Nano, reads text aloud). The Models page has one
  section for each kind.
- **recommended model**: the one model of a kind that the Models page marks
  "Recommended": the default speech and voice models, and the first cleanup
  model this machine is offered.
- **details sheet**: the sheet that the details button of a model row opens.
  It shows what the model is for, its facts, its source page, its folder,
  and its checksum.
- **partial download**: a `<file>.download` file (`<model id>.<file>.download`
  for a voice model) in the downloads folder that a stopped download leaves
  behind. The next download resumes it. The app
  deletes a partial download older than 7 days at start.
- **flow bar**: the always-on-top pill that never takes focus. It stays dark
  in Light and Dark appearance. It shows only while a dictation is active
  (or with Show when idle on), with a cross to cancel and a tick to finish.
- **Manual view**: the sidebar view for files that the user starts by hand
  (D-31). Its mode is Transcribe or Read aloud, and the app opens it on the
  mode picked last. Files stays its code name (the `files` module, the
  `files.*` settings, the `sidebar.files` hook id), and Import stays the
  code name of one transcribed file (History shows "File").
- **import**: one audio file in the queue of the Manual view, transcribed by
  the engine child into a history row of kind `import`. It is queued,
  running, done, failed, or cancelled.
- **drop zone**: the dashed area at the top of Transcribe. A drop anywhere on
  the main window counts as a drop on it.
- **queue**: the imports of this launch, in the order they were added. One
  runs at a time.
- **read aloud**: text to speech. Typed text or a `.txt` or `.md` file
  (Markdown syntax removed, at most 1 MiB) is spoken in chunks of whole
  sentences, so audio and progress arrive while the rest is still spoken. The
  result can be played, paused, and exported as a WAV.
- **speech engine**: what turns text into speech. The system voice is
  AVSpeechSynthesizer on macOS and espeak-ng on Linux (unavailable when
  espeak-ng is not installed). Kokoro, Chatterbox Turbo, and Chatterbox Nano
  are downloaded models. Chatterbox reads English only, with the voice of a
  clip (D-35).
- **voice**: one voice of a speech engine, with an id, a name, a language,
  and a quality (default, enhanced, or premium).
- **voice library**: the cloned voices, kept in `voices/` and shared by both
  Chatterbox models. The Manage Voices sheet adds, plays, renames, and
  deletes them.
- **cloned voice**: a voice made from a clip of 5 to 20 s of someone
  speaking, recorded in the sheet or taken from an audio file.
- **default voice**: the voice Chatterbox reads with when the Voice menu says
  Default: a cloned voice set as default, or else the voice that comes with
  the model (Built-in).
- **build stamp**: the short commit and its date that `build.rs` compiles
  into `hushpen`. About shows it after the version (D-27).
- **appearance**: the system Light or Dark setting. Views follow it live.
- **settings registry**: the owner of `config/settings.json`. It validates
  values, writes atomically, and keeps unknown keys.
- **pop-up button**: a button that shows the current value of a choice, such
  as the microphone, and opens its **menu**: the options, floating above the
  page, with a check before the chosen one (D-33).
- **settings row**: one line of the Settings list, such as Microphone. The
  row list in `settings/search.rs` gives each its section, key, label, and
  the aliases the search matches.
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
- **hushpen-tts**: the child binary that runs the voice models: Kokoro on
  sherpa-onnx (D-28) and Chatterbox on ONNX Runtime (D-35).
- Children talk to `hushpen` over framed stdin and stdout only.

## History notes

ASR Pro was the name of the Electron app (1.x and the 2.0 Electron mission).
Hushpen replaced it in 2.0.0. The terms bundle, speaker, segment, session,
open target, and error code belonged to that app and are not part of Hushpen.
