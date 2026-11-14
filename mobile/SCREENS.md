# Screen contracts

The desktop Devices screens follow `DESIGN.md`. The iOS screens use the
system look: Liquid Glass, system fonts, SF Symbols, grouped lists. Core owns
the rules; views render state and send intents.

## Needs design

- Phone Models: review the local cleanup choices in the existing download
  rows. Speech and cleanup keep separate selections. The system model is first.
- Phone Settings: the existing cleanup switch enables system or downloaded
  cleanup. No new visual treatment is included.

## Devices

- State: remote enabled, local models disabled, separate speech and cleanup
  route rows, and a notice. Each row has device id, name, model, status,
  latency, and whether it can move up or down.
- Intents: set remote enabled, set local models disabled, move a device in
  one route by one place or to a place (a drag), probe devices, open Add
  device, and open device details.
- Empty: no paired devices; local routes contain This computer while local
  models are allowed. Disabled local routes have no local fallback.
- Errors: settings could not be saved, a device did not answer, or no device
  can serve. Keep saved routes when a probe fails.

## Add device

- State: Phone, SSH, or Tailscale method; discovered devices and selection;
  SSH hosts and selection; address and device OS; pairing URI; QR offer and expiry;
  pairing state; progress rows; busy state; and a notice.
- Intents: choose method, select a discovered device, refresh discovery,
  create or cancel an offer, pair from a URI, set an address and OS, select an SSH
  host, start SSH setup, and close.
- Empty: no nearby phones or no concrete SSH hosts. An address can be entered
  for Tailscale. Discovery never adds or trusts a device by itself.
- Errors: discovery failed, offer expired, pairing failed, invalid address,
  SSH connection failed, unsupported server, verification failed, install
  failed, or pairing could not be saved. Show the failed progress step.

## Device details

- State: selected device id, name and rename draft, kind, OS, transport,
  address, status, latency, allowed unload rules, selected rule, idle
  minutes, and a notice.
- Intents: rename, set an allowed unload rule and idle time, probe, forget,
  and close.
- Empty: the selected device was forgotten; return to Devices.
- Errors: invalid name, unsupported unload rule, invalid idle time, or save
  failure. A failed save keeps the previous device settings.
- Forget removes the device and its route entries. This computer cannot be
  renamed, moved, or forgotten; the local-model switch controls its use.

## Android onboarding

- State: microphone grant, keyboard enabled and selected, phone model rows,
  selected speech model, download bytes, verification state, and notice.
- Intents: request microphone, open input-method settings, show the keyboard
  picker, download or verify a speech model, and select a ready model.
- Empty: no speech model is ready. Show the Rust phone catalog and Download.
- Errors: permission denied, model download or verification failed, or save
  failed. Keep the last saved selection.

## Android Models

- State: Rust catalog rows and status, separate speech and cleanup selections, active download,
  bytes received and total, verification state, and notice.
- Intents: refresh, download, cancel download, verify, and select.
- Empty: no downloaded model. Rust filters the phone catalog by reported RAM.
- Errors: network, space, checksum, busy model operation, or save failure.

## Android Devices and Server mode

- State: server running, port, paired desktops (name and id), Rust-approved
  unload rules, selected rule and idle minutes, scanner and Type Code sheets,
  the Nearby pairing number, and notice.
- Intents: start or stop serving, choose unload rule, change idle minutes,
  scan a QR, type the 8-digit code and an optional address, pair or cancel on
  the Nearby number, forget, and refresh.
- Nearby (D-57): while Devices or Server mode is in front and Server mode is
  on, the phone opens a two-minute pairing window and lists in the computer's
  Nearby list. The computer picks it; both screens show the same 6-digit
  number; Pair on each saves trust. Leaving the screen closes the window.
- Empty: no paired desktops. On your computer, open Add a Device and pick this phone.
- Errors: expired offer, wrong code, no computer found, handshake failure,
  port busy, service start refused, unsupported unload rule, or save failure.
- Closing the activity stops the open-only rule. Other rules keep the
  foreground service. Memory readings and callbacks feed Rust's memory rule.
- Pairing retains cleanup capability while the native system model is off
  or unavailable. Enabling it later does not require pairing again.

