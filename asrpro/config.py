"""Configuration management for asrpro."""

from __future__ import annotations
import json
from pathlib import Path
from typing import Dict, Any, Optional

# Default configuration
DEFAULT_CONFIG = {
    "device_preference": ["cuda", "vulkan", "cpu"],
    "server": {"enabled": True, "host": "127.0.0.1", "port": 7341, "auto_start": True},
    "ui": {
        "theme": "dark",
        "opacity": 0.95,
        "remember_window_position": True,
        "enable_animations": True,
    },
    "audio": {
        "sample_rate": 16000,
        "channels": 1,
        "format": "wav",
        "quality": "medium",
    },
    "models": {
        "auto_unload_after_minutes": 30,
        "prefer_gpu": True,
        "cache_directory": None,  # None = use default
    },
    "hotkey": {
        "combination": "<ctrl>+<alt>+t",
        "enable_overlay": True,
        "auto_paste": True,
    },
}


class Config:
    """Configuration manager for asrpro settings."""

    def __init__(self):
        self.config_dir = Path.home() / ".asrpro"
        self.config_file = self.config_dir / "config.json"
        self.config_dir.mkdir(exist_ok=True)
        self._config = self._load_config()

    def _load_config(self) -> Dict[str, Any]:
        """Load configuration from file or create default."""
        if self.config_file.exists():
            try:
                with open(self.config_file, "r", encoding="utf-8") as f:
                    loaded_config = json.load(f)
                # Merge with defaults to ensure all keys exist
                return self._merge_configs(DEFAULT_CONFIG, loaded_config)
            except Exception:
                # If config is corrupted, use defaults
                pass

        # Use defaults and save them
        self._save_config(DEFAULT_CONFIG)
        return DEFAULT_CONFIG.copy()

    def _merge_configs(
        self, default: Dict[str, Any], loaded: Dict[str, Any]
    ) -> Dict[str, Any]:
        """Recursively merge configurations, with loaded taking precedence."""
        result = default.copy()
        for key, value in loaded.items():
            if (
                key in result
                and isinstance(result[key], dict)
                and isinstance(value, dict)
            ):
                result[key] = self._merge_configs(result[key], value)
            else:
                result[key] = value
        return result

    def _save_config(self, config: Dict[str, Any]) -> None:
        """Save configuration to file."""
        try:
            with open(self.config_file, "w", encoding="utf-8") as f:
                json.dump(config, f, indent=2, ensure_ascii=False)
        except Exception:
            pass  # Fail silently

    def get(self, key_path: str, default: Any = None) -> Any:
        """Get configuration value using dot notation (e.g., 'server.port')."""
        keys = key_path.split(".")
        value = self._config

        for key in keys:
            if isinstance(value, dict) and key in value:
                value = value[key]
            else:
                return default

        return value

    def set(self, key_path: str, value: Any) -> None:
        """Set configuration value using dot notation."""
        keys = key_path.split(".")
        config = self._config

        # Navigate to the parent of the target key
        for key in keys[:-1]:
            if key not in config:
                config[key] = {}
            config = config[key]

        # Set the final value
        config[keys[-1]] = value
        self._save_config(self._config)

    def get_device_preference(self) -> list[str]:
        """Get ordered list of preferred devices."""
        return self.get("device_preference", ["cuda", "vulkan", "cpu"])

    def get_server_config(self) -> Dict[str, Any]:
        """Get server configuration."""
        return self.get("server", DEFAULT_CONFIG["server"])

    def get_hotkey(self) -> str:
        """Get current hotkey combination."""
        return self.get("hotkey.combination", DEFAULT_CONFIG["hotkey"]["combination"])

    def set_hotkey(self, combination: str) -> None:
        """Set hotkey combination."""
        self.set("hotkey.combination", combination)

    def is_server_enabled(self) -> bool:
        """Check if server should be started."""
        return self.get("server.enabled", True)

    def get_server_port(self) -> int:
        """Get server port."""
        return self.get("server.port", 7341)


# Global config instance
config = Config()

__all__ = ["config", "Config", "DEFAULT_CONFIG"]
