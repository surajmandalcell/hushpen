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

Hushpen is a quiet, fixed-size desktop workspace for private dictation, file transcription, local model selection, and playback. It follows the system: it looks like a native macOS app, in Light or Dark, and it stays out of the way.

The product is not a marketing site. It has no hero layout, no promotional panels, and no decorative compositions. The first screen is the working app. Layouts are dense enough for daily use and calm enough that recording and transcription states stay easy to read.

The look is neutral and uses very few colors. Gray fills, one blue accent, and a status color only on status words and the one status icon. Avoid exposing implementation details such as framework names or internal architecture in the UI.

## App Icon

The app icon is a Roman statue drawn in outline: a young woman in profile, eyes closed, whispering into an upright fountain pen held just below her lips. Ink lines (`#1B2228`) sit on a warm marble tile (`#F7F4EF` to `#E8E3DA`, top to bottom), with no text and no other color.

Each icon design lives in `assets/icons/<name>/`: `mark.svg` is the full drawing on the 1024 tile, and `mark-small.svg` a bold crop of the head and pen. `classic` ships. `sari` (braided bun, jhumka, sari pallu) and `waves` (long loose hair) are kept for seasonal use. `harness/icons.sh <name>` builds every icon file from one design:

- `assets/hushpen-app-icon.svg` and `.png`: the drawing on an 824px squircle at 100px on the 1024 canvas (the macOS grid), with the template shadow (10px down, 30% black).
- `assets/hushpen-app-icon.icns` and `assets/linux-icons/*.png`: 128px and up show the full drawing, 48 and 64px the bold crop, and 16 to 32px the solid head and pen of the menu bar glyph, because finer lines turn gray there.
- `assets/hushpen-app-icon.icon` and `assets/hushpen-app-icon.car`: the Icon Composer source and its compiled catalog, shipped as `Assets.car` with `CFBundleIconName`. macOS 26 and later draw it in the Dock and Finder; without it they shrink the .icns into a gray tile. In the Dark icon style the system draws a dark tile and the lines turn light.
- `assets/hushpen-app-icon-192.png`: About draws the tile without its margin at 96px, with the 16px app-icon corner, at 2x.
- The menu bar and Linux tray glyphs, from `assets/hushpen-menubar.svg`.

## Appearance

The app follows the system appearance, Light or Dark, and changes live when the system setting changes, with no restart. Every view reads its colors from one palette for the current appearance. If the appearance is unknown (Linux without a desktop setting), the app uses Dark.

The flow bar pill is the one exception. It stays a dark HUD (`#2B2B2B`, light text) in both appearances, because it floats over other apps.

The macOS menu bar item is the glyph alone, with no name beside it: the solid head with its bun and an upright pen, 1.5pt wide with a 1pt gap before the lips so the two never merge. It is a template image (`assets/hushpen-menubar.png`, 22 by 16 points at 2x), so macOS tints it for Light, Dark, and a highlighted menu bar. Linux panels get the same glyph on a 32px square, black or white for their appearance.

## Colors

All colors are neutral grays plus one accent. Tokens come in a Dark and a Light set.

- **Content (`#1E1E1E` Dark, `#F5F5F5` Light):** the opaque content pane and the onboarding page.
- **Sidebar (`#2B2B2D` Dark, `#E8E8EA` Light):** the solid sidebar on Linux without a compositor. On macOS the sidebar has no fill: the native sidebar material shows through.
- **Sidebar blur (the sidebar color at 80%):** the sidebar on Linux with a compositor. What is behind the window shows through; KWin blurs it, and picom blurs it when its own configuration asks.
- **Group (white 5% Dark, black 3.5% Light):** the fill of a grouped section.
- **Selection (white 10% Dark, black 9% Light):** the neutral fill of the selected sidebar row and of a selected list row. Never blue.
- **Control (white 12% Dark, black 8% Light):** the neutral fill of a push button.
- **Label (white 85% Dark, black 85% Light):** main text.
- **Label secondary (white 55% Dark, black 50% Light):** secondary lines, sidebar icons, and group footers.
- **Label tertiary (white 25% Dark, black 26% Light):** placeholders and disabled text.
- **Separator (white 10% Dark, black 10% Light):** hairlines and the group stroke.
- **Segment (white 22% Dark, white Light):** the selected segment of a segmented control, raised above the control fill.
- **Accent (`#0A84FF` Dark, `#007AFF` Light):** the default button, a switch that is on, the focus ring, the selection focus, links, and the outline of a drop zone while files are dragged over the window. It is the only accent.
- **Status colors, on status words only:** success (`#30D158` Dark, `#34C759` Light) for ready, error (`#FF453A` Dark, `#FF3B30` Light) for a failure.
- **Status icon, for a row that needs the user:** warning (`#FF9F0A` Dark, `#FF9500` Light) on an exclamation icon (lucide `circle-alert`, 16px) with the tooltip "Needs attention" and no word. The icon is the only place the warning color appears. Its label in the accessibility tree is "Needs attention".
- **Scrim (black 40% Dark, black 20% Light):** dims the window behind a sheet.
- **Menu (`#2B2B2D` Dark, white Light):** an open pop-up menu and a tooltip.
- **Shadow (black 50% Dark, black 20% Light):** the shadow under an open menu.
- **On accent (`#FFFFFF`):** text on the default button.
- **Linux window buttons:** close `#FF5F57` and minimize `#FEBC2E`, with dark glyphs on hover.

