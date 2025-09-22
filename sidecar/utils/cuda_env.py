"""
Windows CUDA DLL environment fix-ups for ONNX Runtime CUDA EP.

Attempts to add nvidia pip package DLL paths (cuDNN, cuBLAS) to PATH so that
onnxruntime_providers_cuda.dll can resolve dependencies like cublasLt64_12.dll.
"""

import os
import sys
from pathlib import Path
from typing import List


def _maybe_add_dir_to_dll_search(path: Path) -> None:
    if not path or not path.exists():
        return
    try:
        # Prefer explicit DLL directory on Windows 3.8+
        if hasattr(os, "add_dll_directory") and sys.platform.startswith("win"):
            os.add_dll_directory(str(path))
    except Exception:
        # Fallback to PATH extension
        os.environ["PATH"] = f"{str(path)};{os.environ.get('PATH','')}"


def _collect_candidate_dirs() -> List[Path]:
    candidates: List[Path] = []
    try:
        import importlib.util

        # Common layout for nvidia pip wheels:
        # site-packages/nvidia/cublas/<ver>/bin
        # site-packages/nvidia/cudnn/<ver>/bin
        spec = importlib.util.find_spec("nvidia")
        if spec and spec.submodule_search_locations:
            base = Path(list(spec.submodule_search_locations)[0])
            for sub in ("cublas", "cudnn"):
                root = base / sub
                if root.exists():
                    # Add immediate bin dirs
                    for ver_dir in root.iterdir():
                        bin_dir = ver_dir / "bin"
                        if bin_dir.exists():
                            candidates.append(bin_dir)
    except Exception:
        pass

    # Also respect CUDA_PATH env if present (from full CUDA Toolkit install)
    cuda_path = os.environ.get("CUDA_PATH") or os.environ.get("CUDA_PATH_V12_1")
    if cuda_path:
        cuda_bin = Path(cuda_path) / "bin"
        candidates.append(cuda_bin)

    return candidates


def ensure_cuda_dlls_on_path() -> None:
    """Ensure CUDA-dependent DLLs are discoverable for ORT CUDA EP on Windows."""
    if not sys.platform.startswith("win"):
        return
    for d in _collect_candidate_dirs():
        _maybe_add_dir_to_dll_search(d)
