---
version: alpha
name: Hushpen Desktop
description: Native macOS-style desktop design system for private speech recognition and transcription, with Light and Dark appearances.
colors:
  dark-content: "#1E1E1E"
  dark-sidebar: "#2B2B2D"
  dark-sidebar-blur: "#2B2B2DCC"
  dark-group: "#FFFFFF0D"
  dark-selection: "#FFFFFF1A"
  dark-control: "#FFFFFF1F"
  dark-label: "#FFFFFFD9"
  dark-label-secondary: "#FFFFFF8C"
  dark-label-tertiary: "#FFFFFF40"
  dark-separator: "#FFFFFF1A"
  dark-accent: "#0A84FF"
  dark-success: "#30D158"
  dark-warning: "#FF9F0A"
  dark-error: "#FF453A"
  dark-scrim: "#00000066"
  dark-segment: "#FFFFFF38"
  dark-menu: "#2B2B2D"
  dark-shadow: "#00000080"
  light-content: "#F5F5F5"
  light-sidebar: "#E8E8EA"
  light-sidebar-blur: "#E8E8EACC"
  light-group: "#00000009"
  light-selection: "#00000017"
  light-control: "#00000014"
  light-label: "#000000D9"
  light-label-secondary: "#00000080"
  light-label-tertiary: "#00000042"
  light-separator: "#0000001A"
  light-accent: "#007AFF"
  light-success: "#34C759"
  light-warning: "#FF9500"
  light-error: "#FF3B30"
  light-scrim: "#00000033"
  light-segment: "#FFFFFF"
  light-menu: "#FFFFFF"
  light-shadow: "#00000033"
  on-accent: "#FFFFFF"
  window-close: "#FF5F57"
  window-close-glyph: "#6E140F"
  window-minimize: "#FEBC2E"
  window-minimize-glyph: "#8F5B00"
typography:
  title:
    fontFamily: ".SystemUIFont on macOS, Inter on Linux"
    fontSize: 22px
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: 0em
  toolbar-title:
    fontFamily: ".SystemUIFont on macOS, Inter on Linux"
    fontSize: 13px
    fontWeight: 600
    lineHeight: 1
    letterSpacing: 0em
  heading:
    fontFamily: ".SystemUIFont on macOS, Inter on Linux"
    fontSize: 13px
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: 0em
  body:
    fontFamily: ".SystemUIFont on macOS, Inter on Linux"
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.3
    letterSpacing: 0em
  button:
    fontFamily: ".SystemUIFont on macOS, Inter on Linux"
    fontSize: 13px
    fontWeight: 500
    lineHeight: 1
    letterSpacing: 0em
  secondary:
    fontFamily: ".SystemUIFont on macOS, Inter on Linux"
    fontSize: 11px
    fontWeight: 400
    lineHeight: 1.3
    letterSpacing: 0em
rounded:
  none: 0px
  button: 5px
  selection: 6px
  group: 8px
  app-icon: 16px
  window: 10px
  full: 9999px
spacing:
  hairline: 0.5px
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 20px
  "2xl": 24px
  "3xl": 32px
  sidebar-width: 232px
  sidebar-top: 48px
  toolbar-height: 52px
  onboarding-width: 640px
  window-width: 940px
  window-height: 600px
  sheet-width: 460px
components:
  app-window:
    backgroundColor: "{colors.dark-content}"
    textColor: "{colors.dark-label}"
    width: "{spacing.window-width}"
    height: "{spacing.window-height}"
  sidebar:
    backgroundColor: "{colors.dark-sidebar}"
    width: "{spacing.sidebar-width}"
  sidebar-row:
    typography: "{typography.body}"
    rounded: "{rounded.selection}"
    height: 28px
    padding: 8px
  sidebar-row-selected:
    backgroundColor: "{colors.dark-selection}"
    textColor: "{colors.dark-label}"
  sidebar-icon:
    size: 16px
    textColor: "{colors.dark-label-secondary}"
  group:
    backgroundColor: "{colors.dark-group}"
    borderColor: "{colors.dark-separator}"
    rounded: "{rounded.group}"
  row:
    typography: "{typography.body}"
    minHeight: 36px
    padding: 10px
  button:
    typography: "{typography.button}"
    backgroundColor: "{colors.dark-control}"
    textColor: "{colors.dark-label}"
    rounded: "{rounded.button}"
    height: 22px
    padding: 12px
  button-default:
    backgroundColor: "{colors.dark-accent}"
    textColor: "{colors.on-accent}"
  text-input:
    backgroundColor: "{colors.dark-control}"
    textColor: "{colors.dark-label}"
    rounded: "{rounded.button}"
    height: 22px
  focus-ring:
    borderColor: "{colors.dark-accent}"
    size: 2px
  menu:
    backgroundColor: "{colors.dark-menu}"
    borderColor: "{colors.dark-separator}"
    rounded: "{rounded.group}"
    padding: 4px
    minWidth: 180px
    maxWidth: 320px
    maxHeight: 192px
  menu-row:
    typography: "{typography.body}"
    rounded: "{rounded.button}"
    height: 24px
  traffic-light-close:
    backgroundColor: "{colors.window-close}"
    textColor: "{colors.window-close-glyph}"
    size: 12px
    rounded: "{rounded.full}"
  traffic-light-minimize:
    backgroundColor: "{colors.window-minimize}"
    textColor: "{colors.window-minimize-glyph}"
    size: 12px
    rounded: "{rounded.full}"
