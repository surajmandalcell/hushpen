"""Test that all modules can be imported without errors."""

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
