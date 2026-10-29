# Changelog

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