---

# Hushpen Design System

## Overview

Hushpen is compact and fixed-width: private dictation, transcription, models, and playback. Open on the working app. Hide internal names.

Core sets platform defaults. Windows uses Right Alt gestures, CPU, Keep in memory, Inter, and custom window controls. Only Linux offers Page out. Windows 11 build 22621+ uses Mica behind the sidebar; older builds use opaque fill. The flow bar never takes focus. Higher-privilege apps get a copy and notice.

## App Icon

The icon outlines a Roman statue: a young woman in profile, eyes closed, whispering into an upright fountain pen below her lips. Ink (`#1B2228`) on marble (`#F7F4EF` to `#E8E3DA`, top to bottom), without text or other colors.

Each design in `assets/icons/<name>/` has `mark.svg` (full 1024 tile) and `mark-small.svg` (bold head and pen crop). `classic` ships; `sari` (braided bun, jhumka, sari pallu) and `waves` (long loose hair) are seasonal. `harness/icons.sh <name>` builds all icon files:

- `assets/hushpen-app-icon.svg` and `.png`: the drawing on an 824px squircle at 100px on the 1024 canvas (the macOS grid), with the template shadow (10px down, 30% black).
- `assets/hushpen-app-icon.icns` and `assets/linux-icons/*.png`: 128px and up show the full drawing, 48 and 64px the bold crop, and 16 to 32px the solid head and pen of `assets/icons/mark-tiny.svg`, because finer lines turn gray there.
- `assets/hushpen-app-icon.icon` and `assets/hushpen-app-icon.car`: the Icon Composer source and its compiled catalog, shipped as `Assets.car` with `CFBundleIconName`. macOS 26 and later draw it in the Dock and Finder, and the iOS app embeds the same `.icon` for the Home Screen. In the Dark icon style the system draws a dark tile and the lines turn light.
- `assets/hushpen-app-icon-ios.png`: the same art edge to edge on 1024px, opaque and square, for the App Store and the press kit.
- `assets/hushpen-app-icon-192.png`: About draws the tile without its margin at 96px, at 2x.
- The menu bar and Linux tray glyphs, from `assets/hushpen-menubar.svg`.

## Appearance

Views share one palette and follow system Light or Dark live; unknown appearance uses Dark. Reduce Motion follows macOS Accessibility or GNOME `enable-animations` off, read at start and once a second. Unknown motion settings allow motion.

The flow bar pill is the one exception on desktop. It stays a dark HUD (`#1F1F1F`, light text) in both appearances, because it floats over other apps. The iOS keyboard pill follows the appearance of the keyboard instead.

The menu bar shows only a solid quill rising to the top right, writing two 1.5pt waves at bottom left. `assets/hushpen-menubar.png` is a 22x16pt template at 2x, tinted by macOS for appearance and highlight. Its accessible name is "Hushpen". Linux uses the same glyph in a 32px square, black or white for its appearance.

## Colors

Use the Light and Dark tokens above: neutral grays and one accent.

- **Content:** opaque content and onboarding.
- **Sidebar:** native unfilled material on macOS; solid on Linux without a compositor.
- **Sidebar blur:** translucent Linux sidebar; KWin blurs it, picom uses its own blur setting.
- **Group:** grouped section fill.
- **Selection:** neutral hover, sidebar selection, and list selection; never blue.
- **Control:** push button fill.
- **Label:** main text.
- **Label secondary:** secondary lines, sidebar icons, and group footers.
- **Label tertiary:** placeholders and disabled text.
- **Separator:** hairlines and group stroke.
- **Segment:** selected segment, raised above the control fill.
- **Accent:** default button, on switch, focus rings, selection focus, links, and dragged-over drop zone outline.
- **Success and error:** ready or failed status words only.
- **Warning:** only the 16px lucide `circle-alert` icon, with tooltip and accessible name "Needs attention"; no word.
- **Scrim:** dims behind a sheet.
- **Menu:** pop-up menus and tooltips.
- **Shadow:** under an open menu.
- **On accent:** default button text.
- **Linux window buttons:** close and minimize, with dark glyphs on hover.

