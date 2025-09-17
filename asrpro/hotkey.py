"""Global toggle hotkey using pynput."""

from __future__ import annotations
import threading
import platform
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Callable, Optional

try:
    from pynput import keyboard  # type: ignore
except Exception:  # pragma: no cover
    keyboard = None  # type: ignore

try:
    import pyperclip  # type: ignore
except ImportError:
    pyperclip = None  # type: ignore

from .config import config
from .audio_recorder import AudioRecorder
from .model_manager import ModelManager

DEFAULT_HOTKEY = "<ctrl>+<alt>+t"


def load_hotkey():
    return config.get_hotkey()


def save_hotkey(hk: str):
    config.set_hotkey(hk)


class ToggleHotkey:
    def __init__(self, on_toggle: Callable[[bool], None], model_manager: Optional[ModelManager] = None):
        self.on_toggle = on_toggle
        self.is_active = False
        self.is_recording = False
        self.listener: Optional[keyboard.GlobalHotKeys] = None  # type: ignore
        self.esc_listener: Optional[keyboard.Listener] = None  # type: ignore
        self.hotkey = load_hotkey()
        
        # Recording components
        self.recorder = AudioRecorder()
        self.model_manager = model_manager or ModelManager()
        self.current_recording_path = None
        
        # Ensure Whisper Medium is loaded
        self._ensure_model_loaded()

    def _build_listener(self):  # pragma: no cover
        if keyboard is None:
            return
        self.listener = keyboard.GlobalHotKeys({self.hotkey: self._toggle})

    def start(self):  # pragma: no cover
        if keyboard is None:
            return
            
        # Check for accessibility permissions on macOS
        if platform.system() == 'Darwin':
            if not self._check_macos_accessibility():
                print("[Hotkey] Warning: Accessibility permissions not granted")
                print("[Hotkey] Please grant accessibility access in System Settings > Privacy & Security > Accessibility")
                self._request_macos_accessibility()
                return
                
        self._build_listener()
        if self.listener:
            threading.Thread(target=self.listener.run, daemon=True).start()
    
    def _check_macos_accessibility(self) -> bool:
        """Check if we have accessibility permissions on macOS."""
        if platform.system() != 'Darwin':
            return True
        
        try:
            # Try to import the macOS-specific module
            import subprocess
            # This will fail if we don't have accessibility permissions
            result = subprocess.run(
                ['osascript', '-e', 'tell application "System Events" to keystroke ""'],
                capture_output=True,
                timeout=1
            )
            return result.returncode == 0
        except Exception:
            return False
    
    def _request_macos_accessibility(self):
        """Request accessibility permissions on macOS."""
        if platform.system() != 'Darwin':
            return
            
        try:
            # Open System Settings to accessibility pane
            subprocess.run([
                'osascript', '-e',
                'tell application "System Settings" to reveal anchor "Privacy_Accessibility" of pane id "com.apple.preference.security"'
            ])
            subprocess.run(['osascript', '-e', 'tell application "System Settings" to activate'])
        except Exception:
            pass

    def set_hotkey(self, hotkey: str):
        self.hotkey = hotkey
        save_hotkey(hotkey)
        if self.listener:
            try:
                self.listener.stop()
            except Exception:
                pass
        self.start()

    def _toggle(self):  # pragma: no cover
        """Handle hotkey press - start/stop recording."""
        if not self.is_recording:
            # Start recording
            self.start_recording()
        else:
            # Stop recording and transcribe
            self.stop_and_transcribe()
    
    def start_recording(self):  # pragma: no cover
        """Start recording audio."""
        try:
            # Create temp file for recording
            temp_file = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
            self.current_recording_path = Path(temp_file.name)
            temp_file.close()
            
            # Start recording
            self.recorder.start(self.current_recording_path)
            self.is_recording = True
            
            # Notify UI (show overlay)
            self.on_toggle(True)
            
            # Start ESC listener
            if keyboard:
                self.esc_listener = keyboard.Listener(on_press=self._on_key_press)
                self.esc_listener.start()
            
            print("[Hotkey] Recording started")
        except Exception as e:
            print(f"[Hotkey] Failed to start recording: {e}")
            self.is_recording = False
            self.on_toggle(False)
    
    def stop_and_transcribe(self):  # pragma: no cover
        """Stop recording and transcribe the audio."""
        if not self.is_recording:
            return
        
        try:
            # Stop ESC listener
            if self.esc_listener:
                self.esc_listener.stop()
                self.esc_listener = None
            
            # Stop recording
            self.recorder.stop()
            self.is_recording = False
            
            # Update UI to show transcribing
            # This will be handled by the callback
            
            print("[Hotkey] Recording stopped, transcribing...")
            
            # Transcribe in background
            threading.Thread(target=self._transcribe_and_paste, daemon=True).start()
            
        except Exception as e:
            print(f"[Hotkey] Failed to stop recording: {e}")
            self.is_recording = False
            self.on_toggle(False)
    
    def cancel_recording(self):  # pragma: no cover
        """Cancel the current recording."""
        if not self.is_recording:
            return
        
        try:
            # Stop ESC listener
            if self.esc_listener:
                self.esc_listener.stop()
                self.esc_listener = None
            
            # Stop recording
            self.recorder.stop()
            self.is_recording = False
            
            # Clean up temp file
            if self.current_recording_path and self.current_recording_path.exists():
                self.current_recording_path.unlink()
            
            print("[Hotkey] Recording cancelled")
            
            # Notify UI (hide overlay)
            self.on_toggle(False)
            
        except Exception as e:
            print(f"[Hotkey] Failed to cancel recording: {e}")
            self.is_recording = False
            self.on_toggle(False)
    
    def _on_key_press(self, key):  # pragma: no cover
        """Handle ESC key press during recording."""
        try:
            if key == keyboard.Key.esc:
                self.cancel_recording()
                return False  # Stop listener
        except Exception:
            pass
    
    def _transcribe_and_paste(self):  # pragma: no cover
        """Transcribe the recorded audio and paste the result."""
        try:
            if not self.current_recording_path or not self.current_recording_path.exists():
                print("[Hotkey] No recording file found")
                self.on_toggle(False)
                return
            
            # Transcribe using Whisper Medium
            result = self.model_manager.transcribe(
                str(self.current_recording_path),
                model_id="whisper-medium-onnx"
            )
            
            # Extract text from segments
            if isinstance(result, list):
                text = " ".join(seg.get("text", "") for seg in result)
            else:
                text = str(result)
            
            # Clean up text
            text = text.strip()
            
            if text:
                # Copy to clipboard and paste
                if pyperclip:
                    pyperclip.copy(text)
                    print(f"[Hotkey] Transcribed: {text}")
                    
                    # Simulate paste on macOS
                    if platform.system() == 'Darwin':
                        # Use AppleScript to paste
                        subprocess.run([
                            'osascript', '-e',
                            'tell application "System Events" to keystroke "v" using command down'
                        ])
                    # For other platforms, you'd need platform-specific paste
                else:
                    print(f"[Hotkey] Transcribed (no clipboard): {text}")
            else:
                print("[Hotkey] No text transcribed")
            
            # Clean up temp file
            if self.current_recording_path.exists():
                self.current_recording_path.unlink()
            
        except Exception as e:
            print(f"[Hotkey] Transcription failed: {e}")
        finally:
            # Hide overlay
            self.on_toggle(False)
    
    def _ensure_model_loaded(self):  # pragma: no cover
        """Ensure Whisper Medium model is loaded."""
        try:
            # Load Whisper Medium if not already loaded
            if self.model_manager.current_id != "whisper-medium-onnx":
                print("[Hotkey] Loading Whisper Medium model...")
                self.model_manager.load("whisper-medium-onnx")
                print("[Hotkey] Model loaded")
        except Exception as e:
            print(f"[Hotkey] Failed to load model: {e}")
