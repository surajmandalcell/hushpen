"""Simple microphone recorder writing PCM16 WAV."""

from __future__ import annotations
import queue, threading, wave
from pathlib import Path
from typing import Optional
import sounddevice as sd  # type: ignore
import numpy as np


class AudioRecorder:
    def __init__(self, samplerate: int = 16000, channels: int = 1):
        self.samplerate = samplerate
        self.channels = channels
        self._q: queue.Queue = queue.Queue()
        self._stop = threading.Event()
        self._stream = None
        self.file_path: Optional[Path] = None

    def _callback(self, indata, frames, time, status):  # pragma: no cover
        if status:
            pass
        self._q.put(indata.copy())

    def start(self, file_path: Path):  # pragma: no cover
        self.file_path = file_path
        self._stop.clear()
        self._stream = sd.InputStream(
            samplerate=self.samplerate, channels=self.channels, callback=self._callback
        )
        self._stream.start()
        threading.Thread(target=self._writer, daemon=True).start()

    def _writer(self):  # pragma: no cover
        if not self.file_path:
            return
        with wave.open(str(self.file_path), "wb") as wf:
            wf.setnchannels(self.channels)
            wf.setsampwidth(2)
            wf.setframerate(self.samplerate)
            while not self._stop.is_set() or not self._q.empty():
                try:
                    data = self._q.get(timeout=0.1)
                except queue.Empty:
                    continue
                wf.writeframes((data * 32767).astype(np.int16).tobytes())

    def stop(self) -> Optional[Path]:  # pragma: no cover
        self._stop.set()
        if self._stream:
            self._stream.stop()
            self._stream.close()
        return self.file_path
