# Screen contracts

The desktop and Android flows use plain placeholders until the designer supplies
the visuals. Core owns the rules; views render state and send intents.

## Devices

- State: remote enabled, local models disabled, separate speech and cleanup
  route rows, and a notice. Each row has device id, name, status, latency,
  and whether it can move up or down.
- Intents: set remote enabled, set local models disabled, move a device in
  one route, probe devices, open Add device, and open device details.
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

- State: Rust catalog rows and status, selected speech model, active download,
  bytes received and total, verification state, and notice.
- Intents: refresh, download, cancel download, verify, and select.
- Empty: no downloaded model. Both phone speech models remain available.
- Errors: network, space, checksum, busy model operation, or save failure.

## Android Devices and Server mode

- State: server running, port, paired desktop keys, Rust-approved unload rules,
  selected rule and idle minutes, pairing URI, scanner open, and notice.
- Intents: start or stop serving, choose unload rule, change idle minutes,
  scan or enter a desktop offer, pair, forget, and refresh.
- Empty: no paired desktops. Start Server mode and scan a desktop offer.
- Errors: expired or invalid offer, handshake failure, port busy, service
  start refused, unsupported unload rule, or save failure.
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
- Empty: no supported system model. Rule cleanup still works.
- Errors: settings could not be saved. Keep the saved value.
- ML Kit cleanup needs the app in front. Keyboard dictation and background
  serving keep rule-cleaned text when the system model is unavailable.

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

## Phone enrollment contract

The desktop QR contains the existing `hushpen-pair://v1` offer. The phone
first completes the Noise handshake using that offer and pins the desktop
public key. Then it sends one JSON frame over the encrypted stream:

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
