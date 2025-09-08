"""Base model loader interface."""

from __future__ import annotations
from typing import Tuple, Any, Optional, List, Dict


class BaseLoader:
    model_name: str = ""

    def __init__(self, device: str = "cpu"):
        self.device = device

    def load(self, progress_cb=None) -> Tuple[Any, Optional[Any]]:
        raise NotImplementedError

    def transcribe_file(self, wav_path: str) -> List[Dict]:
        raise NotImplementedError
