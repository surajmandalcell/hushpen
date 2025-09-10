"""
Keyboard Manager Component - Enterprise Hotkey Management

Handles all keyboard functionality with:
- Global hotkey registration
- Performance optimization
- Event handling
- Conflict resolution
"""

import json
import logging
from typing import Dict, Any, Optional, List, Callable
from .base import BaseComponent

logger = logging.getLogger(__name__)


class KeyboardManager(BaseComponent):
    """
    Enterprise-grade keyboard management component.
    
    Features:
    - Global hotkey registration
    - Conflict detection and resolution
    - Performance-optimized event handling
    - Cross-platform compatibility
    """
    
    def __init__(self, main_window=None):
        super().__init__("keyboard", main_window)
        
        # Hotkey state
        self.registered_hotkeys = {}
        self.active_hotkeys = set()
        self.hotkey_conflicts = {}
        
        # Event handlers
        self._hotkey_handlers: Dict[str, Callable] = {}
        self._global_handler: Optional[Callable] = None
        
        # Performance tracking
        self._hotkey_press_count = 0
        self._last_press_time = None
        
    def initialize(self) -> bool:
        """Initialize the keyboard manager."""
        try:
            self.start_performance_timer("keyboard_init")
            
            # Initialize hotkey system
            self._init_hotkey_system()
            
            # Register default hotkeys
            self._register_default_hotkeys()
            
            self.is_initialized = True
            duration = self.end_performance_timer("keyboard_init")
            
            logger.info(f"[Keyboard] Initialized in {duration:.3f}s")
            return True
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to initialize: {e}")
            return False
    
    def cleanup(self) -> None:
        """Cleanup keyboard resources."""
        try:
            # Unregister all hotkeys
            self._unregister_all_hotkeys()
            
            # Clear handlers
            self._hotkey_handlers.clear()
            self._global_handler = None
            
            # Reset state
            self.registered_hotkeys.clear()
            self.active_hotkeys.clear()
            self.hotkey_conflicts.clear()
            
            logger.info("[Keyboard] Cleanup completed")
        except Exception as e:
            logger.error(f"[Keyboard] Cleanup failed: {e}")
    
    def _init_hotkey_system(self) -> None:
        """Initialize the hotkey system."""
        try:
            # Hotkey system initialization would go here
            # This is a placeholder for actual hotkey library integration
            logger.debug("[Keyboard] Hotkey system initialized")
        except Exception as e:
            logger.error(f"[Keyboard] Hotkey system init failed: {e}")
            raise
    
    def _register_default_hotkeys(self) -> None:
        """Register default application hotkeys."""
        try:
            # Get default hotkeys from settings
            if self.main_window and hasattr(self.main_window, 'settings_manager'):
                settings = self.main_window.settings_manager
                activation_keys = settings.get_setting('activation_keys', ['alt+`'])
                
                for key_combo in activation_keys:
                    self.register_hotkey('record_toggle', key_combo, self._handle_record_toggle)
            
            logger.debug("[Keyboard] Default hotkeys registered")
        except Exception as e:
            logger.error(f"[Keyboard] Default hotkey registration failed: {e}")
    
    def register_hotkey(self, hotkey_id: str, key_combination: str, handler: Callable) -> bool:
        """Register a global hotkey with conflict detection."""
        try:
            # Check for conflicts
            if self._check_hotkey_conflict(key_combination):
                logger.warning(f"[Keyboard] Hotkey conflict detected: {key_combination}")
                self.hotkey_conflicts[hotkey_id] = key_combination
                return False
            
            # Register hotkey (placeholder for actual registration)
            self.registered_hotkeys[hotkey_id] = {
                'key_combination': key_combination,
                'handler': handler,
                'registered_at': self._get_current_time(),
                'press_count': 0
            }
            
            # Store handler
            self._hotkey_handlers[hotkey_id] = handler
            
            # Notify UI
            self.emit_to_frontend('hotkey_registered', {
                'hotkey_id': hotkey_id,
                'key_combination': key_combination
            })
            
            logger.info(f"[Keyboard] Registered hotkey: {hotkey_id} -> {key_combination}")
            return True
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to register hotkey {hotkey_id}: {e}")
            return False
    
    def unregister_hotkey(self, hotkey_id: str) -> bool:
        """Unregister a global hotkey."""
        try:
            if hotkey_id not in self.registered_hotkeys:
                logger.warning(f"[Keyboard] Hotkey not registered: {hotkey_id}")
                return False
            
            # Unregister hotkey (placeholder for actual unregistration)
            hotkey_info = self.registered_hotkeys[hotkey_id]
            key_combination = hotkey_info['key_combination']
            
            # Remove from tracking
            del self.registered_hotkeys[hotkey_id]
            if hotkey_id in self._hotkey_handlers:
                del self._hotkey_handlers[hotkey_id]
            
            # Remove from conflicts if present
            if hotkey_id in self.hotkey_conflicts:
                del self.hotkey_conflicts[hotkey_id]
            
            # Notify UI
            self.emit_to_frontend('hotkey_unregistered', {
                'hotkey_id': hotkey_id,
                'key_combination': key_combination
            })
            
            logger.info(f"[Keyboard] Unregistered hotkey: {hotkey_id}")
            return True
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to unregister hotkey {hotkey_id}: {e}")
            return False
    
    def _unregister_all_hotkeys(self) -> None:
        """Unregister all hotkeys."""
        try:
            hotkey_ids = list(self.registered_hotkeys.keys())
            for hotkey_id in hotkey_ids:
                self.unregister_hotkey(hotkey_id)
                
            logger.info(f"[Keyboard] Unregistered {len(hotkey_ids)} hotkeys")
        except Exception as e:
            logger.error(f"[Keyboard] Failed to unregister all hotkeys: {e}")
    
    def _check_hotkey_conflict(self, key_combination: str) -> bool:
        """Check if a key combination conflicts with existing hotkeys."""
        try:
            # Check against registered hotkeys
            for hotkey_info in self.registered_hotkeys.values():
                if hotkey_info['key_combination'].lower() == key_combination.lower():
                    return True
            
            # Check against system hotkeys (placeholder)
            # This would integrate with OS-specific hotkey detection
            
            return False
            
        except Exception as e:
            logger.error(f"[Keyboard] Conflict check failed: {e}")
            return True  # Assume conflict on error for safety
    
    def handle_hotkey_press(self, hotkey_id: str) -> bool:
        """Handle hotkey press with performance tracking."""
        try:
            self.start_performance_timer(f"hotkey_{hotkey_id}")
            
            # Update metrics
            self._hotkey_press_count += 1
            self._last_press_time = self._get_current_time()
            
            # Update hotkey statistics
            if hotkey_id in self.registered_hotkeys:
                self.registered_hotkeys[hotkey_id]['press_count'] += 1
            
            # Execute handler
            if hotkey_id in self._hotkey_handlers:
                handler = self._hotkey_handlers[hotkey_id]
                result = handler()
                
                # Execute global handler if set
                if self._global_handler:
                    self._global_handler(hotkey_id)
                
                # Notify UI
                self.emit_to_frontend('hotkey_pressed', {
                    'hotkey_id': hotkey_id,
                    'timestamp': self._last_press_time
                })
                
                duration = self.end_performance_timer(f"hotkey_{hotkey_id}")
                logger.debug(f"[Keyboard] Hotkey {hotkey_id} handled in {duration:.3f}s")
                
                return result if isinstance(result, bool) else True
            
            logger.warning(f"[Keyboard] No handler for hotkey: {hotkey_id}")
            return False
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to handle hotkey {hotkey_id}: {e}")
            return False
    
    def _handle_record_toggle(self) -> bool:
        """Handle record toggle hotkey."""
        try:
            # Get recording manager
            if self.main_window and hasattr(self.main_window, 'recording_manager'):
                recording_manager = self.main_window.recording_manager
                
                if recording_manager.is_recording:
                    return recording_manager.stop_recording()
                else:
                    return recording_manager.start_recording()
            
            logger.warning("[Keyboard] Recording manager not available")
            return False
            
        except Exception as e:
            logger.error(f"[Keyboard] Record toggle failed: {e}")
            return False
    
    def get_registered_hotkeys(self) -> Dict[str, Dict[str, Any]]:
        """Get all registered hotkeys with statistics."""
        return self.registered_hotkeys.copy()
    
    def get_hotkey_conflicts(self) -> Dict[str, str]:
        """Get hotkey conflicts."""
        return self.hotkey_conflicts.copy()
    
    def set_global_handler(self, handler: Callable) -> None:
        """Set global hotkey handler."""
        self._global_handler = handler
    
    def update_hotkey(self, hotkey_id: str, new_key_combination: str) -> bool:
        """Update an existing hotkey with a new key combination."""
        try:
            if hotkey_id not in self.registered_hotkeys:
                logger.warning(f"[Keyboard] Hotkey not registered: {hotkey_id}")
                return False
            
            # Get current handler
            handler = self._hotkey_handlers.get(hotkey_id)
            if not handler:
                logger.error(f"[Keyboard] No handler found for {hotkey_id}")
                return False
            
            # Unregister old hotkey
            self.unregister_hotkey(hotkey_id)
            
            # Register with new combination
            return self.register_hotkey(hotkey_id, new_key_combination, handler)
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to update hotkey {hotkey_id}: {e}")
            return False
    
    def _get_current_time(self) -> float:
        """Get current timestamp."""
        import time
        return time.time()
    
    # Bridge Signal Handlers
    
    def _handle_register_hotkey(self, data: str) -> bool:
        """Handle register hotkey signal from JavaScript."""
        try:
            hotkey_data = json.loads(data)
            hotkey_id = hotkey_data.get('hotkey_id')
            key_combination = hotkey_data.get('key_combination')
            
            if not hotkey_id or not key_combination:
                logger.warning("[Keyboard] Invalid hotkey registration data")
                return False
            
            # Use default handler for UI-registered hotkeys
            return self.register_hotkey(hotkey_id, key_combination, lambda: self._handle_ui_hotkey(hotkey_id))
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to register hotkey from UI: {e}")
            return False
    
    def _handle_unregister_hotkey(self, data: str) -> bool:
        """Handle unregister hotkey signal from JavaScript."""
        try:
            hotkey_data = json.loads(data)
            hotkey_id = hotkey_data.get('hotkey_id')
            
            if not hotkey_id:
                logger.warning("[Keyboard] Invalid hotkey unregistration data")
                return False
            
            return self.unregister_hotkey(hotkey_id)
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to unregister hotkey from UI: {e}")
            return False
    
    def _handle_get_hotkeys(self) -> bool:
        """Handle get hotkeys signal from JavaScript."""
        try:
            hotkeys = self.get_registered_hotkeys()
            conflicts = self.get_hotkey_conflicts()
            
            self.emit_to_frontend('hotkeys_data', {
                'hotkeys': hotkeys,
                'conflicts': conflicts,
                'total_presses': self._hotkey_press_count
            })
            
            return True
            
        except Exception as e:
            logger.error(f"[Keyboard] Failed to get hotkeys: {e}")
            return False
    
    def _handle_ui_hotkey(self, hotkey_id: str) -> bool:
        """Handle hotkey triggered from UI registration."""
        try:
            # Emit to frontend for UI-specific handling
            self.emit_to_frontend('ui_hotkey_triggered', {'hotkey_id': hotkey_id})
            return True
        except Exception as e:
            logger.error(f"[Keyboard] UI hotkey handling failed: {e}")
            return False