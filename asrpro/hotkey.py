"""Global toggle hotkey using pynput."""

from __future__ import annotations
import threading
from typing import Callable, Optional

try:
    from pynput import keyboard  # type: ignore
except Exception:  # pragma: no cover
    keyboard = None  # type: ignore

from .config import config

DEFAULT_HOTKEY = "<ctrl>+<alt>+t"


def load_hotkey():
    return config.get_hotkey()


def save_hotkey(hk: str):
    config.set_hotkey(hk)


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
        if self.listener:
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
