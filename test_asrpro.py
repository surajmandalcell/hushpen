#!/usr/bin/env python3
"""Test script to run asrpro application."""

import sys
import os
from pathlib import Path

# Add the project root to the Python path
project_root = Path(__file__).parent
sys.path.insert(0, str(project_root))

# Set environment variables for better UI experience
os.environ["QT_AUTO_SCREEN_SCALE_FACTOR"] = "1"
os.environ["QT_ENABLE_HIGHDPI_SCALING"] = "1"

if __name__ == "__main__":
    from asrpro.main import launch

    print("Starting asrpro...")
    print("Features:")
    print("  ✓ Modern linear UI design")
    print("  ✓ Global hotkey transcription")
    print("  ✓ Drag & drop SRT generation")
    print("  ✓ OpenAI-compatible API server on port 7341")
    print("  ✓ CUDA/Vulkan/CPU device fallback")
    print("  ✓ System tray integration")
    print("\nPress Ctrl+C to exit.")
    launch()
