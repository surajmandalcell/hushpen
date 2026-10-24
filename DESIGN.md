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

The app icon follows the selected Ink Slate direction while staying recognizable as Hushpen: a centered speech-loop mark, a simple `#20272d` to `#10171d` to `#04070a` background, and no text. The production source is `assets/hushpen-app-icon.svg`, with raster exports in `assets/hushpen-app-icon.png`, `assets/hushpen-app-icon.icns`, and `assets/linux-icons/*.png`. About draws `assets/hushpen-app-icon-192.png` at 96px: the tile of the SVG without its margin, with the 16px app-icon corner, at 2x so a Retina screen gets exact pixels. The macOS menu bar and the Linux tray glyphs use PNG assets.

Use a 1024x1024 source canvas and keep the rounded tile inset inside the canvas so the Dock footprint matches standard macOS icon sizing. The visual construction stays layer-friendly: simple background plane, foreground ribbon mark, subtle glass rim, and appearance variants that still read in light or dark contexts.

## Appearance

The app follows the system appearance, Light or Dark, and changes live when the system setting changes, with no restart. Every view reads its colors from one palette for the current appearance. If the appearance is unknown (Linux without a desktop setting), the app uses Dark.

The flow bar pill is the one exception. It stays a dark HUD (`#2B2B2B`, light text) in both appearances, because it floats over other apps.

The macOS menu bar item is the glyph alone, with no name beside it. It is a template image (`assets/hushpen-menubar.png`, 22 by 16 points at 2x), so macOS tints it for Light, Dark, and a highlighted menu bar. Linux panels get the black or the white tray glyph for their appearance.

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
- **Accent (`#0A84FF` Dark, `#007AFF` Light):** the default button, a switch that is on, the focus ring, the selection focus, and links. It is the only accent.
- **Status colors, on status words only:** success (`#30D158` Dark, `#34C759` Light) for ready, error (`#FF453A` Dark, `#FF3B30` Light) for a failure.
- **Status icon, for a row that needs the user:** warning (`#FF9F0A` Dark, `#FF9500` Light) on an exclamation icon (lucide `circle-alert`, 16px) with the tooltip "Needs attention" and no word. The icon is the only place the warning color appears. Its label in the accessibility tree is "Needs attention".
- **Scrim (black 40% Dark, black 20% Light):** dims the window behind a sheet.
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

Onboarding uses the same groups and rows. Each permission row has its icon, name, status (the word Ready, or the attention icon), and buttons on one line. On macOS the buttons are Allow (the default button), then Open Settings. The description is one short secondary line.

Long local paths show as `~/...` when they are under the user home directory.

## Elevation & Depth

Depth is tonal. There are no shadows on normal content. Shadows are only for dropdowns and menus.

- Hairlines (one device pixel, separator color) sit only between rows inside a group, and span the full width of the group on every page.
- A group has a 0.5px stroke in the separator color. There is no toolbar line and no other stray border.
- Hover and selection use the neutral selection fill.
- The focus ring is 2px in the accent color.
- A sheet floats centered over the scrim: 460px wide (`sheet-width`), the content color, the window radius, and a hairline edge. It has no shadow.

## Shapes

- **Groups:** 8px radius.
- **Push buttons:** 5px radius, 22px high.
- **Selection and list rows:** 6px radius.
- **App icon tile:** 16px radius.
- **Window on Linux with a compositor:** 10px radius, like a macOS window.
- **Window buttons on Linux:** fully round.

## Components

**Sidebar:** 7 rows in this order: Home, History, Dictionary, Import, Models, Settings, About. Icons are monochrome, 16px, in the secondary label color. The selected row uses the selection fill and the label color for its icon. There are no colored tiles. A focused row shows the accent focus ring.

**Toolbar:** the view title (13px semibold, label color) on the left of the title bar area. On Settings the search field sits at the right end, 300px wide, 24px from the window edge, centered on the chrome line. It has no bottom line.

