"""
Concrete model loaders.
"""

import logging
import os
from pathlib import Path

from utils.audio_converter import convert_to_wav
from .base import ONNXBaseLoader
from .base import BaseLoader


logger = logging.getLogger(__name__)


class ConfigDrivenLoader(ONNXBaseLoader):
    def _get_model_name(self):
        # Prefer a list of candidates; fall back to single string if provided
        candidates = self.config.get("candidates")
        if candidates:
            return candidates
        # Back-compat: allow a single model name under 'model_name'
        model_name = self.config.get("model_name")
        return model_name or self.model_id


class NemoParakeetLoader(BaseLoader):
    """NeMo-backed loader for NVIDIA Parakeet-TDT-0.6B-v3."""

    def __init__(self, model_id: str, config):
        super().__init__(model_id, config)
        self.current_backend = None

    def _configure_cache_environment(self):
        cache_dir = Path(self.config.get("cache_dir") or Path.cwd() / "data" / "models")
        os.environ["HF_HOME"] = str(cache_dir / "huggingface")
        os.environ["HUGGINGFACE_HUB_CACHE"] = str(cache_dir / "huggingface" / "hub")
        os.environ["NEMO_HOME"] = str(cache_dir / "nemo")
        os.environ["TORCH_HOME"] = str(cache_dir / "torch")
        os.environ["XDG_CACHE_HOME"] = str(cache_dir.parent / "cache")

        for key in ["HF_HOME", "HUGGINGFACE_HUB_CACHE", "NEMO_HOME", "TORCH_HOME", "XDG_CACHE_HOME"]:
            Path(os.environ[key]).mkdir(parents=True, exist_ok=True)

    async def load(self) -> bool:
        try:
            self._configure_cache_environment()

            import nemo.collections.asr as nemo_asr

            model_name = self.config.get("repo") or self.config.get("candidates", [self.model_id])[0]
            logger.info(f"Loading NeMo model {model_name}")
            self.model = nemo_asr.models.ASRModel.from_pretrained(model_name=model_name)

            try:
                import torch

                if torch.cuda.is_available():
                    self.model = self.model.to("cuda")
                    self.current_backend = "cuda"
                else:
                    self.model = self.model.to("cpu")
                    self.current_backend = "cpu"
            except Exception:
                self.current_backend = "cpu"

            self.is_loaded = True
            return True
        except ImportError as e:
            logger.error(f"nemo-toolkit is not installed for {self.model_id}: {e}")
            return False
        except Exception as e:
            logger.error(f"Failed to load NeMo model {self.model_id}: {e}")
            return False

    async def unload(self) -> bool:
        self.model = None
        self.is_loaded = False
        self.current_backend = None
        return True

    async def transcribe(self, audio_file):
        if not self.is_ready():
            raise Exception("Model not loaded")

        wav_path = convert_to_wav(audio_file, target_sample_rate=16000)
        try:
            result = self.model.transcribe([wav_path], timestamps=True)
            first = result[0] if isinstance(result, list) and result else result
            text = self._extract_text(first)
            return {
                "text": text,
                "segments": self._extract_segments(first),
                "language": "auto",
                "language_probability": 1.0,
                "duration": 0.0,
                "backend": self.current_backend or "cpu",
                "model": self.model_id,
            }
        finally:
            if os.path.exists(wav_path):
                os.unlink(wav_path)

    def _extract_text(self, result) -> str:
        if hasattr(result, "text"):
            return str(result.text)
        if isinstance(result, dict) and "text" in result:
            return str(result["text"])
        return str(result)

    def _extract_segments(self, result):
        timestamp = getattr(result, "timestamp", None)
        if timestamp is None and isinstance(result, dict):
            timestamp = result.get("timestamp")

        segments = []
        if isinstance(timestamp, dict):
            for item in timestamp.get("segment", []) or []:
                segments.append({
                    "start": float(item.get("start", 0.0)),
                    "end": float(item.get("end", 0.0)),
                    "text": str(item.get("segment", item.get("text", ""))),
                })

        if not segments:
            segments.append({"start": 0.0, "end": 0.0, "text": self._extract_text(result)})
        return segments