Do not add colored icon tiles, section accents, or tinted panels.

## Typography

macOS uses the system UI font (`.SystemUIFont`, SF Pro). Linux keeps the embedded Inter, so text looks the same on a machine with no system copy of it.

- **Title:** 22px bold, on onboarding pages and for the app name on About.
- **Toolbar title:** 13px semibold, in the title bar area.
- **Heading:** 13px semibold, in the secondary label color, above each Settings group.
- **Body:** 13px regular, for row titles and values.
- **Button:** 13px medium.
- **Secondary:** 11px regular, in the secondary label color, for descriptions and group footers.

Letter spacing stays at `0em`. Weights are 400, 500, 600, and 700 for the title only.

## Layout

The window is fixed at 940px by 600px. It is not resizable, has no zoom, and has no full screen. On macOS the title bar is transparent with full-size content and AppKit draws the traffic lights. On Linux the frame is custom: the app draws a close and a minimize button (12px circles, 8px apart) and no maximize button. With a compositor (an owner of `_NET_WM_CM_S<screen>` on X11, or any Wayland session) the Linux window is see-through like the macOS one: 10px corners, a hairline edge in the separator color, an opaque content pane, the translucent sidebar, and `_KDE_NET_WM_BLUR_BEHIND_REGION` set to the sidebar. Without one it is square and opaque with the solid sidebar, because a transparent window renders black there. The app reads the compositor at start and once a second, and switches live. The flow bar is a rectangle without a compositor for the same reason.

The sidebar is 232px wide. The first 48px (`sidebar-top`) hold the window buttons and the word "Hushpen" beside them. Rows start below it: 28px high, 2px apart, with 8px of padding at the sides. The title bar area above the content is 52px high and holds the view title.

The chrome has one center line, 26px from the window top. The window buttons, the app name, and the view title all center on it. The 12px circles sit at (20, 20) on both systems. On macOS the native button frame is taller than its circle, so the app reads the frame height after the window opens and moves the frame up by half the difference. The name starts 16px right of the last circle, uses the toolbar-title style (13px semibold, label color), and truncates inside the band.

The content pane has a 24px margin. Groups fill the pane width. Onboarding is one centered column, at most 640px wide.

A group holds rows. A row is one line, at least 36px high, with 10px of side padding. The title is on the left. The value or the control is on the right, on the same line. A row may have one short secondary line under the title. It is truncated, never wrapped. Longer help goes in one group footer line (secondary, 11px) or in a tooltip.

Onboarding uses the same groups and rows. The model step shows its progress bar only while the model downloads. Each permission row has its icon, name, status (the word Ready, or the attention icon), and buttons on one line. On macOS the buttons are Allow (the default button), then Open Settings. The description is one short secondary line.

Long local paths show as `~/...` when they are under the user home directory.

## Elevation & Depth

Depth is tonal. There are no shadows on normal content. Shadows are only for dropdowns and menus: 4px down, 12px of blur, in the shadow color.

