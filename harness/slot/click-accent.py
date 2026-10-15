#!/usr/bin/env python3
"""Clicks the accent-colored default button of the update banner in a window.

usage: click-accent.py <window id> [--dry-run]

A release build has no test hook, so the self-update suite cannot ask for the bounds of the
banner button. The banner sits under the 52 px title bar and its one default button is the only
accent-blue area (#0A84FF dark, #007AFF light, a little darker on hover) in the top 110 px of
the window, so the button is the bounding box of those pixels. Prints the click position and
exits 1 when no button is found.
"""
import subprocess
import sys

# The button is darker while the pointer rests on it, and the app keeps that hover after a
# restart, so the match is a range of blues, not one color.
BAND = (52, 110)


def read_ppm(data):
    fields, pos = [], 0
    while len(fields) < 4:
        while data[pos : pos + 1].isspace():
            pos += 1
        if data[pos : pos + 1] == b"#":
            while data[pos : pos + 1] != b"\n":
                pos += 1
            continue
        end = pos
        while not data[end : end + 1].isspace():
            end += 1
        fields.append(data[pos:end])
        pos = end
    pos += 1
    return int(fields[1]), int(fields[2]), data[pos:]


def main():
    wid = sys.argv[1]
    info = subprocess.run(["xwininfo", "-id", wid], capture_output=True, text=True, check=True).stdout
    origin = {}
    for line in info.splitlines():
        if "Absolute upper-left X" in line:
            origin["x"] = int(line.split()[-1])
        if "Absolute upper-left Y" in line:
            origin["y"] = int(line.split()[-1])
    shot = subprocess.run(["import", "-window", wid, "-depth", "8", "ppm:-"], capture_output=True, check=True).stdout
    width, height, pixels = read_ppm(shot)
    hits = []
    for y in range(BAND[0], min(BAND[1], height)):
        row = pixels[y * width * 3 : (y + 1) * width * 3]
        for x in range(width):
            r, g, b = row[x * 3], row[x * 3 + 1], row[x * 3 + 2]
            if r <= 40 and 90 <= g <= 150 and b >= 190 and b - g >= 70:
                hits.append((x, y))
    if len(hits) < 200:
        print(f"no accent button in the banner band ({len(hits)} pixels)", file=sys.stderr)
        return 1
    xs, ys = [h[0] for h in hits], [h[1] for h in hits]
    cx, cy = (min(xs) + max(xs)) // 2 + origin["x"], (min(ys) + max(ys)) // 2 + origin["y"]
    print(f"button {min(xs)}-{max(xs)} x {min(ys)}-{max(ys)}, click at {cx} {cy}")
    if "--dry-run" not in sys.argv:
        subprocess.run(["xdotool", "mousemove", str(cx), str(cy), "click", "1"], check=True)
    return 0


sys.exit(main())
