# Changelog

## 2.4.0

### New

- **Pick a speech model during setup.** The model step shows the four speech
  models, Parakeet first, each with its accuracy and speed. Download the one
  that suits you; Continue opens once one speech model is downloaded, and
  Details tells you more about each model.
- **Install with one command.** `curl -fsSL
  https://hushpen.mandalsuraj.com/install.sh | sh` installs the latest
  release on macOS or Linux. It checks the download against `SHA256SUMS`
  before it installs anything.

### Improved

- Models shows how accurate and how fast each speech model is, as five dots
  each. Every action is an icon button with a tooltip: Details, Download or
  Cancel, Set as default (a star), and Delete. One quiet word after the name
  says Default, the download percent, or Failed verification.
- The Models sections are now Speech to Text, Text cleanup, and Text to
  Speech. The search still finds the old names.
- The first speech model you download becomes the default by itself. You
  can now delete the default model: another downloaded speech model becomes
  the default.
- The Website link in About opens https://hushpen.mandalsuraj.com/.
- The README says in plain words what Hushpen does and how to install it.
- On macOS, hands-free starts and stops with Option+Tab by default, in place
  of a double tap of Right Option or Right Option+Space. If you never changed
  the hands-free shortcut, Option+Tab applies; a shortcut you picked stays,
  and so do the gestures if another shortcut already uses Option+Tab. Linux
  keeps the double tap, because Alt+Tab switches windows there.

### Fixed

- Linux microphone checks reuse one audio-server connection, so repeated
  checks do not exhaust the server's client limit.

## 2.3.3

### Improved

- The cue sounds are half as loud by default: Sound volume starts at 12.5%
  instead of 25%. If you never changed it, the new default applies; a volume
  you picked stays. The slider now moves in 2.5% steps.
- The cue sounds play 1.25x faster, so they are a fifth shorter and a little
  higher. Mute other audio while dictating starts sooner after the start cue.
- Every cue sound fades in and out, so none clicks at its start or end.
- The flow bar fits its text. Pasted, Copied, Cancelled, and notices no
  longer sit in wide pills, and the pill keeps its size under the pointer.
- The flow bar moves calmly between states. Its width eases to the new size,
  the old content fades out before the new one fades in, and the listening
  bars turn into the processing wave where they stand.

### Fixed

- On macOS, a square and a round shape no longer flash as the flow bar
  hides. The bar fades out in place and has no window shadow.

## 2.3.2

### New

- **Recover a cancelled dictation.** After Esc or the cross, the flow bar
  shows Cancelled with a Recover button for 4 seconds. Recover transcribes
  the dictation and pastes it as if you had finished it.

### Improved

- The flow bar has truly round ends, also on macOS, and every state is
  balanced: a round button or mark at both ends or at neither, with the text
  or the wave centered between them. Its sizes changed a little.
- Pasted with Undo paste leads with a soft green done tick.
- The wave is a little shorter at its ends, so it tapers into the round ends.
- The speech engine runs as its own program, `hushpen-asr`, so Activity
  Monitor and `ps` show it by name next to `hushpen-llm` and `hushpen-tts`.

### Fixed

- A switch of the speech model uses less memory. Hushpen now frees the old
  model before it loads the next one, so the two are never in memory at
  once.

## 2.3.1

### Fixed

- The menu bar item now has a name that VoiceOver reads: "Hushpen".

## 2.3.0

### New

- **Parakeet v3 Q4 is the default speech model.** It is fast and accurate for
  English and 24 other European languages. The speech list is now four
  models: Parakeet v3 Q4, Whisper Large v3 Turbo Q5 for Hindi and other
  languages, Whisper Small Q5 for translation to English, and Whisper Base Q5
  for older or slower computers. The English-only and full-size models are
  gone. If you used one of them, Hushpen moves you to Parakeet and deletes
  the old file.
- **S1-mini by Superwhisper for AI cleanup.** It replaces Gemma 4 E2B: a
  484 MB download instead of 3.1 GB, and it answers in a moment. A Gemma
  choice moves to S1-mini, and the Gemma file is deleted to free its disk
  space. Command Mode needs Qwen3.5 2B or an endpoint.
- **Cleanup profiles.** Settings > Cleanup has a Profile pop-up: General,
  Message, Email, and Notes. The Prompt sheet shows exactly what the cleanup
  model gets. With Qwen3.5 2B or an endpoint you can edit the prompt and
  reset it; S1-mini shows its fixed prompt and three choices.
- **Word packs in the Dictionary.** Indian names, Indian places, Hinglish
  words, and tech terms. Turn a pack on and its words count like your own.
  Preview shows the words.
- **Mute other audio while dictating.** Music and videos go quiet while you
  speak and come back when you stop, also after a crash. It is on by default.
- **When idle.** Settings > Engine chooses what happens to a loaded model
  after a quiet time: keep it in memory, page it out (Linux), or unload it.
- **Voices that clone.** Read aloud has Chatterbox Turbo and Chatterbox Nano.
  They read with their own voice or with a voice you record or pick from an
  audio file. Manage Voices keeps your voices: play, rename, delete, and set
  the default.
- **Mix two Kokoro voices.** Mix with and Mix in Read aloud blend a second
  voice into the first, from 10% to 90%.
- **Read aloud in History.** Each text read to the end is kept in History
  with its audio. Read aloud again plays it with the current voice.
- **Audio enhance.** Settings > Audio makes speech clearer for the
  transcript. Auto is on by default. Custom has a tuning sheet with low cut,
  bass, treble, and noise reduction, and Find best settings tests your own
  reading to pick the values with the fewest wrong words.
