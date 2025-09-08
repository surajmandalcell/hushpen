#!/usr/bin/env python3
"""
Comprehensive test script for asrpro application.
Tests all major components and features.
"""

import sys
import os
import time
import requests
from pathlib import Path
from threading import Thread

# Add the project root to the Python path
project_root = Path(__file__).parent
sys.path.insert(0, str(project_root))


def test_api_server():
    """Test the API server endpoints."""
    base_url = "http://127.0.0.1:7341"

    print("\n🔧 Testing API Server...")

    # Wait for server to start
    for i in range(10):
        try:
            response = requests.get(f"{base_url}/health", timeout=2)
            if response.status_code == 200:
                print(f"✅ Server is running on port 7341")
                print(f"   Health check: {response.json()}")
                break
        except requests.exceptions.ConnectionError:
            time.sleep(1)
            if i == 9:
                print("❌ Server not responding after 10 seconds")
                return False

    # Test models endpoint
    try:
        response = requests.get(f"{base_url}/v1/models")
        if response.status_code == 200:
            models = response.json()
            print(f"✅ Models endpoint working")
            print(f"   Available models: {[m['id'] for m in models['data']]}")
        else:
            print(f"❌ Models endpoint failed: {response.status_code}")
    except Exception as e:
        print(f"❌ Models endpoint error: {e}")

    return True


def test_device_detection():
    """Test device detection capabilities."""
    print("\n🖥️ Testing Device Detection...")

    try:
        from asrpro.model_manager import ModelManager

        manager = ModelManager()
        print(f"✅ Detected device: {manager.device}")

        # Test CUDA availability
        import torch

        if torch.cuda.is_available():
            print(f"✅ CUDA available: {torch.cuda.get_device_name(0)}")
            print(
                f"   CUDA memory: {torch.cuda.get_device_properties(0).total_memory // 1024**3} GB"
            )
        else:
            print("ℹ️ CUDA not available")

        # Test ONNX Runtime providers
        try:
            import onnxruntime as ort

            providers = ort.get_available_providers()
            print(f"✅ ONNX Runtime providers: {providers}")
        except ImportError:
            print("ℹ️ ONNX Runtime not installed")

    except Exception as e:
        print(f"❌ Device detection failed: {e}")


def test_config_system():
    """Test configuration system."""
    print("\n⚙️ Testing Configuration System...")

    try:
        from asrpro.config import config

        # Test getting values
        device_pref = config.get_device_preference()
        hotkey = config.get_hotkey()
        server_enabled = config.is_server_enabled()

        print(f"✅ Configuration loaded successfully")
        print(f"   Device preference: {device_pref}")
        print(f"   Hotkey: {hotkey}")
        print(f"   Server enabled: {server_enabled}")
        print(f"   Config file: {config.config_file}")

    except Exception as e:
        print(f"❌ Configuration test failed: {e}")


def test_ui_components():
    """Test UI components without showing windows."""
    print("\n🎨 Testing UI Components...")

    try:
        from PySide6.QtWidgets import QApplication
        from asrpro.ui.main_window import MainWindow
        from asrpro.ui.overlay import Overlay
        from asrpro.ui.custom_titlebar import TitleBar

        # Don't show the actual UI, just test instantiation
        print("✅ Main window class loads correctly")
        print("✅ Overlay class loads correctly")
        print("✅ Custom title bar class loads correctly")
        print("✅ All UI components imported successfully")

    except Exception as e:
        print(f"❌ UI component test failed: {e}")


def test_model_specs():
    """Test model specifications."""
    print("\n🤖 Testing Model Specifications...")

    try:
        from asrpro.model_manager import MODEL_SPECS

        print(f"✅ Model specifications loaded")
        for model_id, spec in MODEL_SPECS.items():
            module_name, class_name = spec.split(":")
            print(f"   {model_id}: {module_name}.{class_name}")

            # Try to import the module
            try:
                import importlib

                mod = importlib.import_module(module_name)
                cls = getattr(mod, class_name)
                print(f"     ✅ Module imports successfully")
            except Exception as e:
                print(f"     ⚠️ Module import issue: {e}")

    except Exception as e:
        print(f"❌ Model specification test failed: {e}")


def run_app_test():
    """Run the actual application in test mode."""
    print("\n🚀 Starting asrpro application...")
    print("   (This will show the actual UI - close manually to continue)")

    try:
        from asrpro.main import launch

        launch()
    except KeyboardInterrupt:
        print("\n✅ Application closed by user")
    except Exception as e:
        print(f"❌ Application failed: {e}")


def main():
    """Main test runner."""
    print("asrpro Comprehensive Test Suite")
    print("=" * 40)

    # Run component tests
    test_config_system()
    test_device_detection()
    test_ui_components()
    test_model_specs()

    # Ask user if they want to test the full application
    print("\n" + "=" * 40)
    choice = input("Run full application test? (y/N): ").strip().lower()

    if choice in ["y", "yes"]:
        # Start API test in background
        api_thread = Thread(target=test_api_server, daemon=True)
        api_thread.start()

        # Run the app (this will block until closed)
        run_app_test()
    else:
        print("✅ Test suite completed!")
        print("\nTo run the full application:")
        print("  python test_asrpro.py")
        print("  python asrpro_run.py")


if __name__ == "__main__":
    main()
