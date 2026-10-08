#!/usr/bin/env python3
"""Another X client for the shortcut checks.

  grab-chord.py hold ctrl+alt+j    grabs the chord on the root window, like a window manager
                                   hotkey, prints "grabbed", and keeps it until killed
  grab-chord.py probe ctrl+alt+v   tries to grab it and lets go: prints "free" or "taken"
"""
import sys
import time

from Xlib import X, XK, display, error

MODIFIERS = {"ctrl": X.ControlMask, "alt": X.Mod1Mask, "shift": X.ShiftMask, "super": X.Mod4Mask}
# Caps Lock and Num Lock change the modifier state of the same key press.
LOCKS = [0, X.LockMask, X.Mod2Mask, X.LockMask | X.Mod2Mask]


def parse(chord):
    *names, key = chord.lower().split("+")
    mask = 0
    for name in names:
        mask |= MODIFIERS[name]
    return mask, key


def grab(d, mask, key):
    code = d.keysym_to_keycode(XK.string_to_keysym(key))
    root = d.screen().root
    taken = False
    for lock in LOCKS:
        catch = error.CatchError(error.BadAccess)
        root.grab_key(code, mask | lock, True, X.GrabModeAsync, X.GrabModeAsync, onerror=catch)
        d.sync()
        taken = taken or catch.get_error() is not None
    return code, taken


def main():
    mode, chord = sys.argv[1], sys.argv[2]
    d = display.Display()
    mask, key = parse(chord)
    code, taken = grab(d, mask, key)
    if mode == "probe":
        print("taken" if taken else "free", flush=True)
        sys.exit(1 if taken else 0)
    if taken:
        print("taken", flush=True)
        sys.exit(1)
    print("grabbed", flush=True)
    time.sleep(3600)


main()