- **Search in Models and in long menus.** Models has a search field (Cmd+F,
  Ctrl+F on Linux). A pop-up menu with more than eight options, such as
  Language, opens with a search field.
- **A taller window.** Drag the bottom edge to make the window up to 900
  pixels high. Hushpen opens at the same height next time.
- **Follows Reduce Motion.** Views, sheets, menus, and switches now move into
  place. With Reduce Motion on in the system settings, every change shows at
  once, and a change of the setting applies at once.
- **A new app icon:** a statue whispering into a fountain pen. The menu bar
  shows a quill writing a sound wave.

### Improved

- Start a new dictation at once, even while the last one is still being
  transcribed. Texts go in in the order you spoke them.
- Silence is skipped before speech recognition, so long pauses and quiet
  noise give no made-up words.
- Files is now Manual and sits after History. Its Transcribe and Read aloud
  control is in the title bar, and Read aloud has its status, Export Audio,
  and Play in the first row.
- The flow bar comes in faster and looks calmer: a small wave while your
  words are processed, Pasted for one second, and a round Undo paste button
  for four seconds. On macOS it shows at once for each new dictation.
- History details use icon buttons, with Copy beside the facts. Re-paste is
  gone; use Copy.
- The window opens on the view that was open when you quit.
- Every slider shows its value beside the track.
- Input boost now starts at 150% and the sounds at 25%. An older install
  that still has the old values saved (a 100% boost or 50% sounds) moves to
  the new ones once.
- Model details show the measured memory and how the weights are stored.
- Kokoro keeps its natural pauses at punctuation again.
- A model whose files changed in this version, such as Kokoro, updates
  itself in the background. Models shows Updating with the percent instead
  of Failed verification. If the network stops, Hushpen tries again when you
  open Models.

### Fixed

- A long transcript on Home was cut off at both sides. It wraps again.
- A new recording that started right after a stop could lose the
  microphone.
- Kokoro could make broken sound on short text.
- The window could stop for about a second while Read aloud saved the audio
  of a long text.

## 2.2.0

### New

- **Turn a shortcut off.** Each shortcut in Settings has a switch. A shortcut
  that is off keeps its keys for later. With Hold to dictate off, the hold key
  still starts hands-free with a double tap or Hold+Space.
- **Reset one shortcut.** A reset button next to a changed shortcut puts it
  back to its default. Reset to default still resets all of them.
- **Read aloud shortcut.** Select text in any app and press Ctrl+Cmd+S
  (Ctrl+Alt+S on Linux) to hear it with the Read aloud voice. Press it again
  to stop.
- **Input boost.** A slider in the Audio section of Settings makes a quiet
  microphone louder, up to 200%. A change is heard at once, also in the
  microphone test.

### Improved

- The flow bar opens on the screen that has the pointer, so it follows you to
  a second monitor. A position you drag it to applies on every screen.
- On macOS 26 and later the app icon fills its tile like other apps, with no
  gray border around it.
- New sounds for the start, the finish, and the cancel of a dictation.

### Fixed

- Right after a speech or voice model loaded, its status could still say that
  no model was loaded.

## 2.1.0

### New

- **Files.** One workspace for audio files and text. Transcribe: drop audio
  files anywhere on the window (WAV, MP3, FLAC, Ogg Vorbis, M4A, AIFF, up to
  12 hours each) and they run in a queue with the time left. Translate to
  English, then Copy, Export (TXT, SRT, VTT, JSON), Show in History, or Retry.
  Long files stream through the engine with flat memory use.
- **Read aloud.** Type text or open a `.txt` or `.md` file, and hear it with
  the system voice or with the local Kokoro voice. Play, Stop, and Export
  Audio to WAV. A text file dropped on the window opens here.
- **Models.** A details sheet for every model: what it is for, languages,
  speed and accuracy, memory, license, source page, folder, and checksum.
  More Whisper models, a free-disk check before a download, and a Voices
  section for Kokoro.
- **Settings in one list** grouped like System Settings, with a search field
  (Cmd+F on macOS, Ctrl+F on Linux).
- **Shortcuts with any key.** Every key the system reports can be a shortcut:
  media keys such as Play/Pause, the keypad, F1 to F35, Escape with other
  keys, both Ctrl keys together, and chords such as Cmd+Tab or Ctrl+Q.
- **Shortcut conflicts you can fix.** A shortcut that macOS or another app
  already uses is saved, and its field says who uses it ("macOS uses this
  shortcut for "Move focus to next window"."), with one line of steps and an
  Open Settings button. It starts to work as soon as it is free.
- **Cmd+Tab as a shortcut on macOS.** While it is your shortcut, Hushpen turns
  the app switcher off, and turns it on again when you change the shortcut,
  quit, or the app ends any other way.
- **About** has a new header with the version and Check for Updates, and links
  to the website, GitHub, issues, licenses, and the log folder.
- **Linux look.** With a compositor the window matches macOS: rounded
  corners, a see-through sidebar that KWin blurs, and a hairline edge.
  Without one it stays square and solid.

### Improved

- Home groups dictation and the microphone, with a microphone test and level
  meter.
- Pop-up menus float above the page, so no group cuts them off.
- Focus rings, row edges, and separators line up on every view.
- History names the source of a transcribed file "File".

## 2.0.0

The first release of Hushpen in Rust: local dictation with Whisper, rule-based
and optional AI cleanup, Command Mode, history, a personal dictionary, a flow
bar, and signed updates, for macOS on Apple silicon and Linux on X11.