Do not add colored icon tiles, section accents, or tinted panels.

## Typography

Linux embeds Inter.

Typography tokens: Title for onboarding/About; Toolbar title for chrome; Heading for Settings groups; Body for rows; Button for controls; Secondary for help and footers. Headings and secondary text use the secondary label color.

## Layout

The window opens at 940x600px. Only height changes: 600 to 900px, capped by screen height, saved on resize and restored inside the screen. Lists grow with it. No zoom or full screen. macOS uses a transparent full-size title bar and AppKit lights. Linux draws close and minimize (12px circles, 8px apart), without maximize. A compositor means an X11 owner of `_NET_WM_CM_S<screen>` or any Wayland session: use 10px corners, a separator hairline, opaque content, translucent sidebar, and `_KDE_NET_WM_BLUR_BEHIND_REGION` on the sidebar. Without one, use a square opaque window, solid sidebar, and rectangular flow bar; transparency renders black. Check at start and once a second; switch live.

The 232px sidebar starts with a 48px band (`sidebar-top`) for window buttons and "Hushpen". Rows below are 28px high, 2px apart, with 8px side padding. The content title bar is 52px high.

Window buttons, app name, and view title center 26px from the top. Both systems place 12px circles at (20, 20). After opening, macOS reads the taller native button frame and raises it by half the height difference. The name starts 16px after the last circle, in toolbar title style, truncated inside the band.

The content pane has a 24px margin. Groups fill the pane width. Onboarding is one centered column, at most 640px wide.

Rows are at least 36px high with 10px side padding: title left, value or control right on the same line. One secondary line may follow the title, truncated, never wrapped. Longer help uses one 11px secondary group footer or a tooltip.

The model step of onboarding shows the speech rows of Models, Parakeet first; a progress bar replaces the secondary line while a model downloads. Continue stays off until one speech model is downloaded. Details opens the sheet over the step. Each permission row has its icon, name, status (the word Ready, or the attention icon), and buttons on one line. On macOS the buttons are Allow (the default button), then Open Settings. The description is one short secondary line.

Show home-relative paths as `~/...`.

## Elevation & Depth

Depth is tonal. Shadows are only for dropdowns and menus: 4px down, 12px of blur, in the shadow color.

- Hairlines (one device pixel, separator color) sit only between rows inside a group, and span the full width of the group on every page.
- A group has a 0.5px stroke in the separator color. There is no toolbar line and no other stray border.
- The focus ring is 2px in the accent color, inside the bounds of a button, a pop-up button, or a row. A focused text field has a 1px accent border and no glow.
- A sheet floats centered over the scrim: 460px wide (`sheet-width`), the content color, the window radius, and a hairline edge. It has no shadow. No tooltip of the page under it shows over it. Done and Esc close it. Tab stays inside it, and closing it returns focus to the control that opened it.

## Shapes

Use `rounded` tokens for groups, controls, selection, icons, and windows.

## Motion

Motion is short and quiet, with these tokens:

- **Fast (90 ms):** tooltips, menus, and Play/Pause icon changes.
- **Slide (110 ms):** switch knobs and segmented fills.
- **Base (130 ms):** views, onboarding steps, and sheets.
- **Travel (4px):** entry distance.

Every animation uses one cubic ease-out curve. It never overshoots or bounces.

- A tooltip fades in and rises 4px into place.
- A pop-up menu fades in and drops 4px into place, each time it opens.
- Views and onboarding steps fade in and rise 4px; sidebar and title bar stay still.
- Sheets fade in and rise 4px while the scrim fades in; no scaling.
- Switch knobs slide and tracks blend; segmented fills slide.
- Two-state icons crossfade: the old shrinks away, the new grows from quarter size.

Animate state changes, never redraws. Keyboard and accessibility state changes at once. Text moves and fades as one block; layout never animates. Reduce Motion shows changes at once; pixel tests enable it for settled frames.

## Components

**Updates:** Windows setup installs run the verified installer after exit, then restart. Portable ZIP installs show a release notice.

**Sidebar:** 8 rows: Home, History, Manual, Dictionary, Models, Devices, Settings, About. Icons are monochrome, 16px, in the secondary label color. Selection uses the selection fill and label-colored icon. Restore the last view; start on Home. Incomplete onboarding shows first.

**Toolbar:** the view title (toolbar title style) on the left of the title bar area. On Settings and Models the search field sits at the right end, 300px wide, 24px from the window edge, centered on the chrome line. On Manual the mode control sits in the middle of the band instead; it comes after the title and before the content in keyboard order. A press on an empty part of the band moves the window, but not a press on the search field or the control.

