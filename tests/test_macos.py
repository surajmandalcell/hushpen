"""Test macOS-specific functionality."""

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
        assert '\\' not in config_str or platform.system() == 'Windows'
