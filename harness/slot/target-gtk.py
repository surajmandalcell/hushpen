#!/usr/bin/env python3
"""GTK3 paste target: one Entry. On Enter it prints GTK_ENTRY=<text> and writes /out/gtk.txt.

GTK3 entries have no undo, so Ctrl+Z steps back to the text before the last change, as an
editor's entry does. A paste is one step. Every Ctrl+Z that arrives is appended to
/out/gtk-undo.log, which a suite reads to see whether the app took the key. /out/gtk-live.txt
always holds what the entry holds now, so a suite can read it without pressing Enter."""
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gtk  # noqa: E402

steps = [""]
restoring = False


def on_changed(entry):
    text = entry.get_text()
    with open("/out/gtk-live.txt", "w", encoding="utf-8") as live:
        live.write(text)
    if restoring:
        return
    if text != steps[-1]:
        steps.append(text)


def set_text(entry, text):
    global restoring
    restoring = True
    entry.set_text(text)
    entry.set_position(-1)
    restoring = False


def on_activate(entry):
    text = entry.get_text()
    print(f"GTK_ENTRY={text}", flush=True)
    with open("/out/gtk.txt", "w", encoding="utf-8") as out:
        out.write(text)
    set_text(entry, "")
    steps[:] = [""]


def on_key(entry, event):
    ctrl = event.state & Gdk.ModifierType.CONTROL_MASK
    if not ctrl or event.keyval not in (Gdk.KEY_z, Gdk.KEY_Z):
        return False
    with open("/out/gtk-undo.log", "a", encoding="utf-8") as log:
        log.write("ctrl+z\n")
    if len(steps) > 1:
        steps.pop()
        set_text(entry, steps[-1])
    return True


window = Gtk.Window(title="hushpen-gtk-target")
window.set_default_size(360, 60)
window.move(40, 560)
entry = Gtk.Entry()
entry.connect("activate", on_activate)
entry.connect("changed", on_changed)
entry.connect("key-press-event", on_key)
window.add(entry)
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