**Permissions:** onboarding and Settings share core checks. In Settings, the group follows Updates. Keep the three macOS rows. Windows shows Microphone, Admin apps, and Global keys. Linux shows microphone information, an X11 warning on Wayland, and tray information without a watcher. Each row has one secondary line, a status word or attention icon, and at most two actions. Only microphone readiness blocks Continue on all systems; macOS also requires Accessibility and Input Monitoring. Missing keys never promise a working shortcut. Refused paste keeps the text copied with a notice.

**Settings:** one scrolling list, grouped like macOS System Settings. Each section is a heading above its group, with an optional footer line under it, in this order: General, Shortcuts, Audio, Cleanup (with AI cleanup and the endpoint form), Engine, Flow bar, Storage, Updates. Audio holds Microphone, Input boost, Audio enhance, Sounds, Sound volume, Mute other audio while dictating (on by default), and Maximum length. Engine holds When idle (Keep in memory, Page out on Linux only, Unload; macOS defaults to Keep in memory, Linux to Page out) and Idle time (5, 15, 30, or 60 minutes, shown only for Page out and Unload). Cleanup holds Profile, a pop-up of General, Message, Email, and Notes, whose secondary line says "Edited" when the user changed that profile, and Prompt, with View… for S1-mini or Edit… for other models. Prompt opens a sheet titled "<Profile> prompt": the prompt in a 132px text area (read-only for S1-mini), a count or notice row, for S1-mini a group with the Styling, Structure, and Context pop-ups, then a group of what is also sent (the control line, the Dictionary words, and the examples), and the buttons Reset to Default and Done. The sheet shows exactly what is sent. Typing in the search field keeps only the matching rows under their headings; a row's label, its section, and its aliases all match. With no match the list shows `No settings match "<query>"` and a Clear button. Cmd+F (Ctrl+F on Linux) focuses the field. Esc clears it, and a second Esc leaves it. A new query scrolls the list to the top. The query is never saved. Numeric settings use a slider (160px track, 5% steps (2.5% for Sound volume, which defaults to 12.5%), arrow keys, Home and End) or a pop-up of fixed choices inside the registry range.

**Dictionary:** A Packs section has one row per word pack (Indian names, Indian places, Hinglish words, Tech terms): the name, "N words. <about>", Preview…, and a switch, all off at first. Its footer says "Words of a pack that is on count like your own. Your words come first." Preview opens a sheet with the word list (at most 324px high, scrolling inside its group) and Done.

**Groups and rows:** follow Layout and use the shared row builders. A notice has one text line and a 16px secondary-label info icon, or the attention icon when action is needed. Failed checks use error-colored text.

**Home:** the Dictation group (status, Record, the transcript, language) above the Microphone group. The Microphone group is one row: the microphone pop-up (Default with the system microphone in brackets, then each microphone) and Test microphone. The level meter row shows only while a test runs; its bars grow up and down from the middle of the row. The transcript, or the placeholder before it, is centered in its block with 24px at the sides and 16px above and below; a long transcript keeps every line centered.

**History:** one group: the search field, then a row for each transcript and each read aloud, newest first. A row leads with a 16px icon in the secondary label color: lucide `clock`, `volume-2` for a read aloud, the exclamation for a failure, or `x` for a cancel. The secondary line of a read aloud names Read aloud, the engine, and the voice. A row opens its detail, one group of rows:

- The top row holds Back, one secondary line with the time, the app or the file, the length, and the result, and at the right end the copy icons: Copy original (lucide `clipboard-clock`, only for a row with AI text), then Copy (lucide `copy`).
- The Recording row holds Export (lucide `share`, which opens the bar of formats: TXT, SRT, VTT, JSON), Reprocess (lucide `refresh-cw`, off with no audio or while it runs), and Delete (lucide `trash`, with undo). A read aloud has Read aloud again (lucide `volume-2`) in the place of Reprocess: it opens Read aloud on the text and plays it with the voice chosen there.
- The player row, when the audio is kept, holds Play or Pause, the seek bar, and the time.
- The fields follow: the raw text, the text after the rules, the text after AI cleanup with Undo cleanup (lucide `undo-2`) at its right end, the final text, the app, and the facts. A read aloud shows the text, the engine, the voice, the language, the length, and the time.

Each group of icons runs from lightest to heaviest. The focus moves in the order the controls are drawn.

**About:** centered header, facts group, links group, and centered secondary footer. Header: 96px icon with 16px radius, title "Hushpen", "Private dictation that stays on this Mac" ("on this computer" on Linux), secondary build stamp `Version <version> (<commit>, <date>)` (`-dev` after debug versions), and Check for Updates with one secondary result line below. Facts: Version, Engine (whisper.cpp and current model), Data folder with Show. Links: Website, GitHub, Report an issue, Open-source licenses, Open log folder; each ends with the external-link glyph. Footer: "Copyright (c) 2026 Suraj Mandal. Licensed under AGPL-3.0-or-later." Only Engine names a stack component.

