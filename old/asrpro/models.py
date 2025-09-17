"""All model loaders consolidated into a single file."""

from __future__ import annotations
from typing import Tuple, Any, Optional, List, Dict


class BaseLoader:
    """Base model loader interface."""

    model_name: str = ""

    def __init__(self, device: str = "cpu"):
        self.device = device

    def load(self, progress_cb=None) -> Tuple[Any, Optional[Any]]:
        raise NotImplementedError

    def transcribe_file(self, wav_path: str) -> List[Dict]:
        raise NotImplementedError


class Parakeet06BLoader(BaseLoader):
    """Loader for NVIDIA Parakeet TDT 0.6B."""

    model_name = "nvidia/parakeet-tdt-0.6b-v2"

    def load(self, progress_cb=None):  # pragma: no cover heavy
        if progress_cb:
            progress_cb("Importing NeMo (0.6B)...")
        from nemo.collections.asr.models import ASRModel  # type: ignore
        import torch

        model = ASRModel.from_pretrained(model_name=self.model_name).to(self.device)
        
        # Optimize for different devices
        if self.device == "cuda":  # attempt mixed precision for CUDA
            for dt in (torch.bfloat16, torch.float16):
                try:
                    model = model.to(dt)
                    break
                except Exception:
                    continue
        elif self.device == "mps":  # Apple Silicon optimization
            # MPS works best with float32 currently
            model = model.to(torch.float32)
            if progress_cb:
                progress_cb("Using Metal Performance Shaders (Apple Silicon)")
                
        return self._wrap_parakeet(model), None

    def _wrap_parakeet(self, model):  # pragma: no cover
        class Wrapped:
            def transcribe_file(self, wav_path: str):
                out = model.transcribe([wav_path], timestamps=True)
                segs = out[0].timestamp["segment"]
                return [
                    {"start": s["start"], "end": s["end"], "text": s["segment"]}
                    for s in segs
                ]

        return Wrapped()

    def transcribe_file(self, wav_path: str):
        raise RuntimeError("Use wrapper")


class Parakeet11BLoader(BaseLoader):
    """Loader for NVIDIA Parakeet TDT 1.1B."""

    model_name = "nvidia/parakeet-tdt-1.1b"

    def load(self, progress_cb=None):  # pragma: no cover heavy
        if progress_cb:
            progress_cb("Importing NeMo (1.1B)...")
        from nemo.collections.asr.models import ASRModel  # type: ignore
        import torch

        model = ASRModel.from_pretrained(model_name=self.model_name).to(self.device)
        
        # Optimize for different devices
        if self.device == "cuda":  # attempt mixed precision for CUDA
            for dt in (torch.bfloat16, torch.float16):
                try:
                    model = model.to(dt)
                    break
                except Exception:
                    continue
        elif self.device == "mps":  # Apple Silicon optimization
            # MPS works best with float32 currently
            model = model.to(torch.float32)
            if progress_cb:
                progress_cb("Using Metal Performance Shaders (Apple Silicon)")
                
        return self._wrap_parakeet(model), None

    def _wrap_parakeet(self, model):  # pragma: no cover
        class Wrapped:
            def transcribe_file(self, wav_path: str):
                out = model.transcribe([wav_path], timestamps=True)
                segs = out[0].timestamp["segment"]
                return [
                    {"start": s["start"], "end": s["end"], "text": s["segment"]}
                    for s in segs
                ]

        return Wrapped()

    def transcribe_file(self, wav_path: str):
        raise RuntimeError("Use wrapper")


class WhisperMediumOnnxLoader(BaseLoader):
    """Loader for Whisper Medium ONNX via faster-whisper."""

    model_name = "medium"

    def load(self, progress_cb=None):  # pragma: no cover heavy
        if progress_cb:
            progress_cb("Importing faster_whisper (Whisper Medium)...")
        from faster_whisper import WhisperModel  # type: ignore

        # Enhanced device and compute type selection
        if self.device == "cuda":
            compute_type = "float16"
            device = "cuda"
        elif self.device == "mps":
            # MPS support via CoreML in faster-whisper is experimental
            # Fall back to CPU with optimized settings
            compute_type = "int8"
            device = "cpu"
            if progress_cb:
                progress_cb("Note: Whisper using CPU (MPS support pending in faster-whisper)")
        elif self.device == "vulkan":
            # For Vulkan, we need to use CPU device but with optimized settings
            compute_type = "int8"
            device = "cpu"
            if progress_cb:
                progress_cb("Note: Using CPU backend with Vulkan optimizations...")
        else:
            compute_type = "int8"
            device = "cpu"

        # Try to create model with enhanced error handling
        try:
            model = WhisperModel(
                self.model_name,
                device=device,
                compute_type=compute_type,
                download_root=None,  # Use default cache
                local_files_only=False,
            )
            if progress_cb:
                progress_cb(
                    f"Model loaded successfully on {device} with {compute_type}"
                )
        except Exception as e:
            if progress_cb:
                progress_cb(f"Fallback: Loading with CPU int8 due to: {e}")
            # Fallback to basic CPU configuration
            model = WhisperModel(
                self.model_name,
                device="cpu",
                compute_type="int8",
            )

        class Wrapped:
            def transcribe_file(self, wav_path: str):
                segments, _ = model.transcribe(
                    wav_path,
                    vad_filter=True,
                    beam_size=1,  # Faster inference
                    best_of=1,  # Faster inference
                )
                return [
                    {"start": seg.start, "end": seg.end, "text": seg.text.strip()}
                    for seg in segments
                ]

        return Wrapped(), None

    def transcribe_file(self, wav_path: str):
        raise RuntimeError("Use wrapper")


# Model registry for easy access
MODEL_LOADERS = {
    "parakeet-0.6b": Parakeet06BLoader,
    "parakeet-1.1b": Parakeet11BLoader,
    "whisper-medium-onnx": WhisperMediumOnnxLoader,
}
