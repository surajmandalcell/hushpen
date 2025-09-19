"""
Process Manager for ASR Pro Python Sidecar
"""

import os
import psutil
import signal
from pathlib import Path
from typing import Optional
import logging

logger = logging.getLogger(__name__)

class ProcessManager:
    """Manages single instance enforcement and process lifecycle."""
    
    def __init__(self):
        self.pid_file = Path.home() / ".asrpro.pid"
        self.current_pid = os.getpid()
    
    def is_instance_running(self) -> bool:
        """Check if another instance is already running."""
        try:
            # Check if PID file exists
            if not self.pid_file.exists():
                return False
            
            # Read PID from file
            with open(self.pid_file, 'r') as f:
                pid = int(f.read().strip())
            
            # Check if process with that PID exists and is our process
            if pid == self.current_pid:
                return False
            
            # Check if process is running
            if psutil.pid_exists(pid):
                process = psutil.Process(pid)
                # Check if it's an ASR Pro process
                try:
                    cmdline = process.cmdline()
                    if any('asrpro' in cmd.lower() for cmd in cmdline):
                        logger.warning(f"Another ASR Pro instance is running with PID {pid}")
                        return True
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass
            
            # PID file exists but process is not running, clean it up
            self._cleanup_pid_file()
            return False
            
        except (ValueError, FileNotFoundError, psutil.NoSuchProcess):
            # Invalid PID file, clean it up
            self._cleanup_pid_file()
            return False
        except Exception as e:
            logger.error(f"Error checking for existing instance: {e}")
            return False
    
    def create_pid_file(self):
        """Create PID file for current process."""
        try:
            with open(self.pid_file, 'w') as f:
                f.write(str(self.current_pid))
            logger.debug(f"Created PID file: {self.pid_file}")
        except Exception as e:
            logger.error(f"Failed to create PID file: {e}")
    
    def _cleanup_pid_file(self):
        """Remove PID file."""
        try:
            if self.pid_file.exists():
                self.pid_file.unlink()
                logger.debug("Cleaned up PID file")
        except Exception as e:
            logger.error(f"Failed to cleanup PID file: {e}")
    
    def setup_signal_handlers(self):
        """Setup signal handlers for graceful shutdown."""
        def signal_handler(signum, frame):
            logger.info(f"Received signal {signum}, shutting down...")
            self._cleanup_pid_file()
            os._exit(0)
        
        # Register signal handlers
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)
        
        # On Windows, handle CTRL+C and CTRL+BREAK
        if os.name == 'nt':
            signal.signal(signal.SIGBREAK, signal_handler)
    
    def get_process_info(self) -> dict:
        """Get information about current process."""
        try:
            process = psutil.Process(self.current_pid)
            return {
                'pid': self.current_pid,
                'name': process.name(),
                'cpu_percent': process.cpu_percent(),
                'memory_percent': process.memory_percent(),
                'memory_info': process.memory_info()._asdict(),
                'create_time': process.create_time(),
                'status': process.status()
            }
        except psutil.NoSuchProcess:
            return {}
        except Exception as e:
            logger.error(f"Error getting process info: {e}")
            return {}
    
    def kill_existing_instance(self) -> bool:
        """Kill existing ASR Pro instance."""
        try:
            if not self.pid_file.exists():
                return False
            
            with open(self.pid_file, 'r') as f:
                pid = int(f.read().strip())
            
            if pid == self.current_pid:
                return False
            
            if psutil.pid_exists(pid):
                process = psutil.Process(pid)
                # Check if it's an ASR Pro process
                try:
                    cmdline = process.cmdline()
                    if any('asrpro' in cmd.lower() for cmd in cmdline):
                        logger.info(f"Terminating existing ASR Pro instance with PID {pid}")
                        process.terminate()
                        
                        # Wait for process to terminate
                        try:
                            process.wait(timeout=5)
                        except psutil.TimeoutExpired:
                            logger.warning("Process did not terminate gracefully, forcing kill")
                            process.kill()
                        
                        self._cleanup_pid_file()
                        return True
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass
            
            return False
            
        except Exception as e:
            logger.error(f"Error killing existing instance: {e}")
            return False
    
    def get_system_resources(self) -> dict:
        """Get system resource information."""
        try:
            return {
                'cpu_count': psutil.cpu_count(),
                'cpu_percent': psutil.cpu_percent(interval=1),
                'memory_total': psutil.virtual_memory().total,
                'memory_available': psutil.virtual_memory().available,
                'memory_percent': psutil.virtual_memory().percent,
                'disk_usage': {
                    'total': psutil.disk_usage('/').total,
                    'used': psutil.disk_usage('/').used,
                    'free': psutil.disk_usage('/').free
                }
            }
        except Exception as e:
            logger.error(f"Error getting system resources: {e}")
            return {}
