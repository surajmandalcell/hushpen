"""Global toggle hotkey using pynput."""

from __future__ import annotations
import json, threading
from pathlib import Path
from typing import Callable, Optional

try:
    from pynput import keyboard  # type: ignore
except Exception:  # pragma: no cover
    keyboard = None  # type: ignore
CONFIG_PATH = Path.home() / ".asrpro_hotkey.json"
DEFAULT_HOTKEY = "<ctrl>+<alt>+t"


def load_hotkey():
    if CONFIG_PATH.exists():
        try:
            return json.loads(CONFIG_PATH.read_text()).get("hotkey", DEFAULT_HOTKEY)
        except Exception:
            return DEFAULT_HOTKEY
    return DEFAULT_HOTKEY


def save_hotkey(hk: str):
    CONFIG_PATH.write_text(json.dumps({"hotkey": hk}, indent=2))


class ToggleHotkey:
    def __init__(self, on_toggle: Callable[[bool], None]):
        self.on_toggle = on_toggle
        self.is_active = False
        self.listener: Optional[keyboard.GlobalHotKeys] = None  # type: ignore
        self.hotkey = load_hotkey()

    def _build_listener(self):  # pragma: no cover
        if keyboard is None:
            return
        self.listener = keyboard.GlobalHotKeys({self.hotkey: self._toggle})

    def start(self):  # pragma: no cover
        if keyboard is None:
            return
        self._build_listener()
        threading.Thread(target=self.listener.run, daemon=True).start()

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
        self.is_active = not self.is_active
        self.on_toggle(self.is_active)