**Models:** one section for each model kind, in this order: Speech to Text, Text cleanup, Text to Speech. Each section is a heading above its group, with one footer line. Text cleanup lists only the models this machine is offered. A row holds the name, then the word "Recommended" in the secondary style for the suggested model of its kind, then the state word, over one secondary line that says what the model is for. The state word is Default or Checking in the secondary label color, the percent of a download or "Updating 40%" in the label color, or Failed verification in the error color; a model that is downloaded, or not, has none. On the right, lightest to heaviest: the Accuracy and Speed meters (speech only), the size, and up to four icon buttons: Details (lucide `info`, 14px, tertiary label color, secondary on hover), Download (`download`) or Cancel (`x`), Set as default (`star`, speech only), and Delete (`trash`). A meter is a secondary label and five 6px dots 5px apart, filled in the secondary label color and empty in the tertiary; its tooltip and accessible name say "Accuracy: High (4 of 5)". The meters (112px) and the size (56px) keep fixed widths. The button group (22px buttons, 6px apart) fits the most buttons a row of the section has now, and a row's buttons sit against its right end with no empty slot, so the last one ends at the row padding and the sizes line up. The default has no Set as default; a failed speech row has Download, Set as default, and Delete. The first finished speech download becomes the default, and deleting the default makes another downloaded speech model the default (D-45). Speech to Text has four rows, in this order: Parakeet v3 Q4 (Q4_K, 416 MB, Recommended and the default), Whisper Large v3 Turbo Q5 (Q5_0, 574 MB), Whisper Small Q5 (Q5_1, 190 MB), and Whisper Base Q5 (Q5_1, 60 MB). Their secondary lines explain the roles: English and 24 other European languages; Hindi and other languages outside Parakeet's set; translation to English; older or slower computers. No English-only or full-size speech variants are offered. Typing in the search field keeps only the model rows whose name, description, language, or size matches, or whose section name holds the whole query (D-45); a section with no match hides. With no match the view shows `No models match "<query>"` and a Clear button. The field takes Cmd+F and Esc as in Settings, and the query is never saved. A model file may list earlier pins it replaces. When the files on disk match an earlier pin, the app fetches only the changed files in the background at start, and the state word of the row says Updating, with no top notice. A network stop stays quiet and the next start tries again. Failed verification is only for files that match no known pin.

**Manual:** a segmented control in the title bar, centered on the pane width and on the chrome line, picks Transcribe or Read aloud. The view opens on the mode the user picked last. The content starts right under the title bar, with no row of its own for the control.

- Transcribe starts with the drop zone: 152px high, the group fill, a dashed hairline in the tertiary label color, the 8px group radius, a 28px lucide `audio-lines` icon, "Drop audio files here", "Choose Files…", and the formats on one secondary line. Files dragged anywhere over the window turn it to the selection fill with an accent outline. A drop anywhere on the window opens Manual: text files go to Read aloud, the rest to the queue.
- The Options group holds Model, Language, and Translate to English. Translate to English is off for Parakeet and Turbo; Parakeet's tooltip says "Parakeet cannot translate. Choose Whisper Small Q5 in Models." With Parakeet selected and a language outside its 25-language set, show one plain information row: "Parakeet does not support this language. Choose Whisper Large v3 Turbo Q5 in Models." Auto stays available; Home shows no Detected label for Parakeet, which reports no language code.
- The Queue group has one row per file: the name, then size · length · the folder, the time left, the snippet, or the error on the secondary line. On the right are a 120px progress bar, a 96px status column, then the buttons. A waiting or running file has Cancel. A done file has Copy, Export (TXT, SRT, VTT, JSON), Show in History, and Remove. A failed file has Retry and Remove. Clear Finished sits at the right end of the Queue heading, on the same line as the buttons in the rows.
- Read aloud starts with one text group with no heading. Its first row holds the status, the progress while reading, Export Audio…, then Play or Stop as the default button. The 132px text area, the character count, and "Open Text File…" for `.txt` and `.md` follow. The Voice group below it holds Engine (System voice, Kokoro, Chatterbox Turbo, Chatterbox Nano; a model that is not downloaded has "Not downloaded" after its name in the menu), Language, Voice, Voice library with Manage Voices… (Chatterbox only), Mix with and Mix (Kokoro only), Speed, and Format. Mix with is a pop-up (None first and by default, then the other voices of that language); once a voice is picked, the Mix slider (10% to 90% in 10% steps, default 50%) sets its share. History names a mixed read like "Heart with 30% Adam". For Chatterbox the Voice line names the default voice of the library ("Default is Anna"). When the model of the engine is not downloaded, a notice row with "Download in Models" follows the Engine row. While the model updates, the notice reads "<Engine> is updating its voice model. Progress shows in Models." with an Open Models button. Language and Voice are off. Chatterbox reads English only: for other text the error row has Use System Voice. Each text spoken to the end goes in History with its audio.

