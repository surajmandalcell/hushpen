"""Loader for NVIDIA Parakeet TDT 1.1B."""

from __future__ import annotations
from .base import BaseLoader


class Parakeet11BLoader(BaseLoader):
    model_name = "nvidia/parakeet-tdt-1.1b"

    def load(self, progress_cb=None):  # pragma: no cover heavy
        if progress_cb:
            progress_cb("Importing NeMo (1.1B)...")
        from nemo.collections.asr.models import ASRModel  # type: ignore
        import torch

        model = ASRModel.from_pretrained(model_name=self.model_name).to(self.device)
        if self.device == "cuda":
            for dt in (torch.bfloat16, torch.float16):
                try:
                    model = model.to(dt)
                    break
                except Exception:
                    continue
        return model_wrapper(model), None

    def transcribe_file(self, wav_path: str):
        raise RuntimeError("Use wrapper")


def model_wrapper(model):  # pragma: no cover
    class Wrapped:
        def transcribe_file(self, wav_path: str):
            out = model.transcribe([wav_path], timestamps=True)
            segs = out[0].timestamp["segment"]
            return [
                {"start": s["start"], "end": s["end"], "text": s["segment"]}
                for s in segs
            ]

    return Wrapped()
