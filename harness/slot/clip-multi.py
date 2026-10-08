#!/usr/bin/env python3
"""Owns the CLIPBOARD with rich text: text/html "<b>OLD</b>" plus the plain text "OLD".

It answers selection requests by hand with python-xlib (PyGObject cannot offer several targets
from one owner). Runs until killed.
"""
from Xlib import X, Xatom, display
from Xlib.protocol import event

HTML = b"<b>OLD</b>"
PLAIN = b"OLD"

disp = display.Display()
screen = disp.screen()
window = screen.root.create_window(0, 0, 1, 1, 0, screen.root_depth)
CLIPBOARD = disp.intern_atom("CLIPBOARD")
TARGETS = disp.intern_atom("TARGETS")
TEXT_HTML = disp.intern_atom("text/html")
UTF8 = disp.intern_atom("UTF8_STRING")
STRING = Xatom.STRING

data = {TEXT_HTML: HTML, UTF8: PLAIN, STRING: PLAIN}
window.set_selection_owner(CLIPBOARD, X.CurrentTime)
disp.flush()

while True:
    e = disp.next_event()
    if e.type == X.SelectionClear:
        continue
    if e.type != X.SelectionRequest:
        continue
    prop = e.property if e.property != X.NONE else e.target
    served = X.NONE
    if e.target == TARGETS:
        e.requestor.change_property(prop, Xatom.ATOM, 32, [TARGETS, TEXT_HTML, UTF8, STRING])
        served = prop
    elif e.target in data:
        e.requestor.change_property(prop, e.target, 8, data[e.target])
        served = prop
    notify = event.SelectionNotify(
        time=e.time, requestor=e.requestor, selection=e.selection, target=e.target, property=served
    )
    e.requestor.send_event(notify)
    disp.flush()
