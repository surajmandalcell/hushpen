#!/usr/bin/env python3
"""usage: wavstat.py <file.wav> [--lead-ms N]

Prints frames, rate, channels, RMS and peak in dBFS, and silent=true|false for the whole file.
With --lead-ms it also splits the file at N ms and prints the peak of each part
(lead_peak_dbfs, body_peak_dbfs) and lead_silent=true when the lead is at or below -60 dBFS.
Expects 16-bit PCM.
"""
import math
import struct
import sys
import wave

SILENT_DBFS = -60.0


def dbfs(value):
    return 20 * math.log10(value) if value > 0 else float("-inf")


def peak(samples):
    return max((abs(x) for x in samples), default=0) / 32768.0


def main():
    path = sys.argv[1]
    lead_ms = int(sys.argv[sys.argv.index("--lead-ms") + 1]) if "--lead-ms" in sys.argv else None
    with wave.open(path, "rb") as reader:
        frames, rate, channels, width = (
            reader.getnframes(), reader.getframerate(), reader.getnchannels(), reader.getsampwidth(),
        )
        data = reader.readframes(frames)
    assert width == 2, "expects 16-bit PCM"
    samples = struct.unpack(f"<{len(data) // 2}h", data)
    rms = math.sqrt(sum(x * x for x in samples) / max(len(samples), 1)) / 32768.0
    print(
        f"frames={frames} rate={rate} ch={channels} secs={frames / rate:.2f} "
        f"rms_dbfs={dbfs(rms):.1f} peak_dbfs={dbfs(peak(samples)):.1f} "
        f"silent={str(dbfs(rms) <= SILENT_DBFS).lower()}"
    )
    if lead_ms is not None:
        split = rate * lead_ms // 1000 * channels
        lead_peak, body_peak = dbfs(peak(samples[:split])), dbfs(peak(samples[split:]))
        print(
            f"lead_ms={lead_ms} lead_peak_dbfs={lead_peak:.1f} body_peak_dbfs={body_peak:.1f} "
            f"lead_silent={str(lead_peak <= SILENT_DBFS).lower()}"
        )


if __name__ == "__main__":
    main()