**Segmented control:** 22px high, 5px radius, on the control fill. Each segment is 120px wide, or 72px inside a row (Audio enhance), so the row keeps room for its title; the selected one has the segment fill and the label color, the others the secondary label color. Left and Right move the choice.

**Details sheet:** the Details button opens it for a model. It shows the name, the description, and what the model is good for; a facts group (languages, accuracy and speed as a word with "N of 5", quantization with the bits of most weights such as `Q5_1 (5-bit)`, memory as the measured use with the model loaded and at most during a long job such as "About 320 MB idle, up to 470 MB" (a reference measurement on the M4 Pro with Metal and eight threads, not a limit; readings in `assets/models.json`), size on disk, license, and status: Default, Downloaded, Not downloaded, or the state word of the row); and a files group (the source page as a link row, the folder as a `~/...` path with Show in Finder or Show in Files, and the SHA-256 with Copy).

**Manage Voices sheet:** Manage Voices… opens it. It follows the sheet rules above and has two pages.

- Voices: the title, the line "Chatterbox reads with the default voice unless you pick another.", and one group with a row for each voice: Built-in first ("Comes with the model"), then each cloned voice with its length and the day it was added (`8 s · Added 2026-05-01`). On the right, lightest to heaviest: Play (lucide `play`, `square` while it plays), Rename (`pencil`), Delete (`trash`), then Set as Default or the word Default in a 120px column. Rename turns the row into the name field with Cancel and Save. Delete asks in the row (`Delete "Anna"?` over "Its clip is deleted too.") with Cancel and Delete. The buttons are Add Voice… and Done.
- Add Voice: the title, the line "Use only a voice you have the right to use.", and a group with four plain lines to read, the Recording row, the level meter while it records, and the Audio file row ("Or clone the voice from a WAV, MP3, or M4A file", with Choose File…). The Recording row has a record icon button (lucide `mic`, `square` while it records), then a play icon button for the clip, off until a clip exists. A take is 5 to 20 s ("Speak now. 12 s left, or press Stop"). The clip keeps at most 20 s of speech, without the silence around it. A second group holds the Name field ("Shown in the Voice menu"), filled with the first free name such as "Voice 1". The buttons are Back and Save, which is off until a clip exists.

**Pop-up buttons and menus:** show the current value and chevron, at most 240px wide. The menu floats below the button, right-aligned, moved inside window edges and never clipped by groups. Use the `menu` tokens and shadow. Options are 24px high, with a selected check and optional secondary note. Show at most eight whole options, scroll the rest, and reveal the selection on open. Up/Down move and scroll; Enter picks. A choice, Esc, or outside press closes and restores button focus. Outside presses reach nothing below. Tab closes and advances focus. Over eight options, add a focused 22px search field at menu width. Match labels without case; keys act on matches. Esc clears before closing. No match shows one disabled "No matches" row. Shorter menus have no search.

**Buttons:** 22px high, 5px radius; default uses accent fill and white text, others neutral control fill. Disabled text uses the tertiary label color. Icon buttons are 22px squares with 16px secondary-label lucide icons, unfilled until hovered. Give each a tooltip and accessible action name, such as "Copy text". Disabled icons use tertiary labels and cannot take focus. A missing list action leaves no empty slot.

**Text inputs:** use button size, radius, and control fill. Pad text clear of the edges. Search leads with its glyph.

**Switches, sliders, and selection focus:** use the accent. The focus ring of a switch or a slider sits outside its track, and the control moves right by the ring, so the track ends on the same line as the buttons in the rows around it. A slider row shows its value, such as `150%`, `1.25×`, or `+12 dB`, just left of the track in a 48px right-aligned column: body text, the secondary label color, and tabular digits, so the number stays in place while a drag or an arrow key changes it. The line under the row title stays plain help.

**Shortcut fields:** in Settings, every shortcut field is 220px wide (the text field width), so the column lines up. The default hands-free reads `⌥⇥` (Option+Tab) on macOS and `Right Alt ×2 or Right Alt+Space` on Linux, where Alt+Tab switches windows; a binding that may be cut shows whole in a tooltip. Each shortcut row holds, left to right, a reset icon button (lucide `rotate-ccw`, shown only when the shortcut is not its default), the field, and a switch. The field of a shortcut that is off uses the tertiary label color.

