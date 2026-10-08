#!/usr/bin/env python3
"""A focusable window that never reads the clipboard: a paste into it gets no receipt."""
import gi

gi.require_version("Gtk", "3.0")
from gi.repository import Gtk  # noqa: E402

window = Gtk.Window(title="hushpen-idle-target")
window.set_default_size(360, 60)
window.move(40, 560)
window.add(Gtk.Label(label="idle target"))
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
