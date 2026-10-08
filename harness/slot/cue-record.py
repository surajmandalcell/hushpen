#!/usr/bin/env python3
"""usage: cue-record.py <pulse source> <out.tsv>

Records a PulseAudio source with parec until SIGTERM. Writes one line for each 20 ms window:
"<wall clock ms when the window arrived>\t<peak in dBFS>". The clock is the one the test hook
uses for its events (milliseconds since the epoch), so a burst can be matched to a cue event.
"""
import math
import signal
import struct
import subprocess
import sys
import time

RATE = 16000
WINDOW = RATE // 50

source, out_path = sys.argv[1], sys.argv[2]
parec = subprocess.Popen(
    ["parec", "-d", source, "--format=s16le", f"--rate={RATE}", "--channels=1", "--latency-msec=20"],
    stdout=subprocess.PIPE,
)
signal.signal(signal.SIGTERM, lambda *_: parec.terminate())

with open(out_path, "w", encoding="utf-8") as out:
    while True:
        data = parec.stdout.read(2 * WINDOW)
        if len(data) < 2 * WINDOW:
            break
        samples = struct.unpack(f"<{WINDOW}h", data)
        peak = max(abs(s) for s in samples) / 32768.0
        db = 20 * math.log10(peak) if peak > 0 else -120.0
        out.write(f"{int(time.time() * 1000)}\t{db:.1f}\n")
        out.flush()