**Audio enhance:** the Audio group row after Input boost: "Audio enhance", the line "Makes speech clearer for the transcript.", and a segmented control Off | Auto | Custom (Auto by default: a low cut at 80 Hz and the bass 6 dB down). With Custom an "Adjust…" button follows the control, and picking Custom opens the tuning sheet. The sheet (the sheet rules above) has two pages.

- Tuning: the title "Audio enhance" and one secondary line; a Sample group with one row, "Sample" and a line that counts down an 8 s take ("Speak now. 5 s left") or names the kept take, then a record icon button (lucide `mic`, `square` while it records) and a play icon button (`play`, `square` while it plays), off until a take exists so the record button never moves; the level meter shows under the row while it records. The last take is kept on disk as the microphone gave it (`cache/enhance/sample.wav`) and each play runs it through the values as they are now. A values group follows: Low cut (Off to 200 Hz), Bass (-12 to +6 dB), Treble (-6 to +12 dB) as sliders with their readings, and Noise reduction as a switch with the line "Can make the text worse. Use it only in a noisy room." Moving a value makes the mode Custom. The buttons are Reset to Auto, Find best settings…, and Done.
- Find best settings: the title and one line; a group with the three lines to read (sounds such as v and w, th, r, and a number) and up to three Dictionary words, then the row "Your reading" with Record, which turns to Stop, and the meter while it records (30 s at most). Stop starts the test: the row says "Testing 12 of about 60" with a 120px progress bar and Cancel. The test transcribes the take with the current speech model and language, changing one value at a time (low cut, bass, treble, noise reduction, input boost), and keeps a change that gets fewer words wrong, until a round changes nothing. The results group has the five best settings, ties going to the one nearest Auto: the setting in a few words ("Low cut 80 Hz, bass -6 dB") over its score ("2 of 31 words wrong"), a play icon button, and Use, which sets Custom with those values and that boost and goes back to the tuning page. When every setting scores the same, one notice row says so and Auto stays. The buttons are Back, Test again (with results), and Done.

**Recording overlay (flow bar):** a compact dark capsule: 28px high, with fully round ends, a 0.5px `#353535` edge, and 12px medium text. It shows only while a dictation runs and while its result or notice is up. A new dictation starts at once, even while the last one still transcribes or shows its result: the bar shows Listening for the new one, the older one finishes behind it, and the texts go in in the order they were spoken. At rest nothing shows, unless the user turns on Show when idle (a 67 by 22px pill). Every state is balanced: a 20px circle with an 11px glyph sits 4px in from each end, on both sides or on neither, and the text or the wave is centered between the circles, or in the whole pill when it has none. A pill with text is as wide as its line: 4px, the circle, 8px, the line, 8px, the circle, and 4px, or 12px at each end without circles, at most 366px. The line is the widest the pill can show, hover text included, so the pill keeps its size under the pointer. Listening (118px) holds a round cross on the left (cancel), the waveform, and a round white tick on the right (finish). While it transcribes or cleans up (118px), a small wave floats in place of text: bars that ripple while transcribing, a sine ribbon while cleaning; after 2 seconds of waiting it turns once to a calmer look and stays: a slower ripple while transcribing, a breathing pulse while cleaning. A new stage blends from what the wave shows, so it never jumps. The three bars at each end of a wave stand at 70, 85, and 95% of their height. Preparing the AI model shows the wave and its line. Pasted or Copied shows the word alone for 1 second. When the paste can be taken back, a done mark (a `#30D158` tick on the same green at 18%) sits on the left and a round undo button (named "Undo paste"; on hover the line reads "Undo") on the right for 4 seconds. After a cancel the bar shows a gray x mark, "Cancelled", and a round Recover button for 4 seconds: Recover transcribes the cancelled dictation and pastes it as if it had been finished. A mark is a status image, not a button. With Open history a notice has a mark on the left (`#FF9C8F` for an error, `#FFC266` when blocked) and the clock button on the right. It has no language control. It appears on the screen that has the pointer and stays on that screen until it hides. A position the user drags it to is kept relative to the top-left corner of the screen. It fades in and grows from 96% of its size in place over 160 ms when it appears, and fades and shrinks back to 96% when it goes; it never shrinks to a circle. A change of state is one calm move: the width eases to the new size without overshoot (about 280 ms), the old content fades out (100 ms) before the new content fades in (160 ms), the listening bars turn into the processing wave where they stand, and the marks and round buttons of the new state fade in and grow from 90% (180 ms) once the size has settled. On macOS the bar has a clear background and no window shadow, and its window goes only after it drew a clear frame, so nothing flashes as it hides or shows again. With Reduce Motion the wave stands still, the content crossfades, the size changes while the pill is empty, and the bar hides at once.