- Hairlines (one device pixel, separator color) sit only between rows inside a group, and span the full width of the group on every page.
- A group has a 0.5px stroke in the separator color. There is no toolbar line and no other stray border.
- Hover and selection use the neutral selection fill.
- The focus ring is 2px in the accent color, inside the bounds of a button, a pop-up button, or a row. A focused text field has a 1px accent border and no glow.
- A sheet floats centered over the scrim: 460px wide (`sheet-width`), the content color, the window radius, and a hairline edge. It has no shadow. No tooltip of the page under it shows over it.

## Shapes

- **Groups:** 8px radius.
- **Push buttons:** 5px radius, 22px high.
- **Selection and list rows:** 6px radius.
- **App icon tile:** 16px radius.
- **Window on Linux with a compositor:** 10px radius, like a macOS window.
- **Window buttons on Linux:** fully round.

## Motion

Motion is short and quiet. It shows where something comes from and never makes the user wait. Every animation uses the same tokens:

- **Fast (90 ms):** something small that appears or changes, such as a tooltip, a pop-up menu, or the icon of Play and Pause.
- **Slide (110 ms):** a control that moves to a new state, such as the knob of a switch or the fill of a segmented control.
- **Base (130 ms):** a surface that comes in, such as a view, an onboarding step, or a sheet.
- **Travel (4px):** how far a surface moves on its way in.

Every animation uses one cubic ease-out curve. It never overshoots or bounces.

- A tooltip fades in and rises 4px into place.
- A pop-up menu fades in and drops 4px into place, each time it opens.
- A view picked in the sidebar fades in and rises 4px into place in the content pane. The sidebar and the title bar stay still. A new onboarding step comes in the same way.
- As a sheet opens, the scrim fades in, and the sheet fades in and rises 4px into place. It does not scale, because the renderer cannot scale an element.
- A switch slides its knob across and blends its track into the new fill. A segmented control slides its fill to the new segment.
- An icon button with two states, such as Play and Pause, cross-fades its icons: the old icon fades out and shrinks, and the new one fades in and grows from a quarter of its size into place.

An animation plays only when its state changes: a new view, a menu that opens, a switch that turns on. A redraw of the same state never plays it again. The new state is there at once for the keyboard and the accessibility tree, so a switch reads "on" before its knob arrives. Text moves and fades as one block; its layout never animates. With Reduce Motion every change shows at once. The pixel tests turn on Reduce Motion, so every baseline is a settled frame.

The flow bar has its own motion, described under Recording overlay.

## Components

**Sidebar:** 7 rows in this order: Home, History, Manual, Dictionary, Models, Settings, About. Icons are monochrome, 16px, in the secondary label color. The selected row uses the selection fill and the label color for its icon. There are no colored tiles. A focused row shows the accent focus ring. The window opens on the view that was open at the last quit, and on Home the first time. Onboarding, while it is not done, shows first.

**Toolbar:** the view title (13px semibold, label color) on the left of the title bar area. On Settings the search field sits at the right end, 300px wide, 24px from the window edge, centered on the chrome line. It has no bottom line. On Manual the mode control sits in the middle of the band instead; it comes after the title and before the content in keyboard order. A press on an empty part of the band moves the window, but not a press on the search field or the control.

**Settings:** one scrolling list, grouped like macOS System Settings. Each section is a heading above its group, with an optional footer line under it, in this order: General, Shortcuts, Audio, Cleanup (with AI cleanup and the endpoint form), Engine, Flow bar, Storage, Updates. Audio holds Microphone, Input boost, Audio enhance, Sounds, Sound volume, and Maximum length. Typing in the search field keeps only the matching rows under their headings; a row's label, its section, and its aliases all match. With no match the list shows `No settings match "<query>"` and a Clear button. Cmd+F (Ctrl+F on Linux) focuses the field. Esc clears it, and a second Esc leaves it. A new query scrolls the list to the top. The query is never saved. Numeric settings use a slider (160px track, 5% steps, arrow keys, Home and End) or a pop-up of fixed choices inside the registry range.

**Groups and rows:** as in Layout. A row that is a control stays one line: the label on the left, the control on the right. A notice row has a 16px icon in the secondary label color (info, or the exclamation for something the user has to act on) and one line of text; a failed check shows its text in the error color. Every view builds its rows from the shared row builders, so one row kind looks the same on every page.