**Settings:** one scrolling list, grouped like macOS System Settings. Each section is a heading above its group, with an optional footer line under it, in this order: General, Shortcuts, Audio, Cleanup (with AI cleanup and the endpoint form), Engine, Flow bar, Storage, Updates. Typing in the search field keeps only the matching rows under their headings; a row's label, its section, and its aliases all match. With no match the list shows `No settings match "<query>"` and a Clear button. Cmd+F (Ctrl+F on Linux) focuses the field. Esc clears it, and a second Esc leaves it. A new query scrolls the list to the top. The query is never saved. Numeric settings use a slider (160px track, 5% steps, arrow keys, Home and End) or a pop-up of fixed choices inside the registry range.

**Groups and rows:** as in Layout. A row that is a control stays one line: the label on the left, the control on the right. A notice row has a 16px icon in the secondary label color (info, or the exclamation for something the user has to act on) and one line of text; a failed check shows its text in the error color. Every view builds its rows from the shared row builders, so one row kind looks the same on every page.

**Home:** the Dictation group (status, Record, the transcript, language) above the Microphone group (test, level meter, microphone list). The transcript, or the placeholder before it, is centered in its block with 24px at the sides and 16px above and below; a long transcript keeps every line centered.

**About:** a centered header, a facts group, a links group, and a footer line. The header holds the app icon (96px, 16px radius), "Hushpen" in the title style, the line "Private dictation that stays on this Mac" ("on this computer" on Linux), the build line in the secondary style (`Version 2.0.0 (1a2b3c4, 2026-05-15)`, with `-dev` after the version in a debug build), and a Check for Updates button with what the check found on one secondary line under it. The facts are Version, Engine (whisper.cpp and the model in use), and Data folder (`~/...`, with a Show button). The links are Website, GitHub, Report an issue, Open-source licenses, and Open log folder, each with the external-link glyph on the right. The footer is one centered secondary line: "Copyright (c) 2026 Suraj Mandal. Licensed under AGPL-3.0-or-later." The Engine fact is the one place the UI names a component of the stack.

**Models:** one section for each model kind, in this order: Speech recognition, Text cleanup, Voices. Each section is a heading above its group, with one footer line. Text cleanup lists only the models this machine is offered. A row holds the name, then the word "Recommended" in the secondary style for the suggested model of its kind, and one secondary line that says what the model is for. On the right, lightest to heaviest: the size, the status word, the details button (lucide `info`, 16px, in the secondary label color, tooltip "Details"), then Download or Cancel, Use (speech models only), and Delete. The size, status, and button columns keep fixed widths, so they line up from row to row.

**Details sheet:** the details button opens a sheet for that model. It shows the name, the description, and what the model is good for; a facts group (languages, speed and accuracy as a word with "N of 5", memory, size on disk, license, status); and a files group (the source page as a link row, the folder as a `~/...` path with Show in Finder or Show in Files, and the SHA-256 with Copy). Done closes it, and so does Esc. Tab stays inside the sheet, and closing it returns focus to the details button.

**Buttons:** 22px high, 5px radius. The default button has the accent fill and white text. Other buttons use the neutral control fill. A focused button shows the accent focus ring. A disabled button uses the tertiary label color.

**Text inputs:** the button height (22px) and radius (5px), with the control fill, so an input in one row with buttons aligns with them. The inner padding keeps the 13px text clear of the edges.

**Switches, sliders, and selection focus:** use the accent.

**Recording overlay (flow bar):** a compact dark pill in both appearances. It shows only while a dictation runs and while its result or notice is up. At rest nothing shows, unless the user turns on Show when idle. While listening it holds a round cross on the left (cancel), the waveform, and a round white tick on the right (finish). It has no language control: the language is set on Home. It grows out of a circle and fades in when it appears, eases between the sizes of its states, and shrinks and fades when it goes. With Reduce Motion every change shows at once.

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
