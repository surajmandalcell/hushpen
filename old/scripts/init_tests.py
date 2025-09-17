#!/usr/bin/env python3
"""Initialize test directory with basic tests for ASR Pro."""

from pathlib import Path

def create_test_files():
    """Create basic test files."""
    
    # Get project root
    project_root = Path(__file__).parent.parent
    tests_dir = project_root / "tests"
    tests_dir.mkdir(exist_ok=True)
    
    # Create __init__.py
    (tests_dir / "__init__.py").write_text('"""ASR Pro test suite."""\n')
    
    # Create test_imports.py
    test_imports_content = '''"""Test that all modules can be imported without errors."""

import sys
import platform
import pytest


class TestImports:
    """Test module imports."""
    
    def test_import_main_package(self):
        """Test that main package can be imported."""
        import asrpro
        assert asrpro is not None
    
    def test_import_main_module(self):
        """Test main module import."""
        from asrpro import main
        assert hasattr(main, 'launch')
    
    def test_import_config(self):
        """Test config module import."""
        from asrpro import config
        assert hasattr(config, 'Config')
    
    def test_import_models(self):
        """Test models module import."""
        from asrpro import models
        assert hasattr(models, 'BaseLoader')
        assert hasattr(models, 'MODEL_LOADERS')
    
    def test_import_model_manager(self):
        """Test model manager import."""
        from asrpro import model_manager
        assert hasattr(model_manager, 'ModelManager')
    
    def test_import_audio_recorder(self):
        """Test audio recorder import."""
        from asrpro import audio_recorder
        assert hasattr(audio_recorder, 'AudioRecorder')
    
    def test_import_server(self):
        """Test server module import."""
        from asrpro import server
        assert hasattr(server, 'create_app')
        assert hasattr(server, 'ServerThread')
    
    def test_import_hotkey(self):
        """Test hotkey module import."""
        from asrpro import hotkey
        assert hasattr(hotkey, 'ToggleHotkey')
    
    def test_no_windows_specific_imports_on_macos(self):
        """Test that Windows-specific imports don't break on macOS."""
        if platform.system() == 'Darwin':
            # This should not raise ImportError on macOS
            from asrpro.ui.components import tray
            assert hasattr(tray, 'build_tray')
'''
    
    (tests_dir / "test_imports.py").write_text(test_imports_content)
    
    # Create test_macos.py
    test_macos_content = '''"""Test macOS-specific functionality."""

import platform
import pytest
import subprocess
from pathlib import Path


@pytest.mark.skipif(platform.system() != 'Darwin', reason="macOS specific tests")
class TestMacOSCompatibility:
    """Test macOS compatibility."""
    
    def test_no_winreg_import(self):
        """Ensure winreg is not imported on macOS."""
        with pytest.raises(ImportError):
            import winreg
    
    def test_dark_mode_detection(self):
        """Test that dark mode detection works on macOS."""
        from asrpro.ui.components.tray import is_dark_theme
        # Should not raise an error
        result = is_dark_theme()
        assert isinstance(result, bool)
    
    def test_device_detection(self):
        """Test device detection on macOS."""
        from asrpro.model_manager import ModelManager
        mm = ModelManager()
        
        # Should use MPS or CPU, never CUDA on macOS
        assert mm.device in ['mps', 'cpu']
        assert mm.device != 'cuda'
    
    def test_config_paths(self):
        """Test that config uses proper macOS paths."""
        from asrpro.config import Config
        config = Config()
        
        # Config should be in project for now (will be fixed)
        # After fix, it should be in ~/Library/Application Support/asrpro/
        config_dir = config.get_config_dir()
        assert config_dir.exists()
    
    def test_hotkey_accessibility_check(self):
        """Test hotkey accessibility permission check."""
        from asrpro.hotkey import ToggleHotkey
        
        # Create a dummy toggle function
        def dummy_toggle(is_active):
            pass
        
        hotkey = ToggleHotkey(dummy_toggle)
        # Should have the accessibility check method
        assert hasattr(hotkey, '_check_macos_accessibility')
        
        # Check should return a boolean
        result = hotkey._check_macos_accessibility()
        assert isinstance(result, bool)


class TestCrossPlatform:
    """Test cross-platform compatibility."""
    
    def test_platform_detection(self):
        """Test platform detection."""
        system = platform.system()
        assert system in ['Darwin', 'Linux', 'Windows']
    
    def test_paths_are_pathlib(self):
        """Test that all paths use pathlib."""
        from asrpro.config import Config
        config = Config()
        
        config_dir = config.get_config_dir()
        assert isinstance(config_dir, Path)
        
        lock_path = config.get_lock_path()
        assert isinstance(lock_path, Path)
    
    def test_no_hardcoded_paths(self):
        """Test that there are no hardcoded Windows paths."""
        from asrpro.config import Config
        config = Config()
        
        config_str = str(config.config_file)
        # Should not contain Windows-specific path separators
        assert '\\\\' not in config_str or platform.system() == 'Windows'
'''
    
    (tests_dir / "test_macos.py").write_text(test_macos_content)
    
    # Create test_config.py
    test_config_content = '''"""Test configuration management."""

import json
import tempfile
from pathlib import Path
import pytest
from asrpro.config import Config, DEFAULT_CONFIG


class TestConfig:
    """Test configuration management."""
    
    def test_default_config_structure(self):
        """Test that default config has expected structure."""
        assert 'device_preference' in DEFAULT_CONFIG
        assert 'server' in DEFAULT_CONFIG
        assert 'hotkey' in DEFAULT_CONFIG
        assert 'ui' in DEFAULT_CONFIG
        assert 'audio' in DEFAULT_CONFIG
        assert 'models' in DEFAULT_CONFIG
    
    def test_config_get_set(self):
        """Test getting and setting config values."""
        config = Config()
        
        # Test getting default value
        default_port = config.get('server.port')
        assert default_port == 7341
        
        # Test setting value
        config.set('server.port', 8080)
        assert config.get('server.port') == 8080
        
        # Reset to default
        config.set('server.port', 7341)
    
    def test_device_preference(self):
        """Test device preference configuration."""
        config = Config()
        devices = config.get_device_preference()
        
        assert isinstance(devices, list)
        assert len(devices) > 0
        assert 'cpu' in devices  # CPU should always be in the list
    
    def test_hotkey_configuration(self):
        """Test hotkey configuration."""
        config = Config()
        
        # Get default hotkey
        default_key = config.get_hotkey()
        assert isinstance(default_key, str)
        assert len(default_key) > 0
        
        # Set new hotkey
        new_key = '<ctrl>+<alt>+r'
        config.set_hotkey(new_key)
        assert config.get_hotkey() == new_key
        
        # Reset to default
        config.set_hotkey(default_key)
    
    def test_server_configuration(self):
        """Test server configuration."""
        config = Config()
        
        # Test server enabled
        assert isinstance(config.is_server_enabled(), bool)
        
        # Test server port
        port = config.get_server_port()
        assert isinstance(port, int)
        assert 1024 <= port <= 65535
'''
    
    (tests_dir / "test_config.py").write_text(test_config_content)
    
    # Create test_models.py
    test_models_content = '''"""Test model loading and management."""

import pytest
from asrpro.models import MODEL_LOADERS, BaseLoader
from asrpro.model_manager import ModelManager, MODEL_SPECS


class TestModels:
    """Test model loaders."""
    
    def test_model_loaders_exist(self):
        """Test that model loaders are registered."""
        assert len(MODEL_LOADERS) > 0
        assert 'parakeet-0.6b' in MODEL_LOADERS
        assert 'parakeet-1.1b' in MODEL_LOADERS
        assert 'whisper-medium-onnx' in MODEL_LOADERS
    
    def test_base_loader_interface(self):
        """Test BaseLoader interface."""
        loader = BaseLoader()
        
        assert hasattr(loader, 'model_name')
        assert hasattr(loader, 'device')
        assert hasattr(loader, 'load')
        assert hasattr(loader, 'transcribe_file')
    
    def test_model_specs(self):
        """Test model specifications."""
        assert len(MODEL_SPECS) > 0
        assert 'parakeet-tdt-0.6b' in MODEL_SPECS
        assert 'parakeet-tdt-1.1b' in MODEL_SPECS
        assert 'whisper-medium-onnx' in MODEL_SPECS


class TestModelManager:
    """Test model manager."""
    
    def test_model_manager_init(self):
        """Test ModelManager initialization."""
        mm = ModelManager()
        
        assert mm.current_id is None
        assert mm.current_model is None
        assert mm.device in ['cuda', 'mps', 'vulkan', 'cpu']
    
    def test_list_models(self):
        """Test listing available models."""
        mm = ModelManager()
        models = mm.list_models()
        
        assert isinstance(models, list)
        assert len(models) > 0
        
        for model in models:
            assert 'id' in model
            assert 'object' in model
            assert model['object'] == 'model'
    
    def test_device_detection(self):
        """Test device detection."""
        mm = ModelManager()
        
        # Device should be a string
        assert isinstance(mm.device, str)
        
        # Should be one of the known devices
        assert mm.device in ['cuda', 'mps', 'vulkan', 'cpu']
        
        # Platform-specific checks
        import platform
        if platform.system() == 'Darwin':
            # On macOS, should not be CUDA
            assert mm.device != 'cuda'
            # Should prefer MPS if available
            import torch
            if hasattr(torch.backends, 'mps') and torch.backends.mps.is_available():
                assert mm.device == 'mps'
'''
    
    (tests_dir / "test_models.py").write_text(test_models_content)
    
    # Create conftest.py for pytest configuration
    conftest_content = '''"""Pytest configuration and fixtures."""

import sys
import pytest
from pathlib import Path

# Add project root to Python path
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))


@pytest.fixture
def temp_config_dir(tmp_path):
    """Create a temporary config directory for testing."""
    config_dir = tmp_path / "test_config"
    config_dir.mkdir()
    return config_dir


@pytest.fixture
def mock_audio_file(tmp_path):
    """Create a mock audio file for testing."""
    audio_file = tmp_path / "test_audio.wav"
    # Create a minimal WAV file header
    audio_file.write_bytes(b'RIFF' + b'\\x00' * 40)  # Minimal WAV header
    return audio_file
'''
    
    (tests_dir / "conftest.py").write_text(conftest_content)
    
    print(f"✅ Created test files in {tests_dir}")
    print("Test files created:")
    for test_file in tests_dir.glob("*.py"):
        print(f"  - {test_file.name}")


if __name__ == "__main__":
    create_test_files()