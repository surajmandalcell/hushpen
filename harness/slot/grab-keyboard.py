#!/usr/bin/env python3
"""Holds an active keyboard grab on the root window, like a passphrase dialog, until killed."""
import sys
import time

from Xlib import X, display

d = display.Display()
status = d.screen().root.grab_keyboard(True, X.GrabModeAsync, X.GrabModeAsync, X.CurrentTime)
d.sync()
print(f"grab status {status}", flush=True)
if status != X.GrabSuccess:
    sys.exit(1)
time.sleep(3600)