**Home:** the Dictation group (status, Record, the transcript, language) above the Microphone group. The Microphone group is one row: the microphone pop-up (Default with the system microphone in brackets, then each microphone) and Test microphone. The level meter row shows only while a test runs; its bars grow up and down from the middle of the row. The transcript, or the placeholder before it, is centered in its block with 24px at the sides and 16px above and below; a long transcript keeps every line centered.

**History:** one group: the search field, then a row for each transcript and each read aloud, newest first. A row leads with a 16px icon in the secondary label color: lucide `clock`, `volume-2` for a read aloud, the exclamation for a failure, or `x` for a cancel. The secondary line of a read aloud names Read aloud, the engine, and the voice. A row opens its detail, one group of rows:

- The top row holds Back, one secondary line with the time, the app or the file, the length, and the result, and at the right end the copy icons: Copy original (lucide `clipboard-clock`, only for a row with AI text), then Copy (lucide `copy`).
- The Recording row holds Export (lucide `share`, which opens the bar of formats: TXT, SRT, VTT, JSON), Reprocess (lucide `refresh-cw`, off with no audio or while it runs), and Delete (lucide `trash`, with undo). A read aloud has Read aloud again (lucide `volume-2`) in the place of Reprocess: it opens Read aloud on the text and plays it with the voice chosen there.
- The player row, when the audio is kept, holds Play or Pause, the seek bar, and the time.
- The fields follow: the raw text, the text after the rules, the text after AI cleanup with Undo cleanup (lucide `undo-2`) at its right end, the final text, the app, and the facts. A read aloud shows the text, the engine, the voice, the language, the length, and the time.

Each group of icons runs from lightest to heaviest. The focus moves in the order the controls are drawn.

**About:** a centered header, a facts group, a links group, and a footer line. The header holds the app icon (96px, 16px radius), "Hushpen" in the title style, the line "Private dictation that stays on this Mac" ("on this computer" on Linux), the build line in the secondary style (`Version 2.0.0 (1a2b3c4, 2026-05-15)`, with `-dev` after the version in a debug build), and a Check for Updates button with what the check found on one secondary line under it. The facts are Version, Engine (whisper.cpp and the model in use), and Data folder (`~/...`, with a Show button). The links are Website, GitHub, Report an issue, Open-source licenses, and Open log folder, each with the external-link glyph on the right. The footer is one centered secondary line: "Copyright (c) 2026 Suraj Mandal. Licensed under AGPL-3.0-or-later." The Engine fact is the one place the UI names a component of the stack.

**Models:** one section for each model kind, in this order: Speech recognition, Text cleanup, Voices. Each section is a heading above its group, with one footer line. Text cleanup lists only the models this machine is offered. A row holds the name, then the word "Recommended" in the secondary style for the suggested model of its kind, and one secondary line that says what the model is for. On the right, lightest to heaviest: the size, the status word, the details button (lucide `info`, 16px, in the secondary label color, tooltip "Details"), then Download or Cancel, Use (speech models only), and Delete. The size, status, and button columns keep fixed widths, so they line up from row to row.

**Manual:** a segmented control in the title bar, centered on the pane width and on the chrome line, picks Transcribe or Read aloud. The view opens on the mode the user picked last. The content starts right under the title bar, with no row of its own for the control.

- Transcribe starts with the drop zone: 152px high, the group fill, a dashed hairline in the tertiary label color, the 8px group radius, a 28px lucide `audio-lines` icon, "Drop audio files here", "Choose Files…", and the formats on one secondary line. Files dragged anywhere over the window turn it to the selection fill with an accent outline. A drop anywhere on the window opens Manual: text files go to Read aloud, the rest to the queue.
- The Options group holds Model, Language, and Translate to English. Translate is off with a tooltip that says why for English-only and turbo models.
- The Queue group has one row per file: the name, then size · length · the folder, the time left, the snippet, or the error on the secondary line. On the right are a 120px progress bar, a 96px status column, then the buttons. A waiting or running file has Cancel. A done file has Copy, Export (TXT, SRT, VTT, JSON), Show in History, and Remove. A failed file has Retry and Remove. Clear Finished sits at the right end of the Queue heading, on the same line as the buttons in the rows.
- Read aloud has a Text group (a 132px text area, the character count, and "Open Text File…" for `.txt` and `.md`), a Voice group, and an actions row with the status, the progress, Export Audio…, and Play or Stop. The Voice group holds Engine (System voice, Kokoro, Chatterbox Turbo, Chatterbox Nano; a model that is not downloaded has "Not downloaded" after its name in the menu), Language, Voice, Voice library with Manage Voices… (Chatterbox only), Speed, and Format. For Chatterbox the Voice line names the default voice of the library ("Default is Anna"). When the model of the engine is not downloaded, a notice row with "Download in Models" follows the Engine row, and Language and Voice are off. Chatterbox reads English only: for other text the error row has Use System Voice. Each text spoken to the end goes in History with its audio.