## Android scanner

- State: camera permission, scanned code, and notice.
- Intents: request camera permission, bind camera, take a code, stop, and close.
- Empty: permission not granted or no code found. Keep the default permission
  action or camera preview available.
- Errors: camera initialization or barcode decoding failed. Pairing errors
  return from Rust to Devices. The scanner does not parse pairing payloads.

## Android Settings

- State: cleanup enabled, system cleanup available, haptics enabled, and notice.
- Intents: set cleanup and set haptics. Availability refreshes when the activity resumes.
- Empty: no supported system model. Use the selected downloaded cleanup model, or keep rule cleanup.
- Errors: settings could not be saved. Keep the saved value.
- ML Kit cleanup needs the app in front. Local cleanup uses the same Rust
  engine and prompts as desktop when the system model is unavailable.

## Android licenses

- State: the bundled license notice document.
- Intents: open from Settings and return.
- Empty/error: the bundled document is missing or cannot open.
- The default WebView reads packaged assets only. Scripts, remote loads,
  and access to other local files are disabled.

## Android keyboard

- State: Rust dictation phase, shift, symbols, undo available, haptics, and notice.
- Intents: type letters or symbols, shift, space, repeat backspace, enter the
  editor action, tap mic to toggle, hold mic to talk, cancel, recover, and undo.
- Empty: no input connection or no verified model. Dictation cannot insert.
- Errors: microphone denied, capture failed, native speech failed, secure
  field, or changed input target. Never insert into a new field after a switch.
- Speech is busy while a desktop speech session owns the shared engine.
  Stopping Server mode releases that session for local dictation.
- Undo checks the field, cursor, and preceding text before deleting the last
  insertion. All controls remain default buttons until visual design lands.

## iOS tabs

- A floating glass tab bar with four tabs: Home, History, Stats, Settings. It
  shows on every tab, Stats included, and hides on pushed screens (Models,
  Devices, History detail). No central mic button.
- Home, History, and Stats read the phone's own history database. Nothing
  leaves the phone. iOS gives the keyboard no host app, so there are no
  per-app numbers.

## iOS Home

- Cards: Words for this week (bars, change on last week, opens Stats), Time
  saved, Streak, then Recent with the last three dictations and See All.
  A "Finish setup" card (Keyboard, Microphone, Speech model) shows only while
  one of them is missing.
- State: this week's words, time saved, streak, latest dictations, setup, and notice.
- Intents: open Stats or History, request microphone, open system settings.
- Empty: no dictations yet. Recent says they stay on this iPhone.
- iOS does not expose whether this custom keyboard is enabled. The receipt
  records its last use with Full Access; it does not claim current readiness.

## iOS History

- One card per dictation, grouped by day, with search. Under the text one row
  holds the time, "Spoke 7 s · Took 1.4 s · 16 words", and a plain
  copy icon (16 pt glyph, 44 pt touch area). Long stats drop "Took" first, then
  end in an ellipsis; the time and the copy icon never shrink. The text stops
  at 3 lines.
- Intents: search, copy, open a dictation, and load more.
- Empty: no dictations yet, or no match.
- A card opens History detail (a list): Cleaned up or Original, the text and
  its stats, Details (speech model, cleanup, language), and a bar with Share,
  Copy, and Delete.
- The phone saves a row when a keyboard dictation finishes: text, speech
  length, the seconds from the end of speech to the text, model, language, and
  how cleanup went. The audio is never kept.

## iOS Stats

- Day, Week, Month, and Year. A Words card with bars and a callout for the
  picked bar, Time saved (typing at the set speed minus the time spoken),
  Streak with this week's days, and On this iPhone ("43 of 52" in one stacked
  bar, "+N devices" only when the history holds text from other devices).
- Settings holds Typing speed and Keep History (Forever, 1 Year, 3 Months, 1
  Month) and Clear History.

## iOS Models

- Cards. The trailing button is a 28 pt glyph in a 44 pt touch area, centered
  on the name line: Get is `arrow.down.circle`, `checkmark.circle.fill` when
  installed (accent while in use), a progress ring while downloading.
- State: Rust catalog and model status, accuracy and speed ratings, separate speech and cleanup selections, download bytes,
  verification state, and notice.
