# Hushpen for iOS: App Store metadata

Paste each field into App Store Connect as written. Limits are Apple's.
Bundle IDs: `com.mandalsuraj.hushpen` (app), `.keyboard`, `.activity`.
Team `GF57JXJF5A`, SKU `hushpen-ios`, version 2.5.0, primary language English (U.S.).

## Rules applied

Guideline 2.3.7 forbids trademarked terms and popular app names in the name,
subtitle, and keywords, and says a subtitle must not reference other apps. So
no other product is named in any field. People who search by another product's
name are reached on the website comparison pages instead. Guideline 2.3.10
forbids other mobile platforms in metadata, so the text never says Android.
Pricing words ("free") stay out of the name, subtitle, and keywords; the
description and promotional text may say them.

## Text fields

Name (30): `Hushpen: Private Dictation` (26)

Subtitle (30): `Voice typing keyboard, offline` (30)

Promotional text (170):

```
Free, open source dictation. Talk in any app and get clean text. Speech recognition runs on your iPhone: no account, no subscription, no cloud.
```

Keywords (100, comma-separated, no spaces after commas): (98)

```
speech,text,transcribe,transcription,whisper,recognition,ai,open source,on-device,parakeet,dictate
```

Apple already indexes the name, subtitle, and category words (private, dictation,
voice, typing, keyboard, offline, productivity, utilities), so they are not repeated.

Description (4000):

```
Hushpen turns your voice into text on your iPhone or iPad. It is free and open source. There is no subscription, no account, no ads, and no word limit.

Add the Hushpen keyboard, tap the microphone key in any app, and talk. Hushpen opens, listens, and turns your speech into clean text on the device. Swipe back to the app you were typing in and keep talking. Your words appear where the cursor is.

PRIVATE BY DESIGN
- Speech recognition runs on your device. Your voice and your text do not leave it.
- Works offline once a speech model is downloaded.
- No account, no analytics, no tracking.
- The source code is public under AGPL-3.0-or-later, so anyone can check these claims.

DICTATE IN ANY APP
- A full keyboard with a microphone key: Messages, Notes, Mail, and other apps that use the system keyboard.
- Hushpen records only after you tap the microphone. The keyboard itself never records and has no network code.
- Undo the last insertion with one tap.
- A Live Activity and the Dynamic Island show that Hushpen is listening while you use another app.

CHOOSE YOUR SPEECH MODEL
- Parakeet v3 (416 MB): English and 24 other European languages. Recommended.
- Whisper Base (60 MB): other languages, and older or slower iPhones.
Each model is checked against a fixed checksum after it downloads. Models live on your device, and you can delete them at any time.

CLEAN TEXT
Hushpen removes filler words, fixes capitals and spacing, and understands spoken punctuation with fast on-device rules. On iPhones that support it, you can also turn on optional cleanup with Apple's on-device system model. It is off until you turn it on.

USE YOUR IPHONE AS A SPEECH SERVER (OPTIONAL)
Hushpen is also a free dictation app for Mac, Windows, and Linux. Turn on Server Mode, and your own computer can use the models on your iPhone or iPad over your local network. You pair with a QR code, and the connection is encrypted. Nothing goes through a cloud server. Server Mode works while Hushpen is open, so keep the phone plugged in.

ABOUT FULL ACCESS
The keyboard asks for Full Access for one reason: to receive the text Hushpen makes and put it in the app you are typing in. The keyboard has no network code. Full Access never sends what you type anywhere.

Hushpen needs iOS 26 or later and 60 to 416 MB of storage for a speech model. Questions, bugs, and ideas: hushpen.mandalsuraj.com/support
Source code: github.com/surajmandalcell/hushpen
```

## Version fields

What's New in 2.5.0 (4000):

```
The first App Store release of Hushpen for iPhone and iPad.

- A dictation keyboard: tap the microphone, talk, go back, and insert clean text in any app.
- Speech models run on your device: Parakeet v3 and Whisper Base.
- A Live Activity and Dynamic Island show that Hushpen is listening.
- Server Mode and encrypted pairing let your own computer use the models on your iPhone or iPad.
- The new app icon, and a Settings page with the open-source licenses.
```

Copyright: `2026 Suraj Mandal`

## URLs

- Support URL: `https://hushpen.mandalsuraj.com/support`
- Marketing URL: `https://hushpen.mandalsuraj.com/`
- Privacy Policy URL: `https://hushpen.mandalsuraj.com/privacy`

## Categories and price

- Primary: Productivity. Secondary: Utilities.
- Price: Free. No in-app purchases. No subscriptions.
- Availability: all territories. Platforms: iPhone and iPad.
- Content rights: the app contains no third-party content. It uses open-source
  libraries and models under their licenses (listed in Settings, Open-source licenses).

## Age rating

