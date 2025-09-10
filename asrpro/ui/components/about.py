"""
About Manager Component - Enterprise Application Information

Handles application information with:
- Version management
- System information
- Performance metrics
- License information
"""

import json
import platform
import sys
import logging
from typing import Dict, Any, Optional
from pathlib import Path
from .base import BaseComponent

logger = logging.getLogger(__name__)


class AboutManager(BaseComponent):
    """
    Enterprise-grade about information management component.
    
    Features:
    - Application version tracking
    - System information collection
    - Performance metrics aggregation
    - License and credit management
    """
    
    VERSION = "2.0.0"
    BUILD_NUMBER = "20240301"
    
    def __init__(self, main_window=None):
        super().__init__("about", main_window)
        
        # Application info
        self.app_info = {}
        self.system_info = {}
        self.performance_metrics = {}
        self.license_info = {}
        
        # Cache for expensive operations
        self._system_info_cache = None
        self._performance_cache = None
        self._cache_timestamp = None
        
    def initialize(self) -> bool:
        """Initialize the about manager."""
        try:
            self.start_performance_timer("about_init")
            
            # Initialize application info
            self._init_app_info()
            
            # Collect system information
            self._collect_system_info()
            
            # Load license information
            self._load_license_info()
            
            self.is_initialized = True
            duration = self.end_performance_timer("about_init")
            
            logger.info(f"[About] Initialized in {duration:.3f}s")
            return True
            
        except Exception as e:
            logger.error(f"[About] Failed to initialize: {e}")
            return False
    
    def cleanup(self) -> None:
        """Cleanup about manager resources."""
        try:
            # Clear caches
            self._system_info_cache = None
            self._performance_cache = None
            
            # Clear data
            self.app_info.clear()
            self.system_info.clear()
            self.performance_metrics.clear()
            
            logger.info("[About] Cleanup completed")
        except Exception as e:
            logger.error(f"[About] Cleanup failed: {e}")
    
    def _init_app_info(self) -> None:
        """Initialize application information."""
        try:
            self.app_info = {
                'name': 'ASR Pro',
                'version': self.VERSION,
                'build_number': self.BUILD_NUMBER,
                'description': 'Enterprise-grade Automatic Speech Recognition',
                'author': 'ASR Pro Team',
                'website': 'https://asrpro.example.com',
                'support_email': 'support@asrpro.example.com',
                'release_date': '2024-03-01',
                'python_version': f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}",
                'architecture': platform.architecture()[0],
                'build_type': 'Enterprise'
            }
            
            logger.debug("[About] Application info initialized")
        except Exception as e:
            logger.error(f"[About] App info initialization failed: {e}")
            raise
    
    def _collect_system_info(self) -> None:
        """Collect comprehensive system information with caching."""
        try:
            import time
            current_time = time.time()
            
            # Use cache if available and recent (5 minutes)
            if (self._system_info_cache and self._cache_timestamp and 
                current_time - self._cache_timestamp < 300):
                self.system_info = self._system_info_cache
                return
            
            # Collect fresh system info
            self.system_info = {
                'platform': platform.system(),
                'platform_release': platform.release(),
                'platform_version': platform.version(),
                'machine': platform.machine(),
                'processor': platform.processor(),
                'python_implementation': platform.python_implementation(),
                'python_version': platform.python_version(),
                'hostname': platform.node(),
            }
            
            # Add memory information
            try:
                import psutil
                memory = psutil.virtual_memory()
                self.system_info.update({
                    'total_memory': memory.total,
                    'available_memory': memory.available,
                    'memory_percent': memory.percent,
                    'cpu_count': psutil.cpu_count(),
                    'cpu_count_logical': psutil.cpu_count(logical=True)
                })
            except ImportError:
                logger.warning("[About] psutil not available for memory info")
            
            # Cache the results
            self._system_info_cache = self.system_info.copy()
            self._cache_timestamp = current_time
            
            logger.debug("[About] System info collected")
            
        except Exception as e:
            logger.error(f"[About] System info collection failed: {e}")
    
    def _load_license_info(self) -> None:
        """Load license and credit information."""
        try:
            self.license_info = {
                'license_type': 'Enterprise License',
                'license_version': '1.0',
                'copyright': f'© 2024 ASR Pro Team. All rights reserved.',
                'third_party_licenses': [
                    {
                        'name': 'PyQt6',
                        'version': '6.x',
                        'license': 'GPL v3 / Commercial',
                        'description': 'Cross-platform GUI toolkit'
                    },
                    {
                        'name': 'NumPy',
                        'version': '1.x',
                        'license': 'BSD-3-Clause',
                        'description': 'Numerical computing library'
                    },
                    {
                        'name': 'SciPy',
                        'version': '1.x', 
                        'license': 'BSD-3-Clause',
                        'description': 'Scientific computing library'
                    }
                ],
                'credits': [
                    'Development Team',
                    'QA Team',
                    'Design Team',
                    'Open Source Contributors'
                ]
            }
            
            logger.debug("[About] License info loaded")
            
        except Exception as e:
            logger.error(f"[About] License info loading failed: {e}")
    
    def get_app_info(self) -> Dict[str, Any]:
        """Get application information."""
        return self.app_info.copy()
    
    def get_system_info(self) -> Dict[str, Any]:
        """Get system information with performance optimization."""
        try:
            # Refresh if cache is stale
            import time
            if (not self._system_info_cache or not self._cache_timestamp or 
                time.time() - self._cache_timestamp > 300):
                self._collect_system_info()
            
            return self.system_info.copy()
            
        except Exception as e:
            logger.error(f"[About] Failed to get system info: {e}")
            return {}
    
    def get_performance_metrics(self) -> Dict[str, Any]:
        """Get aggregated performance metrics from all components."""
        try:
            import time
            current_time = time.time()
            
            # Use cache if available and recent (1 minute)
            if (self._performance_cache and self._cache_timestamp and 
                current_time - self._cache_timestamp < 60):
                return self._performance_cache
            
            # Collect performance metrics from all components
            metrics = {
                'app_uptime': current_time - self._get_app_start_time(),
                'total_operations': 0,
                'average_response_time': 0.0,
                'memory_usage': self._get_memory_usage(),
                'component_metrics': {}
            }
            
            # Collect from main window components
            if self.main_window:
                for attr_name in dir(self.main_window):
                    if attr_name.endswith('_manager'):
                        component = getattr(self.main_window, attr_name)
                        if hasattr(component, '_performance_metrics'):
                            metrics['component_metrics'][attr_name] = component._performance_metrics.copy()
            
            # Cache the results
            self._performance_cache = metrics
            self._cache_timestamp = current_time
            
            return metrics
            
        except Exception as e:
            logger.error(f"[About] Failed to get performance metrics: {e}")
            return {}
    
    def get_license_info(self) -> Dict[str, Any]:
        """Get license and credit information."""
        return self.license_info.copy()
    
    def get_complete_about_info(self) -> Dict[str, Any]:
        """Get complete about information including all sections."""
        try:
            return {
                'application': self.get_app_info(),
                'system': self.get_system_info(),
                'performance': self.get_performance_metrics(),
                'license': self.get_license_info(),
                'timestamp': self._get_current_timestamp()
            }
            
        except Exception as e:
            logger.error(f"[About] Failed to get complete info: {e}")
            return {}
    
    def export_system_report(self, filepath: str) -> bool:
        """Export comprehensive system report to file."""
        try:
            report_data = self.get_complete_about_info()
            
            # Add additional diagnostic information
            report_data['diagnostics'] = {
                'log_level': logging.getLogger().getEffectiveLevel(),
                'working_directory': str(Path.cwd()),
                'executable_path': sys.executable,
                'python_path': sys.path[:5],  # First 5 entries only
                'environment_vars': {
                    key: value for key, value in [
                        ('PATH', len(str(Path.cwd()).split(';'))),
                        ('PYTHONPATH', sys.path[0] if sys.path else 'Not set'),
                        ('HOME', str(Path.home()))
                    ]
                }
            }
            
            # Write to file
            with open(filepath, 'w', encoding='utf-8') as f:
                json.dump(report_data, f, indent=2, default=str)
                
            logger.info(f"[About] System report exported to {filepath}")
            return True
            
        except Exception as e:
            logger.error(f"[About] Failed to export system report: {e}")
            return False
    
    def _get_app_start_time(self) -> float:
        """Get application start time (placeholder)."""
        try:
            if self.main_window and hasattr(self.main_window, '_start_time'):
                return self.main_window._start_time
            return 0.0
        except:
            return 0.0
    
    def _get_memory_usage(self) -> Dict[str, Any]:
        """Get current memory usage."""
        try:
            import psutil
            process = psutil.Process()
            memory_info = process.memory_info()
            
            return {
                'rss': memory_info.rss,  # Resident Set Size
                'vms': memory_info.vms,  # Virtual Memory Size
                'percent': process.memory_percent(),
                'available': psutil.virtual_memory().available
            }
        except ImportError:
            return {'error': 'psutil not available'}
        except Exception as e:
            return {'error': str(e)}
    
    def _get_current_timestamp(self) -> str:
        """Get current timestamp in ISO format."""
        from datetime import datetime
        return datetime.now().isoformat()
    
    # Bridge Signal Handlers
    
    def _handle_get_app_info(self) -> bool:
        """Handle get app info signal from JavaScript."""
        try:
            app_info = self.get_app_info()
            self.emit_to_frontend('app_info_loaded', app_info)
            return True
        except Exception as e:
            logger.error(f"[About] Failed to get app info: {e}")
            return False
    
    def _handle_get_system_info(self) -> bool:
        """Handle get system info signal from JavaScript."""
        try:
            system_info = self.get_system_info()
            self.emit_to_frontend('system_info_loaded', system_info)
            return True
        except Exception as e:
            logger.error(f"[About] Failed to get system info: {e}")
            return False
    
    def _handle_get_performance_metrics(self) -> bool:
        """Handle get performance metrics signal from JavaScript."""
        try:
            metrics = self.get_performance_metrics()
            self.emit_to_frontend('performance_metrics_loaded', metrics)
            return True
        except Exception as e:
            logger.error(f"[About] Failed to get performance metrics: {e}")
            return False
    
    def _handle_get_complete_info(self) -> bool:
        """Handle get complete about info signal from JavaScript."""
        try:
            complete_info = self.get_complete_about_info()
            self.emit_to_frontend('complete_about_info_loaded', complete_info)
            return True
        except Exception as e:
            logger.error(f"[About] Failed to get complete info: {e}")
            return False
    
    def _handle_export_system_report(self, data: str) -> bool:
        """Handle export system report signal from JavaScript."""
        try:
            export_data = json.loads(data) if data else {}
            filepath = export_data.get('filepath', 'system_report.json')
            
            return self.export_system_report(filepath)
            
        except Exception as e:
            logger.error(f"[About] Failed to export system report: {e}")
            return False