**Segmented control:** 22px high, 5px radius, on the control fill. Each segment is 120px wide, or 72px inside a row (Audio enhance), so the row keeps room for its title; the selected one has the segment fill and the label color, the others the secondary label color. Left and Right move the choice.

**Details sheet:** the details button opens a sheet for that model. It shows the name, the description, and what the model is good for; a facts group (languages, speed and accuracy as a word with "N of 5", quantization with the bits of most weights such as `Q5_1 (5-bit)`, memory as the measured use with the model loaded and at most during a long job such as "About 320 MB idle, up to 470 MB", size on disk, license, status); and a files group (the source page as a link row, the folder as a `~/...` path with Show in Finder or Show in Files, and the SHA-256 with Copy). Done closes it, and so does Esc. Tab stays inside the sheet, and closing it returns focus to the details button.

**Manage Voices sheet:** Manage Voices… opens it. It follows the sheet rules above and has two pages.

- Voices: the title, the line "Chatterbox reads with the default voice unless you pick another.", and one group with a row for each voice: Built-in first ("Comes with the model"), then each cloned voice with its length and the day it was added (`8 s · Added 2026-05-01`). On the right, lightest to heaviest: Play (lucide `play`, `square` while it plays), Rename (`pencil`), Delete (`trash`), then Set as Default or the word Default in a 108px column. Built-in keeps empty places for Rename and Delete, so the columns line up. Rename turns the row into the name field with Cancel and Save. Delete asks in the row (`Delete "Anna"?` over "Its clip is deleted too.") with Cancel and Delete. The buttons are Add Voice… and Done.
- Add Voice: the title, the line "Use only a voice you have the right to use.", and a group with four plain lines to read, the Recording row, the level meter while it records, and the Audio file row ("Or clone the voice from a WAV, MP3, or M4A file", with Choose File…). The Recording row has a record icon button (lucide `mic`, `square` while it records), then a play icon button for the clip, off until a clip exists. A take is 5 to 20 s ("Speak now. 12 s left, or press Stop"). The clip keeps at most 20 s of speech, without the silence around it. A second group holds the Name field ("Shown in the Voice menu"), filled with the first free name such as "Voice 1". The buttons are Back and Save, which is off until a clip exists.

Done and Esc close it. Tab stays inside the sheet, and closing it returns focus to Manage Voices….

**Pop-up buttons and menus:** a pop-up button shows the current value and a chevron, at most 240px wide. Its menu floats above the page under the button, with the right edges lined up, so no group cuts it; near the window edge it moves inside the window. The menu has the menu color, a hairline edge, the 8px group radius, the menu shadow, and 4px around its options. It is 180px to 320px wide. Each option is 24px high, with a check before the chosen one and an optional secondary note after the label. The menu shows at most eight whole options and scrolls past them, and it opens with the chosen option in view. Up and Down move through the options and scroll with them, and Enter picks one. A choice, Esc, or a press outside the menu closes it, and the pop-up button keeps the focus. Tab closes it and moves on to the next control. A press outside only closes the menu; it does not reach what is under it.

**Buttons:** 22px high, 5px radius. The default button has the accent fill and white text. Other buttons use the neutral control fill. A focused button shows the accent focus ring. A disabled button uses the tertiary label color. An icon button is a 22px square with a 16px lucide icon in the secondary label color and no fill until hovered. A tooltip names it, and its name in the accessibility tree says what it does, such as "Copy text" for Copy. An off icon button uses the tertiary label color and takes no focus.

**Text inputs:** the button height (22px) and radius (5px), with the control fill, so an input in one row with buttons aligns with them. The inner padding keeps the 13px text clear of the edges. A search field (Settings, History) leads with the search glyph.

