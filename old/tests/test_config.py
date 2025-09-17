"""Test configuration management."""

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