- Intents: refresh, download, cancel, verify, and select.
- Empty: no downloaded model. Rust filters the phone catalog by reported RAM.
- Errors: network, space, checksum, busy engine, or save failure.

## iOS Devices

- A list: Server Mode with its state, the Power row (title, "Charging" or "On
  battery", then the battery icon and percent), paired desktops, and Pair a Desktop.
- State: paired desktops (name and id), scanner open, the Nearby pairing
  number, server availability, battery, busy state, and notice.
- Intents: scan a QR or type the 8-digit code, pair or cancel on the Nearby
  number, forget (after a confirmation), refresh, show the Server mode
  screen, and turn Server mode on or off. Pairing needs Server mode on.
  Turning it on here opens no card, so the list stays usable.
- Nearby (D-57): with Server mode on, this screen and the Server screen keep
  the pairing window open and show a sheet with the 6-digit number, Pair, and
  Cancel when a computer picks this iPhone. Closing the screen closes the window.
- Empty: no paired desktop. On your computer, open Add a Device and pick this iPhone.
- Errors: camera unavailable, wrong or expired code, no computer found,
  handshake failure, Local Network permission denied, or save failure.

## iOS Server mode

- State: running or starting, listening port, selected speech model, cleanup
  mode, jobs served since it started, paired desktops, battery, and notice.
- Intents: start, stop, close, and refresh. The unload rule is While server mode is open.
- A list on black: a status header, Speech model, Cleanup, Port, Served, and
  the Power row (battery icon, then the percent). The footer says "This iPhone
  can keep serving." while charging, else that it keeps running on battery.
  There is no "Plug in to keep serving" screen.
- Errors: port busy, model unavailable, or host startup failed.
- Serving disables the idle timer and dims the screen. Leaving the foreground
  stops serving and restores the previous brightness and idle-timer state.
- Devices opens this full-screen, always dark screen. Stop closes it. After an
  interrupted stop the screen stays with the notice and a Close button.

## iOS scanner

- State: camera permission, starting, scanning, scanned code, and notice.
- Intents: request permission, start or retry, take a code, stop, open settings,
  open Type Code, and close.
- Empty: no code or no camera. Type Code takes the computer's 8-digit code
  (PAKE) and an Address row, Automatic by default (Bonjour), or host:port.
- Errors: permission denied, camera unavailable, or capture could not start.
  Pairing payload validation stays in Rust.
- A sheet over Devices. A scanned QR pairs at once; there is no link or paste field.

## iOS Settings

- Cards (grouped rows): Keyboard (Microphone, Keyboard, Full Access, Open
  Settings, Left-handed keyboard), Models and Devices, Cleanup, History (Keep History, Clear
  History), Stats (Typing speed), and About.
- State: cleanup enabled, Foundation Models availability, microphone permission,
  keyboard use receipt, typing speed, history retention, and notice.
- Intents: set cleanup, refresh permissions, open settings, set typing speed
  and retention, clear history, and open bundled licenses.
- Empty: no supported system model. Use the selected downloaded cleanup model, or keep rule cleanup.
- Errors: save or native cleanup failure. Keep the saved setting and rule-cleaned text.
- Cleanup stays off by default.

## iOS licenses

- State: the bundled license notice document.
- Intents: open from Settings and return.
- Empty/error: the bundled document is missing or cannot open.
- The WebView reads bundled content only, with scripts and remote navigation disabled.

## iOS dictation

- State: active flow session, Rust dictation phase, transcript, and notice.
- Intents: accept a session URL, record, finish, cancel, and recover.
- Empty: no current keyboard request. Do not record from an arbitrary URL.
- Errors: microphone denied, missing model, capture failure, expired session,
  or speech failure. Keep errors visible and do not insert stale results.
- Recording and model work run in the app. The final text goes to the App Group.
  The user returns to the original app and the keyboard inserts the text.
- The full-screen page, while it listens, says "You're all set. Head back to
  keep typing." over a two second swipe demo, with a quiet ring and an
  arrow at the system back link. The page names no app: iOS draws the link.
  It shows no "Listening" text, animation, or buttons: Cancel and Done live in
  the keyboard strip. Other states keep the
  wave, state, and Cancel, Recover, or Close. Dismissing it cancels the session.

## iOS Live Activity

- State: dictation phase (listening, transcribing, cleaning) and the start time.
- Shown from the first busy phase until the dictation ends. Compact, minimal,
  expanded, and lock screen views show the wave and, while listening, a
  system-drawn timer. The app never updates it for the clock.

## iOS keyboard

- State: Full Access, flow session, strip state, notice, and the emoji panel.
- Intents: start a session, open its app URL, finish or cancel it, undo the
  insert, copy or dismiss a failed insert, type, delete, space, period, return,
  open and close the emoji panel, and switch keyboards.
- Empty: no Full Access or no result. Show the required system setup.
- Errors: session expired, shared file unavailable, or an insert the field
  refused. Never replay a consumed result.
- Finished text goes in by itself at the cursor the field has at that moment.
  Typing, moving the cursor, or changing the field while the app transcribes
  does not cancel it. The session is consumed before the insert.
- iOS reports no result for an insert, so the keyboard checks that the text
  before the cursor changed. If not, it copies the text and the strip says
  "Paste failed. Copied to clipboard."; if the copy fails, "Paste failed." with
  a Copy button.
- The extension links no Rust library and loads no model or microphone.
- The system keyboard look (letters; bottom row 123, emoji, space, period,
  return; the system draws the globe). A fixed 44 pt strip above the 216 pt of
  keys holds the dictation states, so nothing moves. At rest it shows Undo and
  Redo of the keyboard's own edits on the left (by word, dimmed when empty) and
  the hint (mic.fill and "Tap to dictate") on the right, one tap target that
  starts dictation. The Settings switch "Left-handed keyboard" (App Group
  setting, default off) mirrors the strip. Keys work while the app listens or
  transcribes. Listening: a gray glass Cancel button, the elapsed time and a
  blue curve that follows the microphone level (the app sends it to the
  keyboard), a blue Finish button. Transcribing: a low steady curve and the word.
  Inserted: a blue check and the word, with Undo as a round button on the right
  (4 seconds). The two paste failures and other notices show gray text in the
  middle, with Dismiss or Copy as a round button on the right. The parts of a
  new state fade in and rise 4 pt one after another (0.18 s each, 0.05 s apart);
  Reduce Motion swaps them at once. The emoji key swaps the letters for a
  scrolling grid of common emoji, with ABC and delete below it.
- Keys feel like the system keyboard: no fade or animation. A letter shows the
  callout at touch-down and types at touch-up, on the key under the finger.
  Delete types at touch-down and repeats after 0.5 s, then faster. Each key
  plays the system click (Keyboard Clicks setting) and, with Full Access, a
  light haptic. Several fingers work at once. The key area is a UIKit touch
  layer over the SwiftUI keys. `hasDictationKey` is set in `viewDidAppear`;
  earlier, iOS keeps its own mic below the keyboard.

## Phone enrollment contract

A phone enrolls after pairing: a scanned QR (version 2 JSON), a typed
8-digit code, or a Nearby number comparison where the computer connects to
the phone. The phone saves the computer's key and its name from the protected
handshake (a Nearby computer sends no name, so the phone shows "Desktop" and
the key prefix). After a scan or a typed code the phone sends one JSON frame
over the encrypted stream; Nearby needs none, because the computer already
has the phone's address from discovery:

```json
{"type":"enroll","name":"Phone","os":"android","port":7342,"speech":true,"cleanup":true}
```

Use `os` equal to `android` or `ios`. Both capability flags are required.
The name must be nonempty, at most 128 bytes, and contain no control
characters. The listening port must be between 1 and 65535. The frame uses
the worker protocol: a little-endian `u32` payload length, JSON kind byte
`0`, then UTF-8 JSON. Enrollment payloads are at most 1024 bytes and must
arrive within three seconds after the handshake.

The desktop derives the device id from the authenticated phone public key
as lowercase hexadecimal. It combines the connection's peer IP with the
supplied port for the serving address. An enrollment cannot choose its own
id or IP address. iOS receives the fixed While server mode is open unload
rule. This contract does not implement a mobile app.