**Switches, sliders, and selection focus:** use the accent. The focus ring of a switch or a slider sits outside its track, and the control moves right by the ring, so the track ends on the same line as the buttons in the rows around it. A slider row shows its value, such as `150%`, `1.25×`, or `+12 dB`, just left of the track in a 48px right-aligned column: body text, the secondary label color, and tabular digits, so the number stays in place while a drag or an arrow key changes it. The line under the row title stays plain help.

**Shortcut fields:** in Settings, every shortcut field is 220px wide (the text field width), so the column lines up. The default hands-free reads `Right Alt ×2 or Right Alt+Space`; a binding that may be cut shows whole in a tooltip. Each shortcut row holds, left to right, a reset icon button (lucide `rotate-ccw`, shown only when the shortcut is not its default; an empty place of the same size keeps the fields lined up), the field, and a switch. The field of a shortcut that is off uses the tertiary label color.

**Audio enhance:** the Audio group row after Input boost: "Audio enhance", the line "Makes speech clearer for the transcript.", and a segmented control Off | Auto | Custom (Auto by default: a low cut at 80 Hz and the bass 6 dB down). With Custom an "Adjust…" button follows the control, and picking Custom opens the tuning sheet. The sheet (the sheet rules above) has two pages.

- Tuning: the title "Audio enhance" and one secondary line; a Sample group with one row, "Sample" and a line that counts down an 8 s take ("Speak now. 5 s left") or names the kept take, then a record icon button (lucide `mic`, `square` while it records) and a play icon button (`play`, `square` while it plays), off until a take exists so the record button never moves; the level meter shows under the row while it records. The last take is kept on disk as the microphone gave it (`cache/enhance/sample.wav`) and each play runs it through the values as they are now. A values group follows: Low cut (Off to 200 Hz), Bass (-12 to +6 dB), Treble (-6 to +12 dB) as sliders with their readings, and Noise reduction as a switch with the line "Can make the text worse. Use it only in a noisy room." Moving a value makes the mode Custom. The buttons are Reset to Auto, Find best settings…, and Done.
- Find best settings: the title and one line; a group with the three lines to read (sounds such as v and w, th, r, and a number) and up to three Dictionary words, then the row "Your reading" with Record, which turns to Stop, and the meter while it records (30 s at most). Stop starts the test: the row says "Testing 12 of about 60" with a 120px progress bar and Cancel. The test transcribes the take with the current speech model and language, changing one value at a time (low cut, bass, treble, noise reduction, input boost), and keeps a change that gets fewer words wrong, until a round changes nothing. The results group has the five best settings, ties going to the one nearest Auto: the setting in a few words ("Low cut 80 Hz, bass -6 dB") over its score ("2 of 31 words wrong"), a play icon button, and Use, which sets Custom with those values and that boost and goes back to the tuning page. When every setting scores the same, one notice row says so and Auto stays. The buttons are Back, Test again (with results), and Done.

**Recording overlay (flow bar):** a compact dark pill in both appearances. It shows only while a dictation runs and while its result or notice is up. A new dictation starts at once, even while the last one still transcribes or shows its result: the bar shows Listening for the new one, the older one finishes behind it, and the texts go in in the order they were spoken. At rest nothing shows, unless the user turns on Show when idle. While listening it holds a round cross on the left (cancel), the waveform, and a round white tick on the right (finish). It has no language control: the language is set on Home. It appears on the screen that has the pointer and stays on that screen until it hides. A position the user drags it to is kept relative to the top-left corner of the screen. It grows out of a circle and fades in when it appears, eases between the sizes of its states, and shrinks and fades when it goes. With Reduce Motion every change shows at once.

## Do's and Don'ts

- Do keep the app utility-first, dense, and calm.
- Do use one group for each cluster of information.
- Do use hairlines between rows and nothing else for separation.
- Do follow the system Light or Dark setting.
- Do use `~/...` for home-relative storage paths in user-facing UI.
- Do use lucide-style monochrome icons.
- Don't use a color for decoration. Status color goes on status words and the attention icon only.
- Don't wrap a row to a second or third line. Truncate it, or move the help to a footer or a tooltip.
- Don't expose stack names such as Electron or React in product UI.
- Don't make the window resizable, zoomable, or full screen.
- Don't draw a third window button on Linux.
- Don't use gradients, blobs, or hero sections.
