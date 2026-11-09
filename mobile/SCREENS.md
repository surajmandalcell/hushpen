# Screen contracts

The desktop Devices flow uses plain placeholders until the designer supplies
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
