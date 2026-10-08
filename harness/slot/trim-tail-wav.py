#!/usr/bin/env python3
"""usage: trim-tail-wav.py <in.wav> <out.wav> [threshold=300]

Writes a copy of a PCM16 WAV cut right after its last sample above the threshold, so the file
ends on speech with no trailing silence. Used to check that stopping a recording keeps the tail.
"""
import array
import sys
import wave


def main():
    src, dst = sys.argv[1], sys.argv[2]
    threshold = int(sys.argv[3]) if len(sys.argv) > 3 else 300
    with wave.open(src, "rb") as reader:
        params = reader.getparams()
        if params.sampwidth != 2 or params.nchannels != 1:
            sys.exit("trim-tail-wav: only 16-bit mono WAV is supported")
        samples = array.array("h")
        samples.frombytes(reader.readframes(params.nframes))
    last = max((i for i, value in enumerate(samples) if abs(value) > threshold), default=0)
    kept = samples[: last + 1]
    with wave.open(dst, "wb") as writer:
        writer.setparams(params)
        writer.writeframes(kept.tobytes())
    print(f"trimmed={dst} kept_s={len(kept) / params.framerate:.3f} cut_s={(len(samples) - len(kept)) / params.framerate:.3f}")


if __name__ == "__main__":
    main()
