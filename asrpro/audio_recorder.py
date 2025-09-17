"""Simple microphone recorder writing PCM16 WAV."""

from __future__ import annotations
import queue, threading, wave
import platform
import subprocess
from pathlib import Path
from typing import Optional, List, Dict, Any
import sounddevice as sd  # type: ignore
import numpy as np


class AudioRecorder:
    def __init__(self, samplerate: int = 16000, channels: int = 1, device: Optional[int] = None):
        self.samplerate = samplerate
        self.channels = channels
        self.device = device  # Allow specific device selection
        self._q: queue.Queue = queue.Queue()
        self._stop = threading.Event()
        self._stream = None
        self.file_path: Optional[Path] = None

        # Check microphone permissions on macOS
        if platform.system() == 'Darwin':
            self._check_macos_microphone_permission()

    def _callback(self, indata, frames, time, status):  # pragma: no cover
        if status:
            pass
        self._q.put(indata.copy())

    def start(self, file_path: Path):  # pragma: no cover
        self.file_path = file_path
        self._stop.clear()

        try:
            self._stream = sd.InputStream(
                device=self.device,
                samplerate=self.samplerate,
                channels=self.channels,
                callback=self._callback
            )
            self._stream.start()
            threading.Thread(target=self._writer, daemon=True).start()
        except sd.PortAudioError as e:
            if platform.system() == 'Darwin' and 'Input overflowed' not in str(e):
                # Check if this is specifically a permission issue
                error_msg = str(e).lower()
                permission_keywords = ['permission', 'denied', 'unauthorized', 'access', 'restricted']
                if any(keyword in error_msg for keyword in permission_keywords):
                    # Likely a permission issue on macOS
                    print(f"[AudioRecorder] Failed to start recording: {e}")
                    print("[AudioRecorder] Please grant microphone access in System Settings > Privacy & Security > Microphone")
                    self._request_macos_microphone_permission()
                    raise RuntimeError("Microphone access denied. Please grant permission and restart.") from e
                else:
                    # Other PortAudio errors (like device not found) should be raised as-is
                    raise
            else:
                raise

    def _writer(self):  # pragma: no cover
        if not self.file_path:
            return
        try:
            # Create parent directories if they don't exist
            self.file_path.parent.mkdir(parents=True, exist_ok=True)

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
        except (OSError, FileNotFoundError) as e:
            print(f"[AudioRecorder] Failed to write audio file: {e}")
            # Don't raise the exception - handle it gracefully
        except Exception as e:
            print(f"[AudioRecorder] Unexpected error in writer: {e}")
            # Don't raise the exception - handle it gracefully

    def stop(self) -> Optional[Path]:  # pragma: no cover
        self._stop.set()
        if self._stream:
            self._stream.stop()
            self._stream.close()
        return self.file_path

    def _check_macos_microphone_permission(self) -> bool:
        """Check if microphone permission is granted on macOS."""
        if platform.system() != 'Darwin':
            return True

        try:
            # Try to query audio devices - this will fail if no permission
            devices = sd.query_devices()
            # Check if we have any input devices
            input_devices = []
            for d in devices:
                # Handle incomplete device information gracefully
                if isinstance(d, dict) and 'max_input_channels' in d:
                    if d['max_input_channels'] > 0:
                        input_devices.append(d)
                elif hasattr(d, 'max_input_channels'):
                    if d.max_input_channels > 0:
                        input_devices.append(d)

            if not input_devices:
                print("[AudioRecorder] Warning: No input devices found. Microphone permission may be denied.")
                return False
            return True
        except Exception as e:
            print(f"[AudioRecorder] Failed to query audio devices: {e}")
            return False

    def _request_macos_microphone_permission(self):
        """Request microphone permission on macOS."""
        if platform.system() != 'Darwin':
            return

        try:
            # Open System Settings to microphone privacy pane
            subprocess.run([
                'osascript', '-e',
                'tell application "System Settings" to reveal anchor "Privacy_Microphone" of pane id "com.apple.preference.security"'
            ])
            subprocess.run(['osascript', '-e', 'tell application "System Settings" to activate'])
        except Exception as e:
            print(f"[AudioRecorder] Failed to open System Settings: {e}")

    @staticmethod
    def list_devices() -> List[Dict[str, Any]]:
        """List all available audio input devices."""
        try:
            devices = sd.query_devices()
            input_devices = []
            for i, device in enumerate(devices):
                # Handle device as dictionary with proper type checking
                if isinstance(device, dict):
                    max_input_channels = device.get('max_input_channels', 0)
                    name = device.get('name', 'Unknown Device')
                    default_samplerate = device.get('default_samplerate', 44100)

                    if max_input_channels > 0:
                        input_devices.append({
                            'index': i,
                            'name': str(name),
                            'channels': int(max_input_channels),
                            'sample_rate': float(default_samplerate),
                            'is_default': i == sd.default.device[0]
                        })
                else:
                    # Handle device as object with attributes
                    max_input_channels = getattr(device, 'max_input_channels', 0)
                    name = getattr(device, 'name', 'Unknown Device')
                    default_samplerate = getattr(device, 'default_samplerate', 44100)

                    if max_input_channels > 0:
                        input_devices.append({
                            'index': i,
                            'name': str(name),
                            'channels': int(max_input_channels),
                            'sample_rate': float(default_samplerate),
                            'is_default': i == sd.default.device[0]
                        })
            return input_devices
        except Exception as e:
            print(f"[AudioRecorder] Failed to list devices: {e}")
            return []
