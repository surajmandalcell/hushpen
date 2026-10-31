#!/usr/bin/env python3
"""Records a screen region at a fixed rate, and measures how much it moves.

  grab-region.py grab <x> <y> <w> <h> <fps> <out.raw> <stop file> [max s]
      Grabs the region of the root window until the stop file exists (or max s, default 60) and
      writes raw BGRX frames back to back, with "<frame> <unix ms>" lines in <out.raw>.times.
  grab-region.py diff <in.raw> <w> <h>
      Prints "<frame> <changed pixels> <sum of channel differences>" against the frame before.

The flow bar motion proof uses it: a fixed rate keeps frame-to-frame differences comparable.
"""
import os
import sys
import time


def grab(x, y, w, h, fps, out, stop, max_s):
    from Xlib import X, display

    root = display.Display().screen().root
    period = 1.0 / fps
    start = time.monotonic()
    n = 0
    with open(out, "wb") as frames, open(out + ".times", "w") as times:
        while not os.path.exists(stop) and time.monotonic() - start < max_s:
            due = start + n * period
            now = time.monotonic()
            if now < due:
                time.sleep(due - now)
            elif now - due > period:
                # A slow grab moves the schedule on instead of bursting to catch up.
                start = now - n * period
            image = root.get_image(x, y, w, h, X.ZPixmap, 0xFFFFFFFF)
            frames.write(image.data)
            times.write(f"{n} {int(time.time() * 1000)}\n")
            n += 1
    print(n)


def diff(path, w, h):
    size = w * h * 4
    with open(path, "rb") as frames:
        last = frames.read(size)
        n = 1
        while True:
            frame = frames.read(size)
            if len(frame) < size:
                break
            changed = total = 0
            for i in range(0, size, 4):
                d = (abs(frame[i] - last[i]) + abs(frame[i + 1] - last[i + 1])
                     + abs(frame[i + 2] - last[i + 2]))
                if d:
                    changed += 1
                    total += d
            print(n, changed, total)
            last = frame
            n += 1


if __name__ == "__main__":
    mode, args = sys.argv[1], sys.argv[2:]
    if mode == "grab":
        x, y, w, h = map(int, args[:4])
        grab(x, y, w, h, float(args[4]), args[5], args[6], float(args[7]) if len(args) > 7 else 60.0)
    elif mode == "diff":
        diff(args[0], int(args[1]), int(args[2]))
    else:
        sys.exit(__doc__)