Answer every question None or No. The result is 4+.

| Question | Answer |
| --- | --- |
| Parental controls, Age assurance | No |
| Unrestricted web access | No |
| User-generated content | No |
| Messaging and chat | No |
| Advertising | No |
| Profanity or crude humor | None |
| Horror or fear themes | None |
| Alcohol, tobacco, or drug use or references | None |
| Sexual content or nudity, graphic sexual content | None |
| Cartoon, fantasy, or realistic violence | None |
| Guns or other weapons | None |
| Medical or treatment information, health or wellness topics | None, No |
| Simulated gambling, contests, gambling | None, No |

## App Privacy (nutrition label)

Answer: Data Not Collected. Tracking: No.

- No account, analytics, advertising, or crash-reporting SDK is in the app, the keyboard, or the widget.
- Audio, text, and settings stay on the device. The keyboard and the app share dictated text only through the App Group `group.com.mandalsuraj.hushpen`.
- The only internet use is the download of a speech model from huggingface.co when the user taps Download. Hushpen sends no identifier or content in it. Like any file host, the server sees the request address.
- Pairing and Server Mode use the local network only, with Noise encryption.
- The privacy manifest `Shared/PrivacyInfo.xcprivacy` declares no tracking, no collected data types, and the required-reason APIs.
- Export compliance: the app bundles the Noise protocol (ChaCha20-Poly1305, X25519, BLAKE2s), so the Info.plist leaves out `ITSAppUsesNonExemptEncryption` and App Store Connect asks the export compliance questions for each build. The owner answers them.

## App Review information

Sign-in required: No. Demo account: none needed.

Contact: Suraj Mandal, `surajmandalcell@gmail.com`.

Notes (4000):

```
Hushpen is a free, open source dictation app. There is no account or login. All speech recognition runs on the device. Source: https://github.com/surajmandalcell/hushpen

To review the main feature (about 3 minutes):
1. Open Hushpen. On Home, tap Speech Model, then tap the download button for "Whisper Base Q5" (60 MB). Wait until it shows "In use".
2. On Home, tap Microphone, Allow.
3. Open Settings > General > Keyboard > Keyboards > Add New Keyboard > Hushpen. Then tap Hushpen in the list and turn on Allow Full Access.
4. Open Notes or Messages and tap a text field. Hold the globe key and choose Hushpen.
5. Tap the microphone key. The keyboard cannot record, so it opens Hushpen, which shows "Listening" and "Swipe right to go back".
6. Swipe right on the bar at the bottom of the screen (or tap the back-to-app link in the status bar) to return to Notes. Speak a sentence, then tap the check mark in the pill above the keys. The text appears at the cursor by itself. The undo button in the pill removes it.

Why Full Access: the keyboard extension can only read the text that the app made through the App Group container (group.com.mandalsuraj.hushpen) with Full Access. The keyboard has no network code, no microphone access, and no analytics.

Permissions:
- Microphone: records speech after the user taps the microphone key. Audio is deleted after the text is made.
- Camera: scans a QR code to pair the user's own computer (Devices). Optional.
- Local network and Bonjour: Server Mode and pairing with the user's own computer on the same network. Optional. The connection uses Noise encryption.
- Background audio: lets a recording continue after the user returns to the app they were typing in. A Live Activity shows the state.
- Internet: only to download the speech model the user picks, from huggingface.co. The file is checked against a fixed SHA-256 checksum.

Optional features that need other hardware (you can skip them): Server Mode and Devices need the free Hushpen desktop app on a Mac, Windows, or Linux computer. Cleanup with the Apple system model needs a device that supports it and is off by default (Settings > Use System Cleanup).

Encryption: Hushpen uses the Noise protocol (standard ChaCha20-Poly1305, X25519, BLAKE2s) for pairing and Server Mode.
```

## Screenshots

Upload from `appstore/iphone-6.9/` (iPhone 6.9 inch, 1320 by 2868, 7 images) and
`appstore/ipad-13/` (iPad 13 inch, 2064 by 2752, 6 images), in file name order.
Every screen is a real capture from the app on an iOS 27 simulator (iPhone 17 Pro
Max, iPad Pro 13-inch M5), with a caption band added above it. The uncaptioned
captures are in `appstore/raw/`. Icon: `appstore/icon/hushpen-icon-1024.png`
(opaque, no alpha; the build also carries the Icon Composer icon).

## TestFlight

Beta app description: `Private dictation for iPhone and iPad. Add the Hushpen keyboard, tap the microphone, talk, and insert clean text in any app. Speech recognition runs on the device.`

What to test: `Download a speech model, enable the keyboard with Full Access, dictate into Notes and Messages, and try Server Mode with a paired computer. Report problems at hushpen.mandalsuraj.com/support.`

Feedback email: `surajmandalcell@gmail.com`
