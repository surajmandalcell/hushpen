"""
Enterprise UI Components Module

This module provides a modular, enterprise-grade architecture for the ASR Pro UI.
All UI components are organized by functionality with clean separation of concerns.
"""

from .base import BaseComponent
from .settings import SettingsManager
from .recording import RecordingManager
from .keyboard import KeyboardManager
from .about import AboutManager

__all__ = [
    'BaseComponent',
    'SettingsManager', 
    'RecordingManager',
    'KeyboardManager',
    'AboutManager'
]