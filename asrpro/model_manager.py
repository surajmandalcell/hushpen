"""Lazy single-resident model manager for Parakeet + Whisper."""

from __future__ import annotations
import gc, importlib, threading
from typing import Optional
import torch

MODEL_SPECS = {
    "parakeet-tdt-0.6b": "asrpro.models.parakeet_06b:Parakeet06BLoader",
    "parakeet-tdt-1.1b": "asrpro.models.parakeet_11b:Parakeet11BLoader",
    "whisper-medium-onnx": "asrpro.models.whisper_medium_onnx:WhisperMediumOnnxLoader",
}


class ModelManager:
    def __init__(self):
        self._lock = threading.RLock()
        self.current_id: Optional[str] = None
        self.current_model = None
        self.current_tokenizer = None
        self.device = self._detect_device()

    def _detect_device(self) -> str:
        """Enhanced device detection with better Vulkan support"""
        # First check CUDA
        if torch.cuda.is_available():
            try:
                # Test CUDA by creating a small tensor
                test_tensor = torch.tensor([1.0], device="cuda")
                del test_tensor
                torch.cuda.empty_cache()
                return "cuda"
            except Exception:
                pass

        # Check Vulkan support
        try:
            # Check PyTorch Vulkan backend (if available)
            vulkan_backend = getattr(torch.backends, "vulkan", None)
            if (
                vulkan_backend
                and hasattr(vulkan_backend, "is_available")
                and vulkan_backend.is_available()
            ):
                return "vulkan"
        except Exception:
            pass

        try:
            # Check ONNX Runtime Vulkan execution provider
            import onnxruntime as ort

            available_providers = ort.get_available_providers()
            if "VulkanExecutionProvider" in available_providers:
                return "vulkan"
        except Exception:
            pass

        # Fallback to CPU
        return "cpu"

    def list_models(self):
        return [
            {"id": mid, "object": "model", "owned_by": "asrpro", "ready": True}
            for mid in MODEL_SPECS.keys()
        ]

    def load(self, model_id: str, progress_cb=None):
        with self._lock:
            if model_id == self.current_id:
                return self.current_model
            self.unload()
            spec = MODEL_SPECS.get(model_id)
            if not spec:
                raise ValueError(f"Unknown model: {model_id}")
            module_name, class_name = spec.split(":")
            mod = importlib.import_module(module_name)
            cls = getattr(mod, class_name)
            loader = cls(device=self.device)
            if progress_cb:
                progress_cb(f"Loading {model_id} on {self.device}...")
            self.current_model, self.current_tokenizer = loader.load(
                progress_cb=progress_cb
            )
            self.current_id = model_id
            if progress_cb:
                progress_cb("Model loaded")
            return self.current_model

    def unload(self):
        with self._lock:
            if self.current_model is not None:
                try:
                    del self.current_model
                    del self.current_tokenizer
                except Exception:
                    pass
                self.current_model = self.current_tokenizer = self.current_id = None
                if torch.cuda.is_available():
                    torch.cuda.empty_cache()
                gc.collect()

    def transcribe(
        self, wav_path: str, model_id: Optional[str] = None, return_srt=False
    ):
        with self._lock:
            if model_id and model_id != self.current_id:
                self.load(model_id)
            if not self.current_model:
                raise RuntimeError("No model loaded")
            result = self.current_model.transcribe_file(wav_path)
            if return_srt:
                return self._to_srt(result)
            return result

    @staticmethod
    def _to_srt(result):
        lines = []
        for idx, seg in enumerate(result, start=1):

            def fmt(t):
                h = int(t // 3600)
                m = int((t % 3600) // 60)
                s = int(t % 60)
                ms = int((t - int(t)) * 1000)
                return f"{h:02d}:{m:02d}:{s:02d},{ms:03d}"

            lines.append(
                f"{idx}\n{fmt(seg['start'])} --> {fmt(seg['end'])}\n{seg['text']}\n"
            )
        return "\n".join(lines)


__all__ = ["ModelManager", "MODEL_SPECS"]