**iOS app:** cards on the system grouped background (24pt round, 16pt padding) and a floating glass tab bar (Home, History, Stats, Settings) on every tab, hidden on pushed screens; no central mic. Home, History, Stats, Models, and Settings are cards; History detail, Devices, and Server mode are lists. A History card holds the text (3 lines at most) and one row: the time, "Spoke 7 s · Took 1.4 s · 16 words", and a plain copy icon (16pt glyph, 44pt touch area). Long stats drop "Took" first, then end in an ellipsis; the time and copy icon never shrink. Stats shows "43 of 52" and "+N devices" from the phone's own history, which stays on the phone; there are no per-app numbers. A Models button is a 28pt glyph in a 44pt touch area on the name line: `arrow.down.circle` to get, `checkmark.circle.fill` when installed, a progress ring while downloading. The Power row shows the battery icon, then the percent at its right.

**iOS keyboard:** a native-looking keyboard (letters, numbers, symbols, a globe key, a return key) with one extra key, the microphone, next to the space bar. There is no suggestion bar. A fixed band 44pt high sits above the keys and holds the status pill, so keys never move: the band is empty at rest. The pill is a 36pt glass capsule in the System banner style, and it follows the keyboard appearance (Light or Dark). A status icon is a 28pt gray filled circle (`checkmark.circle.fill`, `exclamationmark.circle.fill`) with no status color; the buttons on the right are Tinted symbol buttons: 28pt round buttons with a 12% tint (16% in Dark), and a Copy capsule, dark in Light and white in Dark. It shows Listening (cancel and finish buttons, the wave), Transcribing (the wave and the word, 16pt of padding on both sides), Inserted (a gray tick, Undo, for 4 seconds), and Paste failed. The wave is 11 bars, 2pt wide and 2pt apart, 3 to 16pt high, and tapers toward both edges like the flow bar; while it transcribes it turns once from the flowing look to the ripple after 2 seconds. Paste failed has two lines, "Paste failed" and "Copied to clipboard" (11pt, gray). The globe key shows only when iOS asks for one, tap switches keyboard and a hold lists them; the microphone key is the dictation key, so iOS draws no dictation key of its own. The keys stay usable while it listens or transcribes; only the microphone key dims, and while it listens it stops the take. The text goes in at the cursor as it is when the transcript arrives, so typing in the meantime changes nothing. When the host app refuses it, the keyboard copies the text and the pill says "Paste failed. Copied to clipboard."; when the copy fails too, it says "Paste failed." with a Copy button. The Hushpen page the microphone key opens shows a 2 second looped demo of the back swipe, a soft, tight ring around the back link in the status bar, and one line of 17pt semibold text at the top ("You're all set. Head back to keep typing."). It shows no "Listening" text and no listening animation. It never names the host app, because iOS gives the keyboard no way to read it; iOS draws the name in the status bar itself. With Reduce Motion the demo is a still frame.

## Devices

Devices follows flow D: separate Speech to Text and Text cleanup routes. First come "Use models on other devices" (off by default) and "Do not load models on this computer"; while the first is off, the rest fades to 35% and takes no input. Route rows hold a grip, kind icon, name, model (112px), latency (56px, tabular digits), status (100px; Ready in success, Not reachable in error), and Move up and Move down. Dragged rows show at 50% and move past each row's halfway point; release counts as the last move. This computer has no grip, says Fallback, stays last, and hides with local models off. Your devices rows open Details; Add Device… ends that heading. Details holds rename, Remove Device… (asked in a row), and core-approved unload rules.

**Add a Device (D-57):** Nearby | Remote tabs. Nearby lists phones, each with Pair… (then "Waiting…"). When the phone connects, both screens show the same 6-digit pairing number after verified hash commitment, with Cancel and Pair below. Tap Pair on both. The protected handshake sends the computer's real name. Cancel or mismatch shows the QR again.

**Fallback:** always under the Nearby list, even when empty: a QR, white in both appearances, pins the computer's public key; "Or type the code" shows an 8-digit pairing code. The phone's Pair a Desktop scans or types (`mobile/SCREENS.md`). Offers work once, expire in two minutes, and allow three tries; the QR row shows the time and tries left. A wrong code keeps the offer; an expired or used-up one offers Show New Code. No copyable URI or paste field. The listener opens and closes with the sheet.

**Remote:** merge `~/.ssh/config` and installed `tailscale status`; show each machine once. Rows: server icon, name, secondary line ("From ~/.ssh/config", "From Tailscale · <address>", or both), Set Up…. "Another machine" has "Host name or address", a field, and Set Up… (off while empty). Setup progress stays here; SSH carries the offer.

## Do's and Don'ts

Use one group per cluster and lucide-style monochrome icons. No gradients or blobs.
