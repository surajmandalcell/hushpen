"""
Configuration settings for ASR Pro Python Sidecar
"""

import json
import os
from pathlib import Path
from typing import Dict, Any, Optional
import logging

logger = logging.getLogger(__name__)

class Settings:
    """Configuration settings for the sidecar."""
    
    def __init__(self):
        self.config_path = self._get_config_path()
        self.config = self._get_default_config()
        self._ensure_config_directory()
    
    def _get_config_path(self) -> Path:
        """Get platform-specific configuration path."""
        system = os.name
        
        if system == "posix":  # macOS and Linux
            if os.path.exists("/Library/Application Support"):
                # macOS
                return Path.home() / "Library" / "Application Support" / "asrpro" / "config.json"
            else:
                # Linux
                return Path.home() / ".config" / "asrpro" / "config.json"
        elif system == "nt":  # Windows
            return Path(os.environ.get("APPDATA", "")) / "asrpro" / "config.json"
        else:
            # Fallback
            return Path.home() / ".asrpro" / "config.json"
    
    def _ensure_config_directory(self):
        """Ensure configuration directory exists."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
    
    def _get_default_config(self) -> Dict[str, Any]:
        """Get default configuration."""
        return {
            "server": {
                "host": "127.0.0.1",
                "port": 8000
            },
            "models": {
                "default_model": "whisper-base",
                "cache_dir": ""
            },
            "device": {
                "prefer_gpu": True,
                "compute_type": "auto"
            }
        }
    
    async def load_config(self):
        """Load configuration from file."""
        try:
            if self.config_path.exists():
                with open(self.config_path, 'r', encoding='utf-8') as f:
                    loaded_config = json.load(f)
                
                # Merge with default config
                self._merge_config(self.config, loaded_config)
                logger.info(f"Configuration loaded from {self.config_path}")
            else:
                logger.info("No existing configuration found, using defaults")
                await self.save_config()
                
        except Exception as e:
            logger.error(f"Failed to load configuration: {e}")
            self.config = self._get_default_config()
    
    async def save_config(self):
        """Save configuration to file."""
        try:
            with open(self.config_path, 'w', encoding='utf-8') as f:
                json.dump(self.config, f, indent=2, ensure_ascii=False)
            logger.info(f"Configuration saved to {self.config_path}")
        except Exception as e:
            logger.error(f"Failed to save configuration: {e}")
    
    def _merge_config(self, base: Dict[str, Any], update: Dict[str, Any]):
        """Recursively merge configuration dictionaries."""
        for key, value in update.items():
            if key in base and isinstance(base[key], dict) and isinstance(value, dict):
                self._merge_config(base[key], value)
            else:
                base[key] = value
    
    def get_server_config(self) -> Dict[str, Any]:
        """Get server configuration."""
        return self.config.get("server", {})
    
    def get_models_config(self) -> Dict[str, Any]:
        """Get models configuration."""
        return self.config.get("models", {})
    
    def get_device_config(self) -> Dict[str, Any]:
        """Get device configuration."""
        return self.config.get("device", {})
    
    def get_config(self, key: str, default: Any = None) -> Any:
        """Get configuration value by dot notation key."""
        keys = key.split('.')
        value = self.config
        
        for k in keys:
            if isinstance(value, dict) and k in value:
                value = value[k]
            else:
                return default
        
        return value
    
    def set_config(self, key: str, value: Any):
        """Set configuration value by dot notation key."""
        keys = key.split('.')
        config = self.config
        
        # Navigate to the parent of the target key
        for k in keys[:-1]:
            if k not in config:
                config[k] = {}
            config = config[k]
        
        # Set the value
        config[keys[-1]] = value
    
    async def update_config(self, updates: Dict[str, Any]):
        """Update multiple configuration values."""
        self._merge_config(self.config, updates)
        await self.save_config()
