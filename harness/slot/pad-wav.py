#!/usr/bin/env python3
"""usage: pad-wav.py <in.wav> <out.wav> [lead_ms=1000]

Writes a copy of a PCM WAV with lead_ms of digital silence in front. Capture loses the first
moments of audio, so every fixture the harness feeds to the virtual microphone starts with 1 s of
silence (the first word of speech-short.wav was lost without it).
"""
import sys
import wave


def main():
    src, dst = sys.argv[1], sys.argv[2]
    lead_ms = int(sys.argv[3]) if len(sys.argv) > 3 else 1000
    with wave.open(src, "rb") as reader:
        params = reader.getparams()
        body = reader.readframes(params.nframes)
    # 8-bit WAV is unsigned, so its silence is 0x80; wider samples are signed, so it is 0.
    silent_byte = b"\x80" if params.sampwidth == 1 else b"\x00"
    lead_frames = params.framerate * lead_ms // 1000
    lead = silent_byte * (lead_frames * params.nchannels * params.sampwidth)
    with wave.open(dst, "wb") as writer:
        writer.setparams(params)
        writer.writeframes(lead + body)
    seconds = (lead_frames + params.nframes) / params.framerate
    print(
        f"padded={dst} lead_ms={lead_ms} rate={params.framerate} channels={params.nchannels} "
        f"duration_s={seconds:.2f}"
    )


if __name__ == "__main__":
    main()
