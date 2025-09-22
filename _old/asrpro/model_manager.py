"""Lazy single-resident model manager for Parakeet + Whisper."""

from __future__ import annotations
import gc, threading
import platform
import time
from typing import Optional
import torch
from .models import MODEL_LOADERS
from .config import config

MODEL_SPECS = {
    "parakeet-tdt-0.6b": "parakeet-0.6b",
    "parakeet-tdt-1.1b": "parakeet-1.1b",
    "whisper-medium-onnx": "whisper-medium-onnx",
}


class ModelManager:
    def __init__(self):
        self._lock = threading.RLock()
        self.current_id: Optional[str] = None
        self.current_model = None
        self.current_tokenizer = None
        self.device = self._detect_device()
        
        # Auto-unload configuration
        self.last_used_time = None
        self.auto_unload_timer = None
        self.auto_unload_minutes = config.get('models.auto_unload_after_minutes', 30)
        self.auto_unload_enabled = self.auto_unload_minutes > 0

    def _detect_device(self) -> str:
        """Enhanced device detection with MPS support for Apple Silicon"""
        
        # Check for Apple Silicon (MPS) first on macOS
        if platform.system() == 'Darwin':
            try:
                # Check if MPS is available (Apple Silicon M1/M2/M3)
                if hasattr(torch.backends, 'mps') and torch.backends.mps.is_available():
                    # Test MPS by creating a small tensor
                    test_tensor = torch.tensor([1.0], device="mps")
                    del test_tensor
                    print("[ModelManager] Using MPS (Metal Performance Shaders) for Apple Silicon")
                    return "mps"
            except Exception as e:
                print(f"[ModelManager] MPS not available: {e}")
            
            # On macOS, skip directly to CPU if MPS is not available
            # Vulkan via MoltenVK is unreliable and not worth the complexity
            print("[ModelManager] Using CPU (macOS without MPS support)")
            return "cpu"
        
        # Check CUDA for non-macOS platforms
        if torch.cuda.is_available():
            try:
                # Test CUDA by creating a small tensor
                test_tensor = torch.tensor([1.0], device="cuda")
                del test_tensor
                torch.cuda.empty_cache()
                print("[ModelManager] Using CUDA GPU")
                return "cuda"
            except Exception:
                pass

        # Check Vulkan support only on Windows/Linux
        try:
            # Check PyTorch Vulkan backend (if available)
            vulkan_backend = getattr(torch.backends, "vulkan", None)
            if (
                vulkan_backend
                and hasattr(vulkan_backend, "is_available")
                and vulkan_backend.is_available()
            ):
                print("[ModelManager] Using Vulkan GPU")
                return "vulkan"
        except Exception:
            pass

        try:
            # Check ONNX Runtime Vulkan execution provider
            import onnxruntime as ort

            available_providers = ort.get_available_providers()
            if "VulkanExecutionProvider" in available_providers:
                print("[ModelManager] Using Vulkan via ONNX Runtime")
                return "vulkan"
        except Exception:
            pass

        # Fallback to CPU
        print("[ModelManager] Using CPU (no GPU acceleration available)")
        return "cpu"

    def list_models(self):
        return [
            {"id": mid, "object": "model", "owned_by": "asrpro", "ready": True}
            for mid in MODEL_SPECS.keys()
        ]

    def load(self, model_id: str, progress_cb=None):
        with self._lock:
            if model_id == self.current_id:
                self._update_last_used()
                return self.current_model
            self.unload()

            # Get model key from specs
            model_key = MODEL_SPECS.get(model_id)
            if not model_key:
                raise ValueError(f"Unknown model: {model_id}")

            # Get loader class from consolidated models
            loader_class = MODEL_LOADERS.get(model_key)
            if not loader_class:
                raise ValueError(f"Model loader not found: {model_key}")

            loader = loader_class(device=self.device)
            if progress_cb:
                progress_cb(f"Loading {model_id} on {self.device}...")

            self.current_model, self.current_tokenizer = loader.load(
                progress_cb=progress_cb
            )
            self.current_id = model_id
            self._update_last_used()
            
            if progress_cb:
                progress_cb("Model loaded")
            
            # Start auto-unload timer if enabled
            if self.auto_unload_enabled:
                self._start_auto_unload_timer()
                print(f"[ModelManager] Auto-unload enabled: model will unload after {self.auto_unload_minutes} minutes of inactivity")
            
            return self.current_model

    def unload(self):
        with self._lock:
            # Cancel auto-unload timer
            if self.auto_unload_timer:
                self.auto_unload_timer.cancel()
                self.auto_unload_timer = None
            
            if self.current_model is not None:
                print(f"[ModelManager] Unloading model: {self.current_id}")
                try:
                    del self.current_model
                    del self.current_tokenizer
                except Exception:
                    pass
                self.current_model = self.current_tokenizer = self.current_id = None
                self.last_used_time = None
                
                # Clear GPU memory if available
                if torch.cuda.is_available():
                    torch.cuda.empty_cache()
                elif self.device == "mps":
                    # Clear MPS cache on Apple Silicon
                    try:
                        torch.mps.empty_cache()
                    except Exception:
                        pass
                
                gc.collect()
                print("[ModelManager] Model unloaded successfully")

    def transcribe(
        self, wav_path: str, model_id: Optional[str] = None, return_srt=False
    ):
        with self._lock:
            if model_id and model_id != self.current_id:
                self.load(model_id)
            if not self.current_model:
                raise RuntimeError("No model loaded")
            
            # Update last used time
            self._update_last_used()
            
            # Restart auto-unload timer
            if self.auto_unload_enabled:
                self._start_auto_unload_timer()
            
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
    
    def _update_last_used(self):
        """Update the last used timestamp."""
        self.last_used_time = time.time()
    
    def _start_auto_unload_timer(self):
        """Start or restart the auto-unload timer."""
        # Cancel existing timer if any
        if self.auto_unload_timer:
            self.auto_unload_timer.cancel()
        
        # Create new timer
        self.auto_unload_timer = threading.Timer(
            self.auto_unload_minutes * 60,  # Convert minutes to seconds
            self._auto_unload_callback
        )
        self.auto_unload_timer.daemon = True
        self.auto_unload_timer.start()
    
    def _auto_unload_callback(self):
        """Callback for auto-unload timer."""
        with self._lock:
            if self.current_model is not None:
                time_since_use = time.time() - self.last_used_time if self.last_used_time else float('inf')
                
                # Check if model has been inactive for the configured time
                if time_since_use >= (self.auto_unload_minutes * 60):
                    print(f"[ModelManager] Auto-unloading model due to {self.auto_unload_minutes} minutes of inactivity")
                    self.unload()
                else:
                    # Reschedule if model was used recently
                    remaining_time = (self.auto_unload_minutes * 60) - time_since_use
                    print(f"[ModelManager] Model was used recently, rescheduling auto-unload for {remaining_time:.0f} seconds")
                    self.auto_unload_timer = threading.Timer(remaining_time, self._auto_unload_callback)
                    self.auto_unload_timer.daemon = True
                    self.auto_unload_timer.start()
    
    def set_auto_unload_minutes(self, minutes: int):
        """Set the auto-unload timeout in minutes. 0 disables auto-unload."""
        self.auto_unload_minutes = minutes
        self.auto_unload_enabled = minutes > 0
        config.set('models.auto_unload_after_minutes', minutes)
        
        if self.auto_unload_enabled and self.current_model:
            # Restart timer with new timeout
            self._start_auto_unload_timer()
            print(f"[ModelManager] Auto-unload timeout set to {minutes} minutes")
        elif not self.auto_unload_enabled and self.auto_unload_timer:
            # Disable auto-unload
            self.auto_unload_timer.cancel()
            self.auto_unload_timer = None
            print("[ModelManager] Auto-unload disabled")


__all__ = ["ModelManager", "MODEL_SPECS"]
