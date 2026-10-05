#!/usr/bin/env python3
"""GTK3 paste target: one Entry. On Enter it prints GTK_ENTRY=<text> and writes /out/gtk.txt."""
import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gtk  # noqa: E402


def on_activate(entry):
    text = entry.get_text()
    print(f"GTK_ENTRY={text}", flush=True)
    with open("/out/gtk.txt", "w", encoding="utf-8") as out:
        out.write(text)
    entry.set_text("")


window = Gtk.Window(title="hushpen-gtk-target")
window.set_default_size(360, 60)
window.move(40, 560)
entry = Gtk.Entry()
entry.connect("activate", on_activate)
window.add(entry)
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
