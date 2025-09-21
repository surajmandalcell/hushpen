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
        else:
            # For testing purposes, check if we're in a test environment
            try:
                import torch

                if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
                    self.device_info["mps_available"] = True
                    logger.info("MPS is available (Apple Silicon)")
            except ImportError:
                pass
            except Exception as e:
                pass

    def _check_vulkan(self):
        """Check for Vulkan availability."""
        try:
            import vulkan

            self.device_info["vulkan_available"] = True
            logger.info("Vulkan is available")
        except ImportError:
            # Try alternative Vulkan detection
            try:
                import subprocess

                result = subprocess.run(["vulkaninfo"], capture_output=True, text=True)
                if result.returncode == 0:
                    self.device_info["vulkan_available"] = True
                    logger.info("Vulkan is available (via vulkaninfo)")
            except (ImportError, FileNotFoundError):
                logger.debug("Vulkan not available")
        except Exception as e:
            logger.debug(f"Failed to check Vulkan: {e}")

    def _determine_best_device(self):
        """Determine the best available device."""
        # Priority: CUDA > MPS > Vulkan > CPU
        if self.device_info["cuda_available"]:
            self.device_info["device"] = "cuda"
            self.device_info["device_name"] = self.device_info.get(
                "cuda_device_name", "CUDA"
            )
            self.device_info["compute_type"] = "float16"  # Use float16 for CUDA
        elif self.device_info["mps_available"]:
            self.device_info["device"] = "mps"
            self.device_info["device_name"] = "Apple Silicon (MPS)"
            self.device_info["compute_type"] = "float16"  # Use float16 for MPS
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

    def get_device_name(self) -> str:
        """Get the device name."""
        return self.device_info["device_name"]
