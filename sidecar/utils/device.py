"""
Device detection utilities for ASR Pro Python Sidecar
"""

import platform
import logging
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)


class DeviceDetector:
    """Detects and manages device capabilities."""

    def __init__(self):
        self.system = platform.system()
        self.device_info = {
            "system": self.system,
            "device": "cpu",
            "compute_type": "float32",
            "cuda_available": False,
            "mps_available": False,
            "directml_available": False,
            "vulkan_available": False,
            "device_name": "CPU",
        }

    async def detect_capabilities(self):
        """Detect device capabilities."""
        logger.info("Detecting device capabilities")

        # Check for CUDA (highest priority)
        self._check_cuda()

        # Check for MPS (Apple Silicon)
        self._check_mps()

        # Check for DirectML (Windows)
        self._check_directml()

        # Check for Vulkan (cross-platform GPU)
        self._check_vulkan()

        # Determine best device
        self._determine_best_device()

        logger.info(f"Device capabilities: {self.device_info}")

    def _check_cuda(self):
        """Check for CUDA availability."""
        try:
            import torch

            if torch.cuda.is_available():
                self.device_info["cuda_available"] = True
                self.device_info["cuda_device_count"] = torch.cuda.device_count()
                self.device_info["cuda_device_name"] = torch.cuda.get_device_name(0)
                logger.info(
                    f"CUDA is available: {self.device_info['cuda_device_name']}"
                )
        except ImportError:
            logger.warning("PyTorch not available, cannot check CUDA")
        except Exception as e:
            logger.warning(f"Failed to check CUDA: {e}")

        # Fallback: detect CUDA via ONNX Runtime providers even if PyTorch is CPU-only
        if not self.device_info["cuda_available"]:
            try:
                import onnxruntime as ort

                providers = ort.get_available_providers()
                if "CUDAExecutionProvider" in providers:
                    self.device_info["cuda_available"] = True
                    # Keep any existing name from PyTorch if set; otherwise set a generic one
                    self.device_info.setdefault(
                        "cuda_device_name", "CUDA (ONNX Runtime)"
                    )
                    logger.info("CUDA is available via ONNX Runtime")
            except ImportError:
                logger.debug("onnxruntime not installed; skipping ORT CUDA check")
            except Exception as e:
                logger.debug(f"Failed to check CUDA via ORT: {e}")

    def _check_mps(self):
        """Check for MPS availability (Apple Silicon)."""
        if self.system == "Darwin":
            try:
                import torch

                if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
                    self.device_info["mps_available"] = True
                    logger.info("MPS is available (Apple Silicon)")
            except ImportError:
                logger.warning("PyTorch not available, cannot check MPS")
            except Exception as e:
                logger.warning(f"Failed to check MPS: {e}")

    def _check_directml(self):
        """Check for DirectML availability (Windows via ONNX Runtime)."""
        if self.system == "Windows":
            try:
                import onnxruntime as ort

                available = ort.get_available_providers()
                if (
                    "DmlExecutionProvider" in available
                    or "DirectMLExecutionProvider" in available
                ):
                    self.device_info["directml_available"] = True
                    logger.info("DirectML is available (Windows)")
            except ImportError:
                logger.debug("onnxruntime not installed; skipping DirectML check")
            except Exception as e:
                logger.debug(f"Failed to check DirectML: {e}")

    def _check_vulkan(self):
        """Check for Vulkan availability."""
        import subprocess
        import os
        
        # Try to detect Vulkan via vulkaninfo command
        try:
            # Check for vulkaninfo command
            result = subprocess.run(
                ["vulkaninfo"], 
                capture_output=True, 
                text=True, 
                timeout=10
            )
            if result.returncode == 0 and "Vulkan Instance" in result.stdout:
                self.device_info["vulkan_available"] = True
                logger.info("Vulkan is available (via vulkaninfo)")
                return
        except (FileNotFoundError, subprocess.TimeoutExpired):
            logger.debug("vulkaninfo command not found or timed out")
        except Exception as e:
            logger.debug(f"Failed to run vulkaninfo: {e}")
        
        # Try alternative detection via VK_LAYER_PATH or vulkan libraries
        try:
            # Check for common Vulkan environment variables
            vk_layer_path = os.environ.get("VK_LAYER_PATH")
            vulkan_sdk = os.environ.get("VULKAN_SDK")
            
            if vk_layer_path or vulkan_sdk:
                self.device_info["vulkan_available"] = True
                logger.info("Vulkan is available (via environment variables)")
                return
                
            # Try to find Vulkan loader library
            import ctypes
            import ctypes.util
            
            vulkan_lib = None
            for lib_name in ["vulkan-1", "libvulkan.so.1", "libvulkan.dylib"]:
                try:
                    vulkan_lib = ctypes.util.find_library(lib_name)
                    if vulkan_lib:
                        break
                except:
                    continue
            
            if vulkan_lib:
                self.device_info["vulkan_available"] = True
                logger.info("Vulkan is available (via library detection)")
            else:
                logger.debug("Vulkan not available")
                
        except Exception as e:
            logger.debug(f"Failed to check Vulkan: {e}")

    def _determine_best_device(self):
        """Determine the best available device."""
        # Priority per OS:
        # macOS: MPS/CoreML
        # Windows: CUDA > DirectML > Vulkan > CPU
        # Linux: CUDA > Vulkan > CPU
        if self.system == "Darwin" and self.device_info["mps_available"]:
            self.device_info["device"] = "mps"
            self.device_info["device_name"] = "Apple Silicon (MPS)"
            self.device_info["compute_type"] = "float16"  # Use float16 for MPS
        elif self.device_info["cuda_available"]:
            self.device_info["device"] = "cuda"
            self.device_info["device_name"] = self.device_info.get(
                "cuda_device_name", "CUDA"
            )
            self.device_info["compute_type"] = "float16"  # Use float16 for CUDA
        elif self.system == "Windows" and self.device_info.get("directml_available"):
            self.device_info["device"] = "directml"
            self.device_info["device_name"] = "DirectML"
            self.device_info["compute_type"] = "float16"
        elif self.device_info["vulkan_available"]:
            self.device_info["device"] = "vulkan"
            self.device_info["device_name"] = "Vulkan GPU"
            self.device_info["compute_type"] = "float16"  # Use float16 for Vulkan
        else:
            self.device_info["device"] = "cpu"
            self.device_info["device_name"] = "CPU"
            self.device_info["compute_type"] = "float32"  # Use float32 for CPU

        logger.info(
            f"Selected device: {self.device_info['device_name']} ({self.device_info['device']})"
        )

    def get_device_config(self) -> Dict[str, Any]:
        """Get device configuration."""
        return {
            "device": self.device_info["device"],
            "compute_type": self.device_info["compute_type"],
        }

    def get_current_device(self) -> str:
        """Get the current device."""
        return self.device_info["device"]

    def get_device_info(self) -> Dict[str, Any]:
        """Get full device information."""
        return self.device_info.copy()

    def is_cuda_available(self) -> bool:
        """Check if CUDA is available."""
        return self.device_info["cuda_available"]

    def is_mps_available(self) -> bool:
        """Check if MPS is available."""
        return self.device_info["mps_available"]

    def is_vulkan_available(self) -> bool:
        """Check if Vulkan is available."""
        return self.device_info["vulkan_available"]

    def is_directml_available(self) -> bool:
        """Check if DirectML is available (Windows)."""
        return self.device_info["directml_available"]

    def get_device_name(self) -> str:
        """Get the device name."""
        return self.device_info["device_name"]
