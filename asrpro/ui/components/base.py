"""
Base Component for Enterprise UI Architecture

This provides the foundation for all UI components with:
- Performance optimization
- Memory management
- Error handling
- Logging
"""

from abc import ABC, abstractmethod
from typing import Dict, Any, Optional
import json
import logging

logger = logging.getLogger(__name__)


class BaseComponent(ABC):
    """
    Abstract base class for all UI components.
    
    Provides enterprise-grade foundation with:
    - Standardized initialization
    - Configuration management
    - Error handling
    - Performance monitoring
    """
    
    def __init__(self, component_id: str, main_window=None):
        self.component_id = component_id
        self.main_window = main_window
        self.config = {}
        self.is_initialized = False
        self._performance_metrics = {}
        
        logger.info(f"[{self.component_id}] Component created")
    
    @abstractmethod
    def initialize(self) -> bool:
        """Initialize the component. Must be implemented by subclasses."""
        pass
    
    @abstractmethod
    def cleanup(self) -> None:
        """Cleanup component resources. Must be implemented by subclasses."""
        pass
    
    def load_config(self, config: Dict[str, Any]) -> None:
        """Load component configuration."""
        try:
            self.config = config.copy()
            logger.info(f"[{self.component_id}] Configuration loaded")
        except Exception as e:
            logger.error(f"[{self.component_id}] Failed to load config: {e}")
    
    def get_config(self) -> Dict[str, Any]:
        """Get current component configuration."""
        return self.config.copy()
    
    def handle_bridge_signal(self, signal: str, data: Optional[str] = None) -> bool:
        """
        Handle signals from the JavaScript bridge.
        
        Args:
            signal: The signal name
            data: Optional signal data
            
        Returns:
            True if signal was handled, False otherwise
        """
        try:
            method_name = f"_handle_{signal.lower().replace('_signal', '')}"
            if hasattr(self, method_name):
                method = getattr(self, method_name)
                if data:
                    result = method(data)
                else:
                    result = method()
                    
                logger.debug(f"[{self.component_id}] Handled signal: {signal}")
                return result if isinstance(result, bool) else True
                
        except Exception as e:
            logger.error(f"[{self.component_id}] Error handling signal {signal}: {e}")
            
        return False
    
    def emit_to_frontend(self, event: str, data: Dict[str, Any] = None) -> None:
        """
        Emit data to the frontend JavaScript.
        
        This is a placeholder for future WebChannel implementation.
        """
        try:
            if self.main_window and hasattr(self.main_window, 'emit_to_frontend'):
                self.main_window.emit_to_frontend(self.component_id, event, data or {})
            else:
                logger.debug(f"[{self.component_id}] Would emit: {event} with {data}")
        except Exception as e:
            logger.error(f"[{self.component_id}] Failed to emit {event}: {e}")
    
    def validate_data(self, data: Dict[str, Any], schema: Dict[str, Any]) -> bool:
        """
        Validate data against a simple schema.
        
        Args:
            data: Data to validate
            schema: Validation schema
            
        Returns:
            True if valid, False otherwise
        """
        try:
            for key, expected_type in schema.items():
                if key not in data:
                    logger.warning(f"[{self.component_id}] Missing required field: {key}")
                    return False
                    
                if not isinstance(data[key], expected_type):
                    logger.warning(f"[{self.component_id}] Invalid type for {key}")
                    return False
                    
            return True
            
        except Exception as e:
            logger.error(f"[{self.component_id}] Validation error: {e}")
            return False
    
    def start_performance_timer(self, operation: str) -> None:
        """Start timing a performance operation."""
        import time
        self._performance_metrics[operation] = time.time()
    
    def end_performance_timer(self, operation: str) -> float:
        """End timing and return duration in seconds."""
        import time
        if operation in self._performance_metrics:
            duration = time.time() - self._performance_metrics[operation]
            del self._performance_metrics[operation]
            logger.debug(f"[{self.component_id}] {operation} took {duration:.3f}s")
            return duration
        return 0.0
    
    def __repr__(self) -> str:
        return f"{self.__class__.__name__}(id={self.component_id}, initialized={self.is_initialized})"