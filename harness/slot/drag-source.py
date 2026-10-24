#!/usr/bin/env python3
"""GTK3 XDND drag source: a small window titled "drag-source" whose label drags the files named
on the command line as text/uri-list, like a file manager does. slot/xdnd-drag.sh presses the
pointer on it with xdotool and moves it onto the Hushpen window, so the drop arrives through XDND."""
import sys
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gtk  # noqa: E402

uris = [Path(path).resolve().as_uri() for path in sys.argv[1:]]


def on_drag_data_get(_widget, _context, data, _info, _time):
    data.set_uris(uris)


window = Gtk.Window(title="drag-source")
window.set_default_size(200, 100)
window.move(1060, 680)
box = Gtk.EventBox()
box.add(Gtk.Label(label=f"{len(uris)} file(s)"))
box.drag_source_set(
    Gdk.ModifierType.BUTTON1_MASK,
    [Gtk.TargetEntry.new("text/uri-list", 0, 0)],
    Gdk.DragAction.COPY,
)
box.connect("drag-data-get", on_drag_data_get)
window.add(box)
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